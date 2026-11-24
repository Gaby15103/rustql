use crate::btree::{InternalNode, LeafNode, NodeType, get_node_type};
use std::fmt::{self, write};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

pub const PAGE_SIZE: usize = 4096;
pub const TABLE_MAX_PAGES: usize = 400;
// Temporary: the fixed-size leaf-cell format still requires this.
pub const ROW_SIZE: usize = 4 + 32 + 255;

type Page = Box<[u8; PAGE_SIZE]>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Integer(i64),
    Text(String),
    Null,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Integer,
    Text,
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowError {
    WrongColumnCount { expected: usize, actual: usize },
    TypeMismatch { column: String },
    TextTooLong { column: String, max_length: usize },
    UnsupportedNull { column: String },
    IntegerOutOfRange { column: String },
    InvalidRecordSize { expected: usize, actual: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub values: Vec<Value>,
}

impl Row {
    // pub fn new(id: u32, username_str: &str, email_str: &str) -> Self {
    //     let mut username = [0u8; COLUMN_USERNAME_SIZE];
    //     let mut email = [0u8; COLUMN_EMAIL_SIZE];
    //
    //     let u_bytes = username_str.as_bytes();
    //     let u_len = u_bytes.len().min(COLUMN_USERNAME_SIZE);
    //     username[..u_len].copy_from_slice(&u_bytes[..u_len]);
    //
    //     let e_bytes = email_str.as_bytes();
    //     let e_len = e_bytes.len().min(COLUMN_EMAIL_SIZE);
    //     email[..e_len].copy_from_slice(&e_bytes[..e_len]);
    //
    //     Self {
    //         id,
    //         username,
    //         email,
    //     }
    // }

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

pub struct Table {
    pub root_page_num: usize,
    pub pager: Pager,
    pub schema: TableSchema,
}

impl Table {
    pub fn db_open(file_path: &str) -> Self {
        let mut pager = Pager::open(file_path);
        let root_page_num = 0;

        if pager.num_pages == 0 {
            let root_page = pager.get_page(0);
            LeafNode::new(root_page.as_mut()).initialize();
            crate::btree::set_node_root(root_page.as_mut(), true);
        }

        Self {
            root_page_num,
            pager,
            schema: example_table_schema(),
        }
    }

    pub fn db_close(&mut self) {
        for i in 0..self.pager.num_pages {
            if self.pager.pages[i].is_some() {
                self.pager.flush(i, PAGE_SIZE);
                self.pager.pages[i] = None;
            }
        }
    }

    pub fn validate_row(&self, row: &Row) -> bool {
        self.schema.validate(row)
    }

    pub fn find(&mut self, key: u32) -> Cursor<'_> {
        let mut page_num = self.root_page_num;

        loop {
            let page = self.pager.get_page(page_num);
            match get_node_type(page.as_ref()) {
                NodeType::Internal => {
                    let internal = InternalNode::new(page.as_mut());
                    let child_index = internal.find_child(key);
                    page_num = internal.child(child_index) as usize;
                }
                NodeType::Leaf => {
                    let leaf = LeafNode::new(page.as_mut());
                    let cell_num = leaf.find_cell(key);
                    let num_cells = leaf.num_cells() as usize;

                    return Cursor {
                        table: self,
                        page_num,
                        cell_num,
                        end_of_table: cell_num >= num_cells,
                    };
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub value_type: ValueType,
    pub max_length: Option<usize>,
}

impl Column {
    fn encoded_width(&self) -> Result<usize, RowError> {
        match self.value_type {
            ValueType::Integer => Ok(4),
            ValueType::Text => self.max_length.ok_or_else(|| {
                RowError::TypeMismatch {
                    column: self.name.clone(),
                }
            }),
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

pub fn example_table_schema() -> TableSchema {
    TableSchema {
        name: "people".to_owned(),
        columns: vec![
            Column {
                name: "id".to_owned(),
                value_type: ValueType::Integer,
                max_length: None,
            },
            Column {
                name: "username".to_owned(),
                value_type: ValueType::Text,
                max_length: Some(32),
            },
            Column {
                name: "email".to_owned(),
                value_type: ValueType::Text,
                max_length: Some(255),
            },
        ],
        primary_key_index: Some(0),
    }
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
    fn record_size(&self) -> Result<usize, RowError> {
        self.columns.iter().try_fold(0usize, |total, column| {
            Ok(total + column.encoded_width()?)
        })
    }

    fn validate(&self, row: &Row) -> bool {
        self.make_row(row.values.clone()).is_ok()
    }
}

pub struct Cursor<'a> {
    pub table: &'a mut Table,
    pub page_num: usize,
    pub cell_num: usize,
    pub end_of_table: bool,
}

impl<'a> Cursor<'a> {
    pub fn table_start(table: &'a mut Table) -> Self {
        let mut cursor = table.find(0);

        let page = cursor.table.pager.get_page(cursor.page_num);
        let num_cells = LeafNode::new(page.as_mut()).num_cells() as usize;
        cursor.end_of_table = num_cells == 0;

        cursor
    }

    pub fn value(&mut self) -> &mut [u8] {
        let page = self.table.pager.get_page(self.page_num);
        let offset = crate::btree::LEAF_NODE_HEADER_SIZE
            + self.cell_num * crate::btree::LEAF_NODE_CELL_SIZE
            + crate::btree::LEAF_NODE_VALUE_OFFSET;
        &mut page[offset..offset + crate::btree::LEAF_NODE_VALUE_SIZE]
    }

    pub fn advance(&mut self) {
        let page = self.table.pager.get_page(self.page_num);
        let leaf = LeafNode::new(page.as_mut());
        let num_cells = leaf.num_cells() as usize;
        let next_leaf = leaf.next_leaf() as usize;

        self.cell_num += 1;
        if self.cell_num >= num_cells {
            if next_leaf == 0 {
                self.end_of_table = true;
            } else {
                self.page_num = next_leaf;
                self.cell_num = 0;
            }
        }
    }
}

pub struct Pager {
    pub file: File,
    pub file_length: usize,
    pub num_pages: usize,
    pub pages: Vec<Option<Page>>,
}

impl Pager {
    pub fn open(file_path: &str) -> Self {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(file_path)
            .expect("Unable to open database file");

        let file_length = file.metadata().expect("Unable to read file metadata").len() as usize;
        let num_pages = file_length / PAGE_SIZE;

        if file_length % PAGE_SIZE != 0 {
            panic!("Db file is not a whole number of pages. Corrupt file.");
        }

        Self {
            file,
            file_length,
            num_pages,
            pages: vec![None; TABLE_MAX_PAGES],
        }
    }
    pub fn get_page(&mut self, page_num: usize) -> &mut Page {
        if page_num >= TABLE_MAX_PAGES {
            panic!("Tried to fetch page number out of bounds: {}", page_num);
        }

        if self.pages[page_num].is_none() {
            let mut page = Box::new([0u8; PAGE_SIZE]);
            let mut num_pages = self.file_length / PAGE_SIZE;

            if self.file_length % PAGE_SIZE != 0 {
                num_pages += 1;
            }

            if page_num < num_pages {
                let offset = (page_num * PAGE_SIZE) as u64;
                self.file
                    .seek(SeekFrom::Start(offset))
                    .expect("Error seeking file");

                let mut bytes_read = 0;
                while bytes_read < PAGE_SIZE {
                    let n = self
                        .file
                        .read(&mut page[bytes_read..])
                        .expect("Error reading file");
                    if n == 0 {
                        break;
                    }
                    bytes_read += n;
                }
            }

            self.pages[page_num] = Some(page);

            if page_num >= self.num_pages {
                self.num_pages = page_num + 1;
            }
        }

        self.pages[page_num].as_mut().unwrap()
    }

    pub fn flush(&mut self, page_num: usize, size: usize) {
        let page = self.pages[page_num]
            .as_ref()
            .expect("Tried to flush null page");

        let offset = (page_num * PAGE_SIZE) as u64;
        self.file
            .seek(SeekFrom::Start(offset))
            .expect("Error seeking file before flush");

        self.file
            .write_all(&page[..size])
            .expect("Error writing page to file");
    }

    pub fn get_unused_page_num(&mut self) -> usize {
        let page_num = self.num_pages;
        self.num_pages += 1;
        page_num
    }

    pub fn get_two_pages_mut(&mut self, idx1: usize, idx2: usize) -> (&mut Page, &mut Page) {
        assert_ne!(idx1, idx2, "Cannot borrow the same page mutably twice");
        if idx1 < idx2 {
            let (left, right) = self.pages.split_at_mut(idx2);
            (left[idx1].as_mut().unwrap(), right[0].as_mut().unwrap())
        } else {
            let (left, right) = self.pages.split_at_mut(idx1);
            (right[0].as_mut().unwrap(), left[idx2].as_mut().unwrap())
        }
    }
}
