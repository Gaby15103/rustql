use std::collections::HashMap;
use crate::storage::TableSchema;

pub enum CatalogError {
    TableAlreadyExists(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    tables: HashMap<String, TableEntry>,
}

impl Catalog {
    pub fn new() -> Self {
        Self {
            tables: HashMap::new(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&TableEntry> {
        self.tables.get(name)
    }

    pub fn insert(&mut self, name: String, entry: TableEntry) -> Result<(), String> {
        if self.tables.contains_key(&name) {
            return Err(format!("table '{name}' already exists"));
        }

        self.tables.insert(name, entry);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableEntry {
    pub root_page_num: usize,
    pub schema: TableSchema,
}