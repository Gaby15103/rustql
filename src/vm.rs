use crate::btree::{LeafNode, LEAF_NODE_MAX_CELLS};
use crate::compiler::Statement;
use crate::storage::{Row, Table, TABLE_MAX_ROWS, Cursor};

pub enum ExecuteError{
    TableFull,
}

pub fn execute_statement(statement: &Statement, table: &mut Table) -> Result<(),ExecuteError> {
    match statement {
        Statement::Insert(row) => execute_insert(row, table),
        Statement::Select => execute_select(table),
    }
}

fn execute_insert(row: &Row, table: &mut Table) -> Result<(),ExecuteError>{
    let page = table.pager.get_page(table.root_page_num);
    let num_cells = LeafNode::new(page.as_mut()).num_cells() as usize;

    if num_cells >= LEAF_NODE_MAX_CELLS{
        return Err(ExecuteError::TableFull);
    }

    let mut cursor = Cursor::table_end(table);
    let page = cursor.table.pager.get_page(cursor.page_num);
    let mut node = LeafNode::new(page.as_mut());
    node.insert(cursor.cell_num, row.id, row);

    Ok(())
}

fn execute_select(table: &mut Table) -> Result<(), ExecuteError>{
    let mut cursor = Cursor::table_start(table);

    while !cursor.end_of_table {
        let row = Row::deserialize(cursor.value());
        println!("{}", row);
        cursor.advance();
    }

    Ok(())
}