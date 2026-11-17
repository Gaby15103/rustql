use std::process;

pub enum MetaError {
    UnrecognizedCommand,
}

pub fn do_meta_command(input: &str) -> Result<(), MetaError> {
    match input {
        ".exit" => {
            // Note: Rust automatically drops/frees variables before exiting here
            // if we return a signal, or we can exit directly for now.
            process::exit(0);
        }
        _ => Err(MetaError::UnrecognizedCommand),
    }
}
