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
}