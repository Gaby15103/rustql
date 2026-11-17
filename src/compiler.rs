use crate::storage::Row;

#[derive(Debug)]
pub enum Statement {
    Insert(Row),
    Select,
}

pub enum PrepareError {
    SyntaxError,
    UnrecognizedStatement,
}

pub fn prepare_statement(input: &str) -> Result<Statement, PrepareError> {
    if input.starts_with("insert") {
        let mut parts = input.split_whitespace();
        let _keyword = parts.next();

        let id_str = parts.next().ok_or(PrepareError::SyntaxError)?;
        let username = parts.next().ok_or(PrepareError::SyntaxError)?;
        let email = parts.next().ok_or(PrepareError::SyntaxError)?;

        let id: u32 = id_str.parse().map_err(|_| PrepareError::SyntaxError)?;

        let row = Row::new(id, username, email);
        return Ok(Statement::Insert(row))
    }
    if input == "select" {
        return Ok(Statement::Select);
    }

    Err(PrepareError::UnrecognizedStatement)
}
