use crate::btree::{LeafNode, print_constants};
use std::process;
use crate::storage::Database;

pub enum MetaAction {
    Continue,
    Exit,
}

pub enum MetaError {
    UnrecognizedCommand,
}

pub fn do_meta_command(input: &str, db: &mut Database) -> Result<MetaAction, MetaError> {
    match input {
        ".exit" => {
            db.db_close();
            Ok(MetaAction::Exit)
        }
        ".constants" => {
            println!("Constants:");
            print_constants();
            Ok(MetaAction::Continue)
        }
        ".btree" => {
            println!("Tree:");
            crate::btree::print_tree(&mut db.pager, 0, 0);
            Ok(MetaAction::Continue)
        }
        _ => Err(MetaError::UnrecognizedCommand),
    }
}
