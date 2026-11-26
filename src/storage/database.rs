use crate::btree::{LeafNode, set_node_root};
use crate::storage::{Catalog, PAGE_SIZE, Pager, TableEntry, TableSchema};

pub struct Database {
    pub pager: Pager,
    pub catalog: Catalog,
}

impl Database {
    pub fn open(file_path: &str) -> Self {
        let mut pager = Pager::open(file_path);

        if pager.num_pages == 0 {
            pager.get_page(0);
        }

        Self {
            pager,
            catalog: Catalog::new(),
        }
    }
    pub fn close(&mut self) {
        for i in 0..self.pager.num_pages {
            if self.pager.pages[i].is_some() {
                self.pager.flush(i, PAGE_SIZE);
                self.pager.pages[i] = None;
            }
        }
    }
    pub fn table(&self, name: &str) -> Option<&TableEntry> {
        self.catalog.get(&name.to_ascii_lowercase())
    }
    pub fn create_table(&mut self, schema: TableSchema) -> Result<(), String> {
        let name = schema.name.to_ascii_lowercase();

        if self.catalog.get(&name).is_some() {
            return Err(format!("Table {name} already exists"));
        }

        let record_size = schema.record_size().map_err(|error| format!("{error:?}"))?;
        if record_size != crate::storage::ROW_SIZE {
            return Err(format!(
                "this storage format currently requires {0}-byte rows; schema encodes to {record_size} bytes",
                crate::storage::ROW_SIZE,
            ));
        }

        let root_page_num = self.pager.get_unused_page_num();
        {
            let page = self.pager.get_page(root_page_num);
            LeafNode::new(page.as_mut()).initialize();
            set_node_root(page.as_mut(), true)
        }

        let schema = TableSchema {
            name: name.clone(),
            ..schema
        };
        self.catalog.insert(
            name.clone(),
            TableEntry {
                root_page_num: root_page_num,
                schema: schema.clone(),
            },
        );
        Ok(())
    }
}
