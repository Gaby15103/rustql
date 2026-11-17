use std::fmt::{self, write};
use std::fs::File;

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
        destination[ID_OFFSET..ID_OFFSET + ID_SIZE]
            .copy_from_slice(&self.id.to_ne_bytes());
        destination[USERNAME_OFFSET..USERNAME_OFFSET + USERNAME_SIZE]
            .copy_from_slice(&self.username);
        destination[EMAIL_OFFSET..EMAIL_OFFSET + EMAIL_SIZE]
            .copy_from_slice(&self.email);
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

pub struct Pager {
    file: File,
    file_length: usize,
    pages: Vec<Option<Page>>,
}

pub struct Table {
    pub num_rows: usize,
    pager: Pager,
}

impl Table {
    pub fn db_open(file_path: &str)->Self{
        Self {
            num_rows: 0,
            pager: Pager::open(file_path),
        }
    }


    pub fn row_slot_mut(&mut self, row_num: usize) -> &mut [u8] {
        let page_num = row_num / ROWS_PER_PAGE;
        // allocate the 4KB pages on the heap only iif it is currently 'None'
        let page = self.pager.pages[page_num].get_or_insert_with(|| Box::new([0u8; PAGE_SIZE]));
        let row_offset = row_num % ROWS_PER_PAGE;
        let byte_offset = row_offset * ROW_SIZE;

        &mut page[byte_offset..byte_offset + ROW_SIZE]
    }

    pub fn row_slot(&self, row_num: usize) -> &[u8] {
        let page_num = row_num / ROWS_PER_PAGE;
        let page = self.pager.pages[page_num]
            .as_ref()
            .expect("Attempted to read from unallocated page");
        let page = self.pager::get_page(page_num);

        let row_offset = row_num % ROWS_PER_PAGE;
        let byte_offset = row_offset * ROW_SIZE;

        &page[byte_offset..byte_offset + ROW_SIZE]
    }
}

impl Pager {
    pub fn open(file_path: &str)->Self{
        let mut file = File::open(file_path).unwrap();
        let file_length = file.metadata().unwrap().len();

        return Self{
            file,
            file_length: file_length as usize,
            pages: vec![],
        }
    }
    pub fn get_page(&self, page_num: u32){

    }
}