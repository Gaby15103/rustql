use crate::storage::{Row, TableSchema, Value};

#[derive(Debug)]
pub enum Statement {
    Insert(Row),
    Select,
}

pub enum PrepareError {
    SyntaxError,
    UnrecognizedStatement,
    InvalidRow(String),
}

pub fn prepare_statement(
    input: &str,
    schema: &TableSchema,
) -> Result<Statement, PrepareError> {
    if input.starts_with("insert") {
        let mut parts = input.split_whitespace();
        let _keyword = parts.next();

        let id_str = parts.next().ok_or(PrepareError::SyntaxError)?;
        let username = parts.next().ok_or(PrepareError::SyntaxError)?;
        let email = parts.next().ok_or(PrepareError::SyntaxError)?;

        let id: u32 = id_str.parse().map_err(|_| PrepareError::SyntaxError)?;

        let values = vec![
            Value::Integer(id as i64),
            Value::Text(username.to_owned()),
            Value::Text(email.to_owned()),
        ];

        let row = schema
            .make_row(values)
            .map_err(|error| PrepareError::InvalidRow(format!("{error:?}")))?;

        return Ok(Statement::Insert(row));
    }

    if input == "select" {
        return Ok(Statement::Select);
    }

    Err(PrepareError::UnrecognizedStatement)
}