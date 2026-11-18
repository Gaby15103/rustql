use crate::btree::{print_constants, LeafNode};
use crate::storage::Table;
use std::process;

pub enum MetaAction {
    Continue,
    Exit,
}

pub enum MetaError {
    UnrecognizedCommand,
}

pub fn do_meta_command(input: &str, table: &mut Table) -> Result<MetaAction, MetaError> {
    match input {
        ".exit" => {
            table.db_close();
            Ok(MetaAction::Exit)
        }
        ".constants" => {
            println!("Constants:");
            print_constants();
            Ok(MetaAction::Continue)
        }
        ".btree" => {
            println!("Btree:");
            let root_page = table.pager.get_page(0);
            LeafNode::new(root_page.as_mut()).print_tree();
            Ok(MetaAction::Continue)
        }
        _ => Err(MetaError::UnrecognizedCommand),
    }
}
