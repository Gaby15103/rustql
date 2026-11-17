use crate::compiler::Statement;

pub fn execute_statement(statement: &Statement) {
    match statement {
        Statement::Insert => {
            println!("This is where we would do an insert.");
        }
        Statement::Select => {
            println!("This is where we would do a select.");
        }
    }
}
