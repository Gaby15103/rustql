use crate::compiler::Statement;
use crate::storage::{Row, Table, TABLE_MAX_ROWS};

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
    if (table.num_rows >= TABLE_MAX_ROWS) {
        return Err(ExecuteError::TableFull);
    }

    let slot = table.row_slot_mut(table.num_rows);
    row.serialize(slot);
    table.num_rows += 1;
    Ok(())
}

fn execute_select(table: & Table) -> Result<(), ExecuteError>{
    for i in 0..table.num_rows{
        let slot = table.row_slot(i);
        let row = Row::deserialize(slot);
        println!("{}", row);
    }
    Ok(())
}