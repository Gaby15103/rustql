use crate::btree::Cursor;
use crate::btree::{LEAF_NODE_MAX_CELLS, LeafNode, leaf_node_split_and_insert};
use crate::sql_parser::DataType;
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

    database.create_table(schema).map_err(|error| {
        if statement.if_not_exists
            && error.starts_with("table ")
            && error.ends_with("already exists")
        {
            return ExecuteError::InvalidRow;
        }
        ExecuteError::TableAlreadyExists
    })
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

    let projection = validate_select(&statement, &entry.schema)?;
    select_rows(
        database,
        &entry,
        &projection,
        statement.limit,
    )?;
    Ok(())
}
fn execute_update(statement: &UpdateStmt, database: &mut Database) -> Result<(), ExecuteError> {
    Ok(())
}
fn execute_delete(statement: &DeleteStmt, database: &mut Database) -> Result<(), ExecuteError> {
    Ok(())
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
