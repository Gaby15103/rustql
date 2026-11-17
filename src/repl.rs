use std::io::{self, Write};

pub fn print_prompt() {
    print!("db > ");
    // Flush stdout immediately so "db > " appears before waiting for input
    io::stdout().flush().expect("Failed to flush stdout");
}

pub fn read_input(buffer: &mut String) {
    buffer.clear(); // Reuse the allocated heap memory on every loop
    let bytes_read = io::stdin()
        .read_line(buffer)
        .expect("Error reading input");

    if bytes_read == 0 {
        // Handles Ctrl+D (EOF) gracefully
        println!();
        std::process::exit(0);
    }

    // Remove the trailing newline (\n or \r\n) in place
    let trimmed_len = buffer.trim_end().len();
    buffer.truncate(trimmed_len);
}
