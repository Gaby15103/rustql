use std::fmt;
use crate::storage::{RowError, TableSchema, ValueType};

pub const ROW_SIZE: usize = 4 + 32 + 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Integer(i64),
    Text(String),
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub values: Vec<Value>,
}

impl Row {
    pub fn for_example_table(id: u32, username: &str, email: &str) -> Self {
        Self {
            values: vec![
                Value::Integer(id as i64),
                Value::Text(username.to_owned()),
                Value::Text(email.to_owned()),
            ],
        }
    }

    pub fn to_bytes(&self, schema: &TableSchema) -> Result<Vec<u8>, RowError> {
        let mut bytes = vec![0; ROW_SIZE];
        self.serialize(schema, &mut bytes)?;
        Ok(bytes)
    }

    pub fn from_bytes(bytes: &[u8], schema: &TableSchema) -> Result<Self, RowError> {
        if bytes.len() != ROW_SIZE {
            return Err(RowError::InvalidRecordSize {
                expected: ROW_SIZE,
                actual: bytes.len(),
            });
        }

        Self::deserialize(schema, bytes)
    }

    fn serialize(&self, schema: &TableSchema, destination: &mut [u8]) -> Result<(), RowError> {
        if !schema.validate(self) {
            return Err(RowError::TypeMismatch {
                column: "row".to_owned(),
            });
        }

        let expected_size = schema.record_size()?;
        if destination.len() != expected_size || expected_size != ROW_SIZE {
            return Err(RowError::InvalidRecordSize {
                expected: ROW_SIZE,
                actual: expected_size,
            });
        }

        destination.fill(0);
        let mut offset = 0;

        for (column, value) in schema.columns.iter().zip(&self.values) {
            let width = column.encoded_width()?;

            match value {
                Value::Integer(value) => {
                    let value = u32::try_from(*value).map_err(|_| RowError::IntegerOutOfRange {
                        column: column.name.clone(),
                    })?;

                    destination[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
                }
                Value::Text(text) => {
                    copy_fixed_text(&mut destination[offset..offset + width], text);
                }
                Value::Null => {
                    return Err(RowError::UnsupportedNull {
                        column: column.name.clone(),
                    });
                }
            }

            offset += width;
        }

        Ok(())
    }

    fn deserialize(schema: &TableSchema, source: &[u8]) -> Result<Self, RowError> {
        let expected_size = schema.record_size()?;
        if source.len() != expected_size || expected_size != ROW_SIZE {
            return Err(RowError::InvalidRecordSize {
                expected: ROW_SIZE,
                actual: source.len(),
            });
        }

        let mut values = Vec::with_capacity(schema.columns.len());
        let mut offset = 0;

        for column in &schema.columns {
            let width = column.encoded_width()?;
            let field = &source[offset..offset + width];

            let value = match column.value_type {
                ValueType::Integer => {
                    let int_bytes: [u8; 4] =
                        field.try_into().map_err(|_| RowError::InvalidRecordSize {
                            expected: 4,
                            actual: field.len(),
                        })?;

                    Value::Integer(u32::from_ne_bytes(int_bytes) as i64)
                }
                ValueType::Text => Value::Text(read_fixed_text(field)),
                ValueType::Null => {
                    return Err(RowError::UnsupportedNull {
                        column: column.name.clone(),
                    });
                }
            };

            values.push(value);
            offset += width;
        }

        schema.make_row(values)
    }
}
impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, value) in self.values.iter().enumerate() {
            if index > 0 {
                f.write_str(",")?;
            }

            match value {
                Value::Integer(value) => write!(f, "{value}")?,
                Value::Text(value) => f.write_str(value)?,
                Value::Null => f.write_str("NULL")?,
            }
        }

        Ok(())
    }
}

fn copy_fixed_text(destination: &mut [u8], text: &str) {
    destination.fill(0);

    let bytes = text.as_bytes();
    let length = bytes.len().min(destination.len());
    destination[..length].copy_from_slice(&bytes[..length]);
}
fn read_fixed_text(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}