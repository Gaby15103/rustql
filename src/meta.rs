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
            db.close();
            Ok(MetaAction::Exit)
        }
        _ => Err(MetaError::UnrecognizedCommand),
    }
}
