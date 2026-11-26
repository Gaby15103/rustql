use crate::storage::{Catalog, Pager, PAGE_SIZE};

pub struct Database {
    pub pager: Pager,
    pub catalog: Catalog,
}

impl Database {
    pub fn open(file_path: &str) -> Self {
        Self {
            pager: Pager::open(file_path),
            catalog: Catalog::new(),
        }
    }
    pub fn close(&mut self){
        for i in 0..self.pager.num_pages {
            if self.pager.pages[i].is_some() {
                self.pager.flush(i, PAGE_SIZE);
                self.pager.pages[i] = None;
            }
        }
    }
}