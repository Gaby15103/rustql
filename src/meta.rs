use crate::btree::{LeafNode, print_constants};
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
            println!("Tree:");
            crate::btree::print_tree(&mut table.pager, 0, 0);
            Ok(MetaAction::Continue)
        }
        _ => Err(MetaError::UnrecognizedCommand),
    }
}
