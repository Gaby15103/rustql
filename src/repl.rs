use std::io::{self, Write};

pub fn print_prompt() {
    print!("db > ");
    io::stdout().flush().expect("Failed to flush stdout");
}

pub fn read_input(buffer: &mut String) {
    buffer.clear();
    let bytes_read = io::stdin()
        .read_line(buffer)
        .expect("Error reading input");

    if bytes_read == 0 {
        println!();
        std::process::exit(0);
    }

    let trimmed_len = buffer.trim_end().len();
    buffer.truncate(trimmed_len);
}
