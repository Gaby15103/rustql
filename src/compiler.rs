#[derive(Debug)]
pub enum Statement {
    Insert,
    Select,
}

pub enum PrepareError {
    UnrecognizedStatement,
}

pub fn prepare_statement(input: &str) -> Result<Statement, PrepareError> {
    if input.starts_with("insert") {
        return Ok(Statement::Insert);
    }
    if input == "select" {
        return Ok(Statement::Select);
    }

    Err(PrepareError::UnrecognizedStatement)
}
