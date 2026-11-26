use crate::storage::{Row, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowError {
    WrongColumnCount { expected: usize, actual: usize },
    TypeMismatch { column: String },
    TextTooLong { column: String, max_length: usize },
    UnsupportedNull { column: String },
    IntegerOutOfRange { column: String },
    InvalidRecordSize { expected: usize, actual: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Integer,
    Text,
    Bool,
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub value_type: ValueType,
    pub max_length: Option<usize>,
}

impl Column {
    pub(crate) fn encoded_width(&self) -> Result<usize, RowError> {
        match self.value_type {
            ValueType::Integer => Ok(4),
            ValueType::Text => self.max_length.ok_or_else(|| RowError::TypeMismatch {
                column: self.name.clone(),
            }),
            ValueType::Bool => Ok(1),
            ValueType::Null => Err(RowError::UnsupportedNull {
                column: self.name.clone(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<Column>,
    pub primary_key_index: Option<usize>,
}

impl TableSchema {
    pub fn make_row(&self, values: Vec<Value>) -> Result<Row, RowError> {
        if values.len() != self.columns.len() {
            return Err(RowError::WrongColumnCount {
                expected: self.columns.len(),
                actual: values.len(),
            });
        }

        for (column, value) in self.columns.iter().zip(&values) {
            match (&column.value_type, value) {
                (ValueType::Integer, Value::Integer(_)) => {}
                (ValueType::Text, Value::Text(text)) => {
                    if let Some(max_length) = column.max_length {
                        if text.len() > max_length {
                            return Err(RowError::TextTooLong {
                                column: column.name.clone(),
                                max_length,
                            });
                        }
                    }
                }
                (ValueType::Bool, Value::Bool(_)) => {}
                (ValueType::Null, Value::Null) => {}
                _ => {
                    return Err(RowError::TypeMismatch {
                        column: column.name.clone(),
                    });
                }
            }
        }

        Ok(Row { values })
    }
    pub(crate) fn record_size(&self) -> Result<usize, RowError> {
        self.columns
            .iter()
            .try_fold(0usize, |total, column| Ok(total + column.encoded_width()?))
    }

    pub fn validate(&self, row: &Row) -> bool {
        self.make_row(row.values.clone()).is_ok()
    }
}