use crate::storage::{Database, TableSchema, ValueType};

pub enum MetaAction {
    Continue,
    Exit,
}

pub enum MetaError {
    UnrecognizedCommand,
    TableNotFound(String),
    InvalidArguments(&'static str),
}

pub fn do_meta_command(input: &str, db: &mut Database) -> Result<MetaAction, MetaError> {
    let mut parts = input.split_whitespace();
    let command = parts.next().unwrap_or("");
    match command {
        ".exit" => {
            db.close();
            Ok(MetaAction::Exit)
        },
        ".help" => {
            println!("Meta-commands:");
            println!("  .help             Show this help");
            println!("  .tables           List tables");
            println!("  .schema [table]   Show table schema(s)");
            println!("  .exit             Save and exit");
            Ok(MetaAction::Continue)
        }
        ".tables" => {
            let names = db.table_names();

            if names.is_empty() {
                println!("(no tables)");
            } else {
                for name in names {
                    println!("{name}");
                }
            }

            Ok(MetaAction::Continue)
        },
        ".schema" => {
            let requested_table = parts.next();

            if parts.next().is_some() {
                return Err(MetaError::InvalidArguments(
                    "usage: .schema [table]",
                ));
            }

            if let Some(name) = requested_table {
                let entry = db
                    .table(name)
                    .ok_or_else(|| MetaError::TableNotFound(name.to_string()))?;
                print_schema(&entry.schema);
            } else {
                for name in db.table_names() {
                    if let Some(entry) = db.table(&name) {
                        print_schema(&entry.schema);
                    }
                }
            }

            Ok(MetaAction::Continue)
        }
        _ => Err(MetaError::UnrecognizedCommand),
    }
}
fn print_schema(schema: &TableSchema) {
    println!("CREATE TABLE {} (", schema.name);

    for (index, column) in schema.columns.iter().enumerate() {
        let data_type = match column.value_type {
            ValueType::Integer => "INT",
            ValueType::Text => "TEXT",
            ValueType::Bool => "BOOL",
            ValueType::Null => "NULL",
        };

        let primary_key = if schema.primary_key_index == Some(index) {
            " PRIMARY KEY"
        } else {
            ""
        };

        let comma = if index + 1 < schema.columns.len() {
            ","
        } else {
            ""
        };

        println!("  {} {}{}{}", column.name, data_type, primary_key, comma);
    }

    println!(");");
}
