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
use vm::{ExecuteError, execute_statement};

use crate::sql_parser::{
    Lexer, Parser,
};
use crate::storage::Database;

fn main() {
    let filename = env::args().nth(1).unwrap_or_else(|| {
        println!("Must supply a database filename.");
        process::exit(1);
    });
    let mut db = Database::open(&filename);

    let mut input_buffer = String::new();

    loop {
        print_prompt();
        read_input(&mut input_buffer);

        if input_buffer.is_empty() {
            continue;
        }

        if input_buffer.starts_with('.') {
            match do_meta_command(&input_buffer, &mut db) {
                Ok(MetaAction::Continue) => continue,
                Ok(MetaAction::Exit) => break,
                Err(MetaError::UnrecognizedCommand) => {
                    println!("Unrecognized command '{}'", input_buffer);
                    continue;
                }
            }
        }

        use sql_parser::{Lexer, Parser};

        let tokens = Lexer::new(&input_buffer)
            .tokenize()
            .map_err(|error| format!("SQL error: {error}"));

        let tokens = match tokens {
            Ok(tokens) => tokens,
            Err(error) => {
                println!("{error}");
                continue;
            }
        };

        let mut parser = Parser::new(tokens);
        let statements = match parser.parse_statements() {
            Ok(statements) => statements,
            Err(error) => {
                println!("SQL error: {error}");
                continue;
            }
        };

        let statement = match statements.as_slice() {
            [statement] => statement,
            [] => continue,
            _ => {
                println!("Error: enter one SQL statement at a time.");
                continue;
            }
        };

        match execute_statement(&statement, &mut db) {
            Ok(()) => println!("Executed."),
            Err(ExecuteError::TableFull) => println!("Error: Table full."),
            Err(ExecuteError::DuplicateKey) => println!("Error: Duplicate key."),
            Err(ExecuteError::InvalidRow) => println!("Error: Invalid row."),
            Err(ExecuteError::StatementNotFound) => print!("Error: Invalid Statement."),
        }
    }
}
