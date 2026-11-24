use crate::storage::{COLUMN_EMAIL_SIZE, COLUMN_USERNAME_SIZE, Row};

#[derive(Debug)]
pub enum Statement {
    Insert(Row),
    Select,
}

pub enum PrepareError {
    SyntaxError,
    UnrecognizedStatement,
    StringMaxLengthLimitExceeded(String),
}

pub fn prepare_statement(input: &str) -> Result<Statement, PrepareError> {
    if input.starts_with("insert") {
        let mut parts = input.split_whitespace();
        let _keyword = parts.next();

        let id_str = parts.next().ok_or(PrepareError::SyntaxError)?;
        let username = parts.next().ok_or(PrepareError::SyntaxError)?;
        let email = parts.next().ok_or(PrepareError::SyntaxError)?;

        let id: u32 = id_str.parse().map_err(|_| PrepareError::SyntaxError)?;

        if (username.len() > COLUMN_USERNAME_SIZE) {
            let username_len = username.len().to_string();
            return Err(PrepareError::StringMaxLengthLimitExceeded(
                format!("username: {username_len} > max: {COLUMN_USERNAME_SIZE}").to_owned(),
            ));
        }
        if (email.len() > COLUMN_EMAIL_SIZE) {
            let email_len = email.len().to_string();
            return Err(PrepareError::StringMaxLengthLimitExceeded(
                format!("email: {email_len} > max: {COLUMN_EMAIL_SIZE}").to_owned(),
            ));
        }

        let row = Row::for_example_table(id, username, email);
        return Ok(Statement::Insert(row));
    }
    if input == "select" {
        return Ok(Statement::Select);
    }

    Err(PrepareError::UnrecognizedStatement)
}
