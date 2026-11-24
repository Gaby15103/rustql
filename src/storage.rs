use crate::btree::{get_node_type, InternalNode, LeafNode, NodeType};
use std::fmt::{self, write};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

pub const COLUMN_USERNAME_SIZE: usize = 32;
pub const COLUMN_EMAIL_SIZE: usize = 255;

pub const ID_SIZE: usize = size_of::<u32>();
pub const USERNAME_SIZE: usize = COLUMN_USERNAME_SIZE;
pub const EMAIL_SIZE: usize = COLUMN_EMAIL_SIZE;

pub const ID_OFFSET: usize = 0;
pub const USERNAME_OFFSET: usize = ID_OFFSET + ID_SIZE;
pub const EMAIL_OFFSET: usize = USERNAME_OFFSET + USERNAME_SIZE;
pub const ROW_SIZE: usize = ID_SIZE + USERNAME_SIZE + EMAIL_SIZE;

pub const PAGE_SIZE: usize = 4096;
pub const TABLE_MAX_PAGES: usize = 400;
pub const ROWS_PER_PAGE: usize = PAGE_SIZE / ROW_SIZE;
pub const TABLE_MAX_ROWS: usize = ROWS_PER_PAGE * TABLE_MAX_PAGES;

type Page = Box<[u8; PAGE_SIZE]>;

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

    pub fn to_bytes(&self) -> [u8; ROW_SIZE] {
        let mut bytes = [0u8; ROW_SIZE];
        self.serialize(&mut bytes);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        assert_eq!(bytes.len(), ROW_SIZE);
        Self::deserialize(bytes)
    }

    pub fn serialize(&self, destination: &mut [u8]) {
        let [Value::Integer(id), Value::Text(username), Value::Text(email)] =
            self.values.as_slice()
        else {
            panic!("expected row values [Integer, Text, Text]");
        };

        let id: u32 = (*id).try_into().expect("id must fit in u32");
        destination[ID_OFFSET..ID_OFFSET + ID_SIZE].copy_from_slice(&id.to_ne_bytes());

        let mut username_bytes = [0; USERNAME_SIZE];
        let username_len = username.len().min(USERNAME_SIZE);
        username_bytes[..username_len].copy_from_slice(&username.as_bytes()[..username_len]);
        destination[USERNAME_OFFSET..USERNAME_OFFSET + USERNAME_SIZE]
            .copy_from_slice(&username_bytes);

        let mut email_bytes = [0; EMAIL_SIZE];
        let email_len = email.len().min(EMAIL_SIZE);
        email_bytes[..email_len].copy_from_slice(&email.as_bytes()[..email_len]);
        destination[EMAIL_OFFSET..EMAIL_OFFSET + EMAIL_SIZE]
            .copy_from_slice(&email_bytes);
    }

    pub fn deserialize(source: &[u8]) -> Self {
        let id_bytes: [u8; ID_SIZE] = source[ID_OFFSET..ID_OFFSET + ID_SIZE]
            .try_into()
            .expect("invalid ID bytes");
        let id = u32::from_ne_bytes(id_bytes);

        let username = read_fixed_text(
            &source[USERNAME_OFFSET..USERNAME_OFFSET + USERNAME_SIZE],
        );
        let email = read_fixed_text(
            &source[EMAIL_OFFSET..EMAIL_OFFSET + EMAIL_SIZE],
        );

        Self {
            values: vec![
                Value::Integer(id as i64),
                Value::Text(username),
                Value::Text(email),
            ],
        }
    }
}
fn read_fixed_text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len());
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

    pub fn find(&mut self, key: u32) -> Cursor<'_> {
        let mut page_num = self.root_page_num;

        loop {
            let page = self.pager.get_page(page_num);
            match get_node_type(page.as_ref()){
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
