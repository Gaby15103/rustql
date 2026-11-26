use std::cmp::Ordering;
use std::collections::HashSet;
use crate::btree::Cursor;
use crate::btree::{LEAF_NODE_MAX_CELLS, LeafNode, leaf_node_split_and_insert};
use crate::sql_parser::{BinaryOperator, DataType, UnaryOperator};
use crate::sql_parser::Expr;
use crate::sql_parser::LiteralValue;
use crate::sql_parser::SelectItem;
use crate::sql_parser::{
    CreateTableStmt, DeleteStmt, InsertStmt, SelectStmt, Statement, UpdateStmt,
};
use crate::storage::Column;
use crate::storage::TableEntry;
use crate::storage::ValueType;
use crate::storage::{Database, Row, TableSchema, Value};

pub enum ExecuteError {
    TableFull,
    DuplicateKey,
    InvalidRow,
    TableNotFound,
    ColumnNotFound,
    TableAlreadyExists,
    UnsupportedStatement,
    UnsupportedExpression,
}

pub fn execute_statement(
    statement: &Statement,
    database: &mut Database,
) -> Result<(), ExecuteError> {
    match statement {
        Statement::CreateTable(statement) => execute_create_table(statement, database),
        Statement::Insert(statement) => execute_insert(statement, database),
        Statement::Select(statement) => execute_select(statement, database),
        Statement::Update(statement) => execute_update(statement, database),
        Statement::Delete(statement) => execute_delete(statement, database),
        _ => Err(ExecuteError::UnsupportedStatement),
    }
}

fn execute_create_table(
    statement: &CreateTableStmt,
    database: &mut Database,
) -> Result<(), ExecuteError> {
    if statement.columns.is_empty() {
        return Err(ExecuteError::InvalidRow);
    }

    let mut primary_key_index = None;
    let mut schema_columns = Vec::with_capacity(statement.columns.len());

    for (index, column) in statement.columns.iter().enumerate() {
        if column.is_primary_key {
            if primary_key_index.is_some() {
                return Err(ExecuteError::InvalidRow);
            }
            primary_key_index = Some(index);
        }

        let value_type = match column.data_type {
            DataType::Int => ValueType::Integer,
            DataType::Text => ValueType::Text,
            DataType::Bool => ValueType::Bool,
        };

        schema_columns.push(Column {
            name: column.name.clone(),
            value_type,
            max_length: match column.data_type {
                DataType::Text => Some(255),
                _ => None,
            },
        });
    }

    let schema = TableSchema {
        name: statement.table.clone(),
        columns: schema_columns,
        primary_key_index,
    };

    match database.create_table(schema) {
        Ok(()) => Ok(()),
        Err(error) if error.starts_with("Table ") && error.ends_with("already exists") => {
            if statement.if_not_exists {
                Ok(())
            } else {
                Err(ExecuteError::TableAlreadyExists)
            }
        }
        Err(error) => {
            eprintln!("Could not create table: {error}");
            Err(ExecuteError::InvalidRow)
        }
    }
}

fn execute_insert(statement: &InsertStmt, mut database: &mut Database) -> Result<(), ExecuteError> {
    let entry = database
        .table(&statement.table)
        .ok_or(ExecuteError::TableNotFound)?
        .clone();

    for parsed_row in &statement.values {
        let row = bind_insert_row(&entry.schema, statement.columns.as_deref(), parsed_row)?;
        // let row = entry.schema.make_row(values)
        //     .map_err(|_| ExecuteError::InvalidRow)?;

        insert_row(&mut database, &entry, &row)?;
    }

    Ok(())
}

fn execute_select(statement: &SelectStmt, database: &mut Database) -> Result<(), ExecuteError> {
    let entry = database
        .table(&statement.from)
        .ok_or(ExecuteError::TableNotFound)?
        .clone();

    let projection = validate_select(statement, &entry.schema)?;

    if let Some(order_by) = &statement.order_by {
        column_index(&entry.schema, &order_by.column)?;
    }

    let mut rows = read_table_rows(database, &entry)?;

    if let Some(where_clause) = &statement.where_clause {
        let mut matching_rows = Vec::new();
        for row in rows {
            if evaluate_condition(where_clause, &row, &entry.schema)? {
                matching_rows.push(row);
            }
        }
        rows = matching_rows;
    }

    if let Some(order_by) = &statement.order_by {
        let index = column_index(&entry.schema, &order_by.column)?;
        let descending = order_by.descending;

        rows.sort_by(|left, right| {
            let ordering = compare_for_order(&left.values[index], &right.values[index]);
            if descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
    }

    if let Some(limit) = statement.limit {
        rows.truncate(limit);
    }

    for row in rows {
        let projected = Row {
            values: projection
                .iter()
                .map(|&index| row.values[index].clone())
                .collect(),
        };
        println!("{projected}");
    }

    Ok(())
}
fn execute_update(statement: &UpdateStmt, database: &mut Database) -> Result<(), ExecuteError> {
    let entry = database
        .table(&statement.table)
        .ok_or(ExecuteError::TableNotFound)?
        .clone();

    if statement.assignments.is_empty() {
        return Err(ExecuteError::InvalidRow);
    }

    if let Some(where_clause) = &statement.where_clause {
        validate_select_expr(where_clause, &entry.schema)?;
    }

    let mut assignment_indices = Vec::with_capacity(statement.assignments.len());
    let mut seen_columns = HashSet::new();

    for assignment in &statement.assignments {
        let index = column_index(&entry.schema, &assignment.column)?;
        if !seen_columns.insert(index) {
            return Err(ExecuteError::InvalidRow);
        }
        assignment_indices.push(index);

        // Check column references before changing any rows.
        validate_select_expr(&assignment.value, &entry.schema)?;
    }

    let mut rows = read_table_rows(database, &entry)?;

    for row in &mut rows {
        let matches = match &statement.where_clause {
            Some(condition) => matches!(
                evaluate_condition(condition, row, &entry.schema)?,
                true
            ),
            None => true,
        };

        if !matches {
            continue;
        }

        // Evaluate each right-hand side against the original row, as SQL UPDATE
        // normally does, then apply all resulting values together.
        let updated_values = statement
            .assignments
            .iter()
            .map(|assignment| evaluate_value(&assignment.value, row, &entry.schema))
            .collect::<Result<Vec<_>, _>>()?;

        for (&index, value) in assignment_indices.iter().zip(updated_values) {
            row.values[index] = value;
        }

        *row = entry
            .schema
            .make_row(row.values.clone())
            .map_err(|_| ExecuteError::InvalidRow)?;
    }

    rewrite_table(database, &entry, &rows)
}
fn execute_delete(statement: &DeleteStmt, database: &mut Database) -> Result<(), ExecuteError> {
    let entry = database
        .table(&statement.table)
        .ok_or(ExecuteError::TableNotFound)?
        .clone();

    if let Some(where_clause) = &statement.where_clause {
        validate_select_expr(where_clause, &entry.schema)?;
    }

    let mut rows_to_keep = Vec::new();

    for row in read_table_rows(database, &entry)? {
        let should_delete = match &statement.where_clause {
            Some(condition) => evaluate_condition(condition, &row, &entry.schema)?,
            None => true,
        };

        if !should_delete {
            rows_to_keep.push(row);
        }
    }

    rewrite_table(database, &entry, &rows_to_keep)
}

// --- non execute command ---

fn bind_insert_row(
    schema: &TableSchema,
    requested_columns: Option<&[String]>,
    parsed_values: &[Expr],
) -> Result<Row, ExecuteError> {
    fn literal_value(expr: &Expr) -> Result<Value, ExecuteError> {
        match expr {
            Expr::Literal(LiteralValue::Integer(value)) => Ok(Value::Integer(*value)),
            Expr::Literal(LiteralValue::String(value)) => Ok(Value::Text(value.clone())),
            Expr::Literal(LiteralValue::Boolean(value)) => Ok(Value::Bool(*value)),
            Expr::Literal(LiteralValue::Null) => Ok(Value::Null),
            _ => Err(ExecuteError::UnsupportedExpression),
        }
    }

    let mut ordered = vec![Value::Null; schema.columns.len()];

    if let Some(requested_columns) = requested_columns {
        if requested_columns.len() != parsed_values.len() {
            return Err(ExecuteError::InvalidRow);
        }

        for (column_name, expr) in requested_columns.iter().zip(parsed_values) {
            let index = schema
                .columns
                .iter()
                .position(|column| column.name == *column_name)
                .ok_or(ExecuteError::ColumnNotFound)?;

            if ordered[index] != Value::Null {
                return Err(ExecuteError::InvalidRow);
            }

            ordered[index] = literal_value(expr)?;
        }
    } else {
        if parsed_values.len() != schema.columns.len() {
            return Err(ExecuteError::InvalidRow);
        }

        for (index, expr) in parsed_values.iter().enumerate() {
            ordered[index] = literal_value(expr)?;
        }
    }

    schema
        .make_row(ordered)
        .map_err(|_| ExecuteError::InvalidRow)
}

fn insert_row(database: &mut Database, entry: &TableEntry, row: &Row) -> Result<(), ExecuteError> {
    let schema = &entry.schema;

    if !schema.validate(row) {
        return Err(ExecuteError::InvalidRow);
    }

    let key = schema
        .primary_key_index
        .and_then(|index| row.values.get(index))
        .and_then(|value| match value {
            Value::Integer(value) => u32::try_from(*value).ok(),
            _ => None,
        })
        .ok_or(ExecuteError::InvalidRow)?;

    let row_bytes = row.to_bytes(schema).map_err(|_| ExecuteError::InvalidRow)?;

    let mut cursor = Cursor::find(&mut database.pager, entry.root_page_num, key);

    let page_num = cursor.page_num;
    let cell_num = cursor.cell_num;

    let page = database.pager.get_page(page_num);
    let num_cells = LeafNode::new(page.as_mut()).num_cells() as usize;

    if cell_num < num_cells {
        let existing_key = LeafNode::new(page.as_mut()).key(cell_num);
        if existing_key == key {
            return Err(ExecuteError::DuplicateKey);
        }
    }

    if num_cells >= LEAF_NODE_MAX_CELLS {
        leaf_node_split_and_insert(
            &mut database.pager,
            entry.root_page_num,
            page_num,
            cell_num,
            key,
            &row_bytes,
        );
    } else {
        let page = database.pager.get_page(page_num);
        LeafNode::new(page.as_mut()).insert(cell_num, key, &row_bytes);
    }

    Ok(())
}

fn validate_select(
    statement: &SelectStmt,
    schema: &TableSchema,
) -> Result<Vec<usize>, ExecuteError> {
    let mut projection = Vec::new();

    for item in &statement.columns {
        match item {
            SelectItem::Wildcard => {
                projection.extend(0..schema.columns.len());
            }
            SelectItem::Expr(Expr::Identifier(name))
            | SelectItem::AliasedExpr {
                expr: Expr::Identifier(name),
                ..
            } => {
                let index = schema
                    .columns
                    .iter()
                    .position(|column| column.name == *name)
                    .ok_or(ExecuteError::ColumnNotFound)?;

                projection.push(index);
            }
            _ => return Err(ExecuteError::UnsupportedExpression),
        }
    }

    if let Some(where_clause) = &statement.where_clause {
        validate_select_expr(where_clause, schema)?;
    }

    Ok(projection)
}

fn validate_select_expr(expression: &Expr, schema: &TableSchema) -> Result<(), ExecuteError> {
    match expression {
        Expr::Identifier(name) => {
            if !schema.columns.iter().any(|column| column.name == *name) {
                return Err(ExecuteError::ColumnNotFound);
            }
        }
        Expr::Literal(_) => {}
        Expr::UnaryOp { expr, .. } => {
            validate_select_expr(expr, schema)?;
        }
        Expr::BinaryOp { left, right, .. } => {
            validate_select_expr(left, schema)?;
            validate_select_expr(right, schema)?;
        }
        Expr::FunctionCall { args, .. } => {
            for argument in args {
                validate_select_expr(argument, schema)?;
            }
        }
    }

    Ok(())
}
fn select_rows(
    database: &mut Database,
    entry: &TableEntry,
    projection: &[usize],
    limit: Option<usize>,
) -> Result<(), ExecuteError> {
    let mut cursor = Cursor::table_start(&mut database.pager, entry.root_page_num);

    let mut printed = 0;

    while !cursor.end_of_table {
        if limit.is_some_and(|limit| printed >= limit) {
            break;
        }

        let row =
            Row::from_bytes(cursor.value(), &entry.schema).map_err(|_| ExecuteError::InvalidRow)?;

        let values = projection
            .iter()
            .map(|index| row.values[*index].clone())
            .collect::<Vec<_>>();

        let projected_row = Row { values };
        println!("{projected_row}");

        printed += 1;
        cursor.advance();
    }

    Ok(())
}

fn column_index(schema: &TableSchema, name: &str) -> Result<usize, ExecuteError> {
    schema
        .columns
        .iter()
        .position(|column| column.name.eq_ignore_ascii_case(name))
        .ok_or(ExecuteError::ColumnNotFound)
}

fn read_table_rows(
    database: &mut Database,
    entry: &TableEntry,
) -> Result<Vec<Row>, ExecuteError> {
    let mut rows = Vec::new();
    let mut cursor = Cursor::table_start(&mut database.pager, entry.root_page_num);

    while !cursor.end_of_table {
        let row = Row::from_bytes(cursor.value(), &entry.schema)
            .map_err(|_| ExecuteError::InvalidRow)?;
        rows.push(row);
        cursor.advance();
    }

    Ok(rows)
}
fn match_where(
    expression: &Expr,
    row: &Row,
    schema: &TableSchema,
) -> Result<bool, ExecuteError> {
    evaluate_condition(expression, row, schema)
}

fn evaluate_condition(
    expression: &Expr,
    row: &Row,
    schema: &TableSchema,
) -> Result<bool, ExecuteError> {
    match evaluate_value(expression, row, schema)? {
        Value::Bool(value) => Ok(value),
        // NULL and non-boolean values do not match a WHERE condition.
        Value::Null => Ok(false),
        _ => Err(ExecuteError::UnsupportedExpression),
    }
}

fn evaluate_value(
    expression: &Expr,
    row: &Row,
    schema: &TableSchema,
) -> Result<Value, ExecuteError> {
    match expression {
        Expr::Literal(LiteralValue::Integer(value)) => Ok(Value::Integer(*value)),
        Expr::Literal(LiteralValue::String(value)) => Ok(Value::Text(value.clone())),
        Expr::Literal(LiteralValue::Boolean(value)) => Ok(Value::Bool(*value)),
        Expr::Literal(LiteralValue::Null) => Ok(Value::Null),

        Expr::Identifier(name) => {
            let index = column_index(schema, name)?;
            Ok(row.values[index].clone())
        }

        Expr::UnaryOp { op, expr } => {
            let value = evaluate_value(expr, row, schema)?;
            match (op, value) {
                (UnaryOperator::Not, Value::Bool(value)) => Ok(Value::Bool(!value)),
                (UnaryOperator::Minus, Value::Integer(value)) => value
                    .checked_neg()
                    .map(Value::Integer)
                    .ok_or(ExecuteError::UnsupportedExpression),
                _ => Err(ExecuteError::UnsupportedExpression),
            }
        }

        Expr::BinaryOp { left, op, right } => {
            let left = evaluate_value(left, row, schema)?;
            let right = evaluate_value(right, row, schema)?;

            match op {
                BinaryOperator::And => match (left, right) {
                    (Value::Bool(left), Value::Bool(right)) => Ok(Value::Bool(left && right)),
                    _ => Err(ExecuteError::UnsupportedExpression),
                },
                BinaryOperator::Or => match (left, right) {
                    (Value::Bool(left), Value::Bool(right)) => Ok(Value::Bool(left || right)),
                    _ => Err(ExecuteError::UnsupportedExpression),
                },

                BinaryOperator::Eq => {
                    if left == Value::Null || right == Value::Null {
                        Ok(Value::Bool(false))
                    } else {
                        Ok(Value::Bool(left == right))
                    }
                }
                BinaryOperator::NotEq => {
                    if left == Value::Null || right == Value::Null {
                        Ok(Value::Bool(false))
                    } else {
                        Ok(Value::Bool(left != right))
                    }
                }

                BinaryOperator::Lt
                | BinaryOperator::Gt
                | BinaryOperator::LtEq
                | BinaryOperator::GtEq => {
                    let Some(ordering) = compare_values(&left, &right)? else {
                        return Ok(Value::Bool(false));
                    };

                    let result = match op {
                        BinaryOperator::Lt => ordering == Ordering::Less,
                        BinaryOperator::Gt => ordering == Ordering::Greater,
                        BinaryOperator::LtEq => ordering != Ordering::Greater,
                        BinaryOperator::GtEq => ordering != Ordering::Less,
                        _ => unreachable!(),
                    };

                    Ok(Value::Bool(result))
                }

                BinaryOperator::Plus
                | BinaryOperator::Minus
                | BinaryOperator::Star
                | BinaryOperator::Slash => {
                    let (Value::Integer(left), Value::Integer(right)) = (left, right) else {
                        return Err(ExecuteError::UnsupportedExpression);
                    };

                    let result = match op {
                        BinaryOperator::Plus => left.checked_add(right),
                        BinaryOperator::Minus => left.checked_sub(right),
                        BinaryOperator::Star => left.checked_mul(right),
                        BinaryOperator::Slash if right != 0 => left.checked_div(right),
                        BinaryOperator::Slash => None,
                        _ => unreachable!(),
                    }
                        .ok_or(ExecuteError::UnsupportedExpression)?;

                    Ok(Value::Integer(result))
                }
            }
        }

        Expr::FunctionCall { .. } => Err(ExecuteError::UnsupportedExpression),
    }
}

fn compare_values(
    left: &Value,
    right: &Value,
) -> Result<Option<Ordering>, ExecuteError> {
    match (left, right) {
        (Value::Null, _) | (_, Value::Null) => Ok(None),
        (Value::Integer(left), Value::Integer(right)) => Ok(Some(left.cmp(right))),
        (Value::Text(left), Value::Text(right)) => Ok(Some(left.cmp(right))),
        (Value::Bool(left), Value::Bool(right)) => Ok(Some(left.cmp(right))),
        _ => Err(ExecuteError::UnsupportedExpression),
    }
}

fn compare_for_order(left: &Value, right: &Value) -> Ordering {
    match compare_values(left, right) {
        Ok(Some(ordering)) => ordering,
        // Put NULL values after non-NULL values for ascending order.
        Ok(None) => match (left, right) {
            (Value::Null, Value::Null) => Ordering::Equal,
            (Value::Null, _) => Ordering::Greater,
            (_, Value::Null) => Ordering::Less,
            _ => Ordering::Equal,
        },
        Err(_) => Ordering::Equal,
    }
}

fn rewrite_table(
    database: &mut Database,
    entry: &TableEntry,
    rows: &[Row],
) -> Result<(), ExecuteError> {
    let primary_key_index = entry
        .schema
        .primary_key_index
        .ok_or(ExecuteError::InvalidRow)?;

    let mut seen_keys = HashSet::new();

    for row in rows {
        if !entry.schema.validate(row) {
            return Err(ExecuteError::InvalidRow);
        }

        let key = match row.values.get(primary_key_index) {
            Some(Value::Integer(value)) => {
                u32::try_from(*value).map_err(|_| ExecuteError::InvalidRow)?
            }
            _ => return Err(ExecuteError::InvalidRow),
        };

        if !seen_keys.insert(key) {
            return Err(ExecuteError::DuplicateKey);
        }
    }

    // Reset the existing root page. Old child pages become unreachable; rows
    // are reinserted using the normal B-tree insertion path.
    {
        let page = database.pager.get_page(entry.root_page_num);
        LeafNode::new(page.as_mut()).initialize();
        crate::btree::set_node_root(page.as_mut(), true);
    }

    for row in rows {
        insert_row(database, entry, row)?;
    }

    Ok(())
}