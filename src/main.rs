mod compiler;
mod meta;
mod repl;
mod vm;

use compiler::{prepare_statement, PrepareError};
use meta::{do_meta_command, MetaError};
use repl::{print_prompt, read_input};
use vm::execute_statement;

fn main() {
    let mut input_buffer = String::new();

    loop {
        print_prompt();
        read_input(&mut input_buffer);

        if input_buffer.is_empty() {
            continue;
        }

        // Handle meta-commands starting with '.'
        if input_buffer.starts_with('.') {
            match do_meta_command(&input_buffer) {
                Ok(()) => continue,
                Err(MetaError::UnrecognizedCommand) => {
                    println!("Unrecognized command '{}'", input_buffer);
                    continue;
                }
            }
        }

        // Compile SQL string into a Statement
        let statement = match prepare_statement(&input_buffer) {
            Ok(stmt) => stmt,
            Err(PrepareError::UnrecognizedStatement) => {
                println!("Unrecognized keyword at start of '{}'.", input_buffer);
                continue;
            }
        };

        // Execute the compiled statement
        execute_statement(&statement);
        println!("Executed.");
    }
}
