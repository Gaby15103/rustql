use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

pub const PAGE_SIZE: usize = 4096;
pub const TABLE_MAX_PAGES: usize = 400;

type Page = Box<[u8; PAGE_SIZE]>;

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

