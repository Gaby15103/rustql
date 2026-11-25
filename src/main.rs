mod btree;
mod compiler;
mod meta;
mod repl;
mod storage;
mod vm;
mod sql_parser;

use compiler::{PrepareError, prepare_statement};
use meta::{MetaAction, MetaError, do_meta_command};
use repl::{print_prompt, read_input};
use std::{env, process};
use storage::Table;
use vm::{ExecuteError, execute_statement};

fn main() {
    let filename = env::args().nth(1).unwrap_or_else(|| {
        println!("Must supply a database filename.");
        process::exit(1);
    });
    let mut table = Table::db_open(&filename);

    let mut input_buffer = String::new();

    loop {
        print_prompt();
        read_input(&mut input_buffer);

        if input_buffer.is_empty() {
            continue;
        }

        if input_buffer.starts_with('.') {
            match do_meta_command(&input_buffer, &mut table) {
                Ok(MetaAction::Continue) => continue,
                Ok(MetaAction::Exit) => break,
                Err(MetaError::UnrecognizedCommand) => {
                    println!("Unrecognized command '{}'", input_buffer);
                    continue;
                }
            }
        }

        let statement = match prepare_statement(&input_buffer, &table.schema) {
            Ok(stmt) => stmt,
            Err(PrepareError::SyntaxError) => {
                println!("Syntzx error. Could not parse statement");
                continue;
            }
            Err(PrepareError::UnrecognizedStatement) => {
                println!("Unrecognized keyword at start of '{}'.", input_buffer);
                continue;
            }
            Err(PrepareError::InvalidRow(message)) => {
                println!("Invalid row: {message}");
                continue;
            }
        };

        match execute_statement(&statement, &mut table) {
            Ok(()) => println!("Executed."),
            Err(ExecuteError::TableFull) => println!("Error: Table full."),
            Err(ExecuteError::DuplicateKey) => println!("Error: Duplicate key."),
            Err(ExecuteError::InvalidRow) => println!("Error: Invalid row."),
            Err(ExecuteError::StatementNotFound) => print!("Error: Invalid Statement."),
        }
    }
}
