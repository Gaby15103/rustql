use crate::storage::{
    Catalog, Column, TableEntry, TableSchema, ValueType, PAGE_SIZE,
};

const MAGIC: &[u8; 8] = b"RUSTQL01";
const VERSION: u16 = 1;
const NO_PRIMARY_KEY: u16 = u16::MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogFormatError {
    pub message: String,
}

impl CatalogFormatError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub fn encode(catalog: &Catalog) -> Result<[u8; PAGE_SIZE], CatalogFormatError> {
    let mut bytes = Vec::with_capacity(PAGE_SIZE);

    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());

    let names = catalog.names();

    let count =
        u32::try_from(names.len()).map_err(|_| CatalogFormatError::new("too many tables"))?;
    bytes.extend_from_slice(&count.to_le_bytes());

    for name in names {
        let entry = catalog
            .get(&name)
            .ok_or_else(|| CatalogFormatError::new("missing catalog entry"))?;

        write_string(&mut bytes, &entry.schema.name, &format!("table name '{name}'"))?;
        write_u64(&mut bytes, entry.root_page_num as u64)?;

        let column_count = u16::try_from(entry.schema.columns.len())
            .map_err(|_| CatalogFormatError::new("too many columns"))?;
        write_u16(&mut bytes, column_count)?;

        for column in &entry.schema.columns {
            write_string(&mut bytes, &column.name, "column name")?;
            write_u8(&mut bytes, value_type_id(column.value_type))?;

            if let Some(max_length) = column.max_length {
                write_u32(
                    &mut bytes,
                    u32::try_from(max_length)
                        .map_err(|_| CatalogFormatError::new("invalid max length"))?,
                )?;
            } else {
                write_u32(&mut bytes, 0)?;
            }

            let flags = u8::from(column.nullable) | (u8::from(column.unique) << 1);
            write_u8(&mut bytes, flags)?;
        }

        let primary_key = entry
            .schema
            .primary_key_index
            .and_then(|index| u16::try_from(index).ok())
            .unwrap_or(NO_PRIMARY_KEY);
        write_u16(&mut bytes, primary_key);
    }

    if bytes.len() > PAGE_SIZE {
        return Err(CatalogFormatError::new(format!(
            "catalog requires {} bytes but page 0 is only {PAGE_SIZE} bytes",
            bytes.len()
        )));
    }

    let mut page = [0u8; PAGE_SIZE];
    page[..bytes.len()].copy_from_slice(&bytes);
    Ok(page)
}

pub fn decode(page: &[u8; PAGE_SIZE]) -> Result<Catalog, CatalogFormatError> {
    let mut reader = Reader::new(page);

    reader.expect_bytes(MAGIC, "invalid database format")?;

    let version = reader.read_u16()?;
    if version != VERSION {
        return Err(CatalogFormatError::new(format!(
            "unsupported catalog format version {version}"
        )));
    }

    let table_count = reader.read_u32()? as usize;
    let catalog = Catalog::new();

    decode_tables(reader, catalog, table_count)
}

fn decode_tables(
    mut reader: Reader<'_>,
    mut catalog: Catalog,
    table_count: usize,
) -> Result<Catalog, CatalogFormatError> {
    for _ in 0..table_count {
        let table_name = reader.read_string("table name")?;
        let root_page_num = reader.read_u64()? as usize;

        let column_count = reader.read_u16()? as usize;
        let mut columns = Vec::with_capacity(column_count);

        for _ in 0..column_count {
            let column_name = reader.read_string("column name")?;
            let type_id = reader.read_u8()?;
            let max_length = reader.read_u32()? as usize;
            let flags = reader.read_u8()?;

            if flags & !0b11 != 0 {
                return Err(CatalogFormatError::new(
                    "invalid stored column constraint flags",
                ));
            }

            let value_type = match type_id {
                0 => ValueType::Integer,
                1 => ValueType::Text,
                2 => ValueType::Bool,
                3 => ValueType::Null,
                _ => {
                    return Err(CatalogFormatError::new(
                        "invalid stored column value type",
                    ));
                }
            };

            columns.push(Column {
                name: column_name,
                value_type,
                max_length: if value_type == ValueType::Text {
                    if max_length == 0 {
                        None
                    } else {
                        Some(max_length)
                    }
                } else {
                    None
                },
                nullable: flags & 0b01 != 0,
                unique: flags & 0b10 != 0,
            });
        }

        let primary_key = reader.read_u16()?;
        let primary_key_index = if primary_key == NO_PRIMARY_KEY {
            None
        } else {
            Some(primary_key as usize)
        };

        if let Some(index) = primary_key_index {
            if index >= columns.len() {
                return Err(CatalogFormatError::new(
                    "invalid stored primary-key column index",
                ));
            }
        }

        let name = table_name.to_ascii_lowercase();
        let schema = TableSchema {
            name: name.clone(),
            columns,
            primary_key_index,
        };

        catalog
            .insert(
                name,
                TableEntry {
                    root_page_num,
                    schema,
                },
            )
            .map_err(|error| CatalogFormatError::new(error))?;
    }

    Ok(catalog)
}

fn value_type_id(value_type: ValueType) -> u8 {
    match value_type {
        ValueType::Integer => 0,
        ValueType::Text => 1,
        ValueType::Bool => 2,
        ValueType::Null => 3,
    }
}

fn write_u8(bytes: &mut Vec<u8>, value: u8) -> Result<(), CatalogFormatError> {
    bytes.push(value);
    Ok(())
}

fn write_u16(bytes: &mut Vec<u8>, value: u16) -> Result<(), CatalogFormatError> {
    bytes.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_u32(bytes: &mut Vec<u8>, value: u32) -> Result<(), CatalogFormatError> {
    bytes.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_u64(bytes: &mut Vec<u8>, value: u64) -> Result<(), CatalogFormatError> {
    bytes.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_string(
    bytes: &mut Vec<u8>,
    value: &str,
    description: &str,
) -> Result<(), CatalogFormatError> {
    let length =
        u16::try_from(value.len()).map_err(|_| CatalogFormatError::new(format!(
            "{description} is too long"
        )))?;

    write_u16(bytes, length)?;
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            position: 0,
        }
    }

    fn expect_bytes(
        &mut self,
        expected: &[u8],
        error: &str,
    ) -> Result<(), CatalogFormatError> {
        let actual = self.read_bytes(expected.len())?;
        if actual != expected {
            return Err(CatalogFormatError::new(error));
        }
        Ok(())
    }

    fn read_u8(&mut self) -> Result<u8, CatalogFormatError> {
        Ok(self.read_bytes(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, CatalogFormatError> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, CatalogFormatError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&mut self) -> Result<u64, CatalogFormatError> {
        let bytes = self.read_bytes(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_string(
        &mut self,
        description: &str,
    ) -> Result<String, CatalogFormatError> {
        let length = self.read_u16()? as usize;
        let bytes = self.read_bytes(length)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| {
            CatalogFormatError::new(format!("invalid UTF-8 for {description}"))
        })
    }

    fn read_bytes(
        &mut self,
        length: usize,
    ) -> Result<&'a [u8], CatalogFormatError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| CatalogFormatError::new("catalog length overflow"))?;

        if end > self.bytes.len() {
            return Err(CatalogFormatError::new("unexpected end of catalog"));
        }

        let bytes = &self.bytes[self.position..end];
        self.position = end;
        Ok(bytes)
    }
}