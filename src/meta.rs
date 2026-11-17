use std::process;

pub enum MetaAction{
    Continue,
    Exit,
}

pub enum MetaError {
    UnrecognizedCommand,
}

pub fn do_meta_command(input: &str) -> Result<MetaAction, MetaError> {
    match input {
        ".exit" => Ok(MetaAction::Exit),
        _ => Err(MetaError::UnrecognizedCommand),
    }
}
