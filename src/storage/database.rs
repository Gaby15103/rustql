use crate::btree::{LeafNode, set_node_root};
use crate::storage::{decode_catalog, Catalog, Pager, TableEntry, TableSchema, PAGE_SIZE, encode_catalog};

const CATALOG_MAGIC: &[u8; 8] = b"RUSTQL01";
const CATALOG_VERSION: u16 = 1;

pub struct Database {
    pub pager: Pager,
    pub catalog: Catalog,
}

impl Database {
    pub fn open(file_path: &str) -> Self {
        let mut pager = Pager::open(file_path);

        let catalog = if pager.file_length == 0 {
            Catalog::new()
        }else{
            let page = pager
                .get_page(0)
                .as_ref()
                .to_owned();

            decode_catalog(&page).unwrap_or_else(|error| {
                panic!("Invalid RustQL database catalog: {}", error.message);
            })
        };

        if pager.num_pages == 0 {
            pager.get_page(0);
        }

        Self {
            pager,
            catalog
        }
    }
    pub fn close(&mut self) {
        let page = encode_catalog(&self.catalog)
            .unwrap_or_else(|error| panic!("Failed to save catalog: {}", error.message));

        self.pager.get_page(0).copy_from_slice(&page);

        for i in 0..self.pager.num_pages {
            if let Some(_) = self.pager.pages[i].as_ref() {
                self.pager.flush(i, PAGE_SIZE);
            }
        }

        self.pager
            .file
            .sync_all()
            .expect("failed to sync database file");
    }

    pub fn table_names(&self) -> Vec<String> {
        self.catalog.names()
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
        if record_size > crate::storage::ROW_SIZE {
            return Err(format!(
                "this storage format currently requires {0}-byte rows; schema encodes to {record_size} bytes",
                crate::storage::ROW_SIZE,
            ));
        }

        let root_page_num = if self.pager.num_pages == 0 {
            1
        } else {
            self.pager.num_pages
        };

        if root_page_num == 0 {
            unreachable!("page 0 is reserved for the catalog");
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
