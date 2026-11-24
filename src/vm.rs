use crate::btree::{LEAF_NODE_MAX_CELLS, LeafNode, leaf_node_split_and_insert};
use crate::compiler::Statement;
use crate::storage::{Cursor, Row, TABLE_MAX_ROWS, Table, Value};

pub enum ExecuteError {
    TableFull,
    DuplicateKey,
    InvalidRow,
}

pub fn execute_statement(statement: &Statement, table: &mut Table) -> Result<(), ExecuteError> {
    match statement {
        Statement::Insert(row) => execute_insert(row, table),
        Statement::Select => execute_select(table),
    }
}

fn primary_key(table: &Table, row: &Row) -> Option<u32> {
    let index = table.schema.primary_key_index?;
    match row.values.get(index)? {
        Value::Integer(id) => u32::try_from(*id).ok(),
        _ => None,
    }
}

fn execute_insert(row: &Row, table: &mut Table) -> Result<(), ExecuteError> {
    if !table.validate_row(row) {
        return Err(ExecuteError::InvalidRow);
    }
    let key_to_insert = match primary_key(table, row) {
        Some(key) => key,
        None => return Err(ExecuteError::InvalidRow),
    };

    let row_bytes = row.to_bytes();
    let cursor = table.find(key_to_insert);

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

fn execute_select(table: &mut Table) -> Result<(), ExecuteError> {
    let mut cursor = Cursor::table_start(table);

    while !cursor.end_of_table {
        let row = Row::from_bytes(cursor.value());
        println!("{}", row);
        cursor.advance();
    }

    Ok(())
}
