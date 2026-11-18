use crate::btree::{LeafNode, NodeType, get_node_type};
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
pub const TABLE_MAX_PAGES: usize = 100;
pub const ROWS_PER_PAGE: usize = PAGE_SIZE / ROW_SIZE;
pub const TABLE_MAX_ROWS: usize = ROWS_PER_PAGE * TABLE_MAX_PAGES;

type Page = Box<[u8; PAGE_SIZE]>;

#[derive(Debug, Clone)]
pub struct Row {
    pub id: u32,
    pub username: [u8; COLUMN_USERNAME_SIZE],
    pub email: [u8; COLUMN_EMAIL_SIZE],
}

impl Row {
    pub fn new(id: u32, username_str: &str, email_str: &str) -> Self {
        let mut username = [0u8; COLUMN_USERNAME_SIZE];
        let mut email = [0u8; COLUMN_EMAIL_SIZE];

        let u_bytes = username_str.as_bytes();
        let u_len = u_bytes.len().min(COLUMN_USERNAME_SIZE);
        username[..u_len].copy_from_slice(&u_bytes[..u_len]);

        let e_bytes = email_str.as_bytes();
        let e_len = e_bytes.len().min(COLUMN_EMAIL_SIZE);
        email[..e_len].copy_from_slice(&e_bytes[..e_len]);

        Self {
            id,
            username,
            email,
        }
    }

    pub fn serialize(&self, destination: &mut [u8]) {
        destination[ID_OFFSET..ID_OFFSET + ID_SIZE].copy_from_slice(&self.id.to_ne_bytes());
        destination[USERNAME_OFFSET..USERNAME_OFFSET + USERNAME_SIZE]
            .copy_from_slice(&self.username);
        destination[EMAIL_OFFSET..EMAIL_OFFSET + EMAIL_SIZE].copy_from_slice(&self.email);
    }

    pub fn deserialize(source: &[u8]) -> Self {
        let id_bytes: [u8; 4] = source[ID_OFFSET..ID_OFFSET + ID_SIZE]
            .try_into()
            .expect("Slice lenght mismatch for ID");
        let id = u32::from_ne_bytes(id_bytes);

        let mut username = [0u8; COLUMN_USERNAME_SIZE];
        username.copy_from_slice(&source[USERNAME_OFFSET..USERNAME_OFFSET + USERNAME_SIZE]);

        let mut email = [0u8; COLUMN_EMAIL_SIZE];
        email.copy_from_slice(&source[EMAIL_OFFSET..EMAIL_OFFSET + EMAIL_SIZE]);

        Self {
            id,
            username,
            email,
        }
    }
}

impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let username = std::str::from_utf8(&self.username)
            .unwrap_or("")
            .trim_end_matches('\0');
        let email = std::str::from_utf8(&self.email)
            .unwrap_or("")
            .trim_end_matches('\0');

        write!(f, "{},{},{}", self.id, username, email)
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
        let root_page_num = self.root_page_num;
        let root_page = self.pager.get_page(root_page_num);

        match get_node_type(root_page.as_ref()) {
            NodeType::Leaf => {
                let cell_num = LeafNode::new(root_page.as_mut()).find_cell(key);
                let num_cells = LeafNode::new(root_page.as_mut()).num_cells() as usize;

                Cursor {
                    table: self,
                    page_num: root_page_num,
                    cell_num,
                    end_of_table: cell_num >= num_cells,
                }
            }
            NodeType::Internal => {
                panic!("Need to implement searching an internal node");
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
        let root_page_num = table.root_page_num;
        let root_page = table.pager.get_page(root_page_num);
        let num_cells = LeafNode::new(root_page.as_mut()).num_cells() as usize;

        Self {
            table,
            page_num: root_page_num,
            cell_num: 0,
            end_of_table: num_cells == 0,
        }
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
        let num_cells = LeafNode::new(page.as_mut()).num_cells() as usize;

        self.cell_num += 1;
        if self.cell_num >= num_cells {
            self.end_of_table = true;
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
