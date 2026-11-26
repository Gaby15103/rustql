use crate::btree::{LEAF_NODE_MAX_CELLS, LeafNode, leaf_node_split_and_insert};
use crate::sql_parser::{CreateTableStmt, DeleteStmt, InsertStmt, SelectStmt, Statement, UpdateStmt};
use crate::storage::{Database, Row, Value};

pub enum ExecuteError {
    TableFull,
    DuplicateKey,
    InvalidRow,
    StatementNotFound
}

pub fn execute_statement(statement: &Statement, db: &mut Database) -> Result<(), ExecuteError> {
    match statement {
        Statement::Select(select) => {Ok(())}
        Statement::Insert(insert) =>{Ok(())}
        Statement::CreateTable(createTable) =>{Ok(())}
        Statement::Update(update) => {Ok(())}
        Statement::Delete(delete) => {Ok(())}
        _ => {Err(ExecuteError::StatementNotFound)}
    }
}

fn primary_key(db: &Database, row: &Row) -> Option<u32> {
    let index = db.schema.primary_key_index?;
    match row.values.get(index)? {
        Value::Integer(id) => u32::try_from(*id).ok(),
        _ => None,
    }
}

fn execute_insert(row: &Row, db: &mut Database) -> Result<(), ExecuteError> {
    if !db.validate_row(row) {
        return Err(ExecuteError::InvalidRow);
    }
    let key_to_insert = match primary_key(db, row) {
        Some(key) => key,
        None => return Err(ExecuteError::InvalidRow),
    };

    let row_bytes = row
        .to_bytes(&db.schema)
        .map_err(|_| ExecuteError::InvalidRow)?;
    let schema = db.schema.clone();
    let mut cursor = Cursor::table_start(db);

    let root_page_num = cursor.table.root_page_num;
    let page_num = cursor.page_num;
    let cell_num = cursor.cell_num;

    let page = cursor.table.pager.get_page(page_num);
    let num_cells = LeafNode::new(page.as_mut()).num_cells() as usize;

    if cell_num < num_cells {
        let key_at_index = LeafNode::new(page.as_mut()).key(cell_num);
        if key_at_index == key_to_insert {
            return Err(ExecuteError::DuplicateKey);
        }
    }

    if num_cells >= LEAF_NODE_MAX_CELLS {
        leaf_node_split_and_insert(
            &mut cursor.table.pager,
            root_page_num,
            page_num,
            cell_num,
            key_to_insert,
            &row_bytes,
        );
    } else {
        let page = cursor.table.pager.get_page(page_num);
        LeafNode::new(page.as_mut()).insert(cell_num, key_to_insert, &row_bytes);
    }

    Ok(())
}

fn execute_select(db: &mut Database) -> Result<(), ExecuteError> {
    // Clone before borrowing the table through the cursor.
    let schema = db.schema.clone();
    let mut cursor = Cursor::table_start(db);

    while !cursor.end_of_table {
        let row = Row::from_bytes(cursor.value(), &schema).map_err(|_| ExecuteError::InvalidRow)?;

        println!("{row}");
        cursor.advance();
    }

    Ok(())
}
