use std::io::Write;
use std::process::{Command, Stdio};

fn run_script(commands: &[&str]) -> Vec<String> {
    let bin_path = env!("CARGO_BIN_EXE_rustql");

    let mut child = Command::new(bin_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn database binary");

    if let Some(mut stdin) = child.stdin.take() {
        for command in commands {
            writeln!(stdin, "{}", command).expect("Failed to write to stdin");
        }
    }
    let output = child
        .wait_with_output()
        .expect("Failed to read stdout from child process");

    let raw_output = String::from_utf8(output.stdout).expect("Output was not valid UTF-8");

    raw_output.lines().map(String::from).collect()
}

#[test]
fn inserts_and_retrieves_a_row() {
    let result = run_script(&[
        "insert 1 user1 person1@example.com",
        "select",
        ".exit",
    ]);

    let expected = vec![
        "db > Executed.",
        "db > 1,user1,person1@example.com",
        "Executed.",
        "db > ",
    ];

    assert_eq!(result, expected);
}

#[test]
fn inserts_multiple_and_retrieve_row_full(){
    let mut items: Vec<String> = Vec::with_capacity(1402);

    for i in 0..1401 {
        items.push(format!("insert {i} user{i} person{i}@example.com"));
    }
    items.push(".exit".to_string());

    let commands: Vec<&str> = items.iter().map(String::as_str).collect();
    let result = run_script(&commands);

    assert_eq!(result[1400], "db > Error: Table full");
}

#[test]
fn insert_strings_that_are_the_maximum_length(){
    let long_username = "a".repeat(32);
    let long_email = "a".repeat(255);

    let result = run_script(&[
        format!("insert 1 {long_username} {long_email}").as_str(),
        "select",
        ".exit",
    ]);

    let output: String = format!("db > 1,{long_username},{long_email}");

    let expected = vec![
        "db > Executed.",
        output.as_str(),
        "Executed.",
        "db > ",
    ];

    assert_eq!(result, expected);
}

#[test]
fn keep_data_after_closing_connection(){

    let result = run_script(&[
        "insert 1 user1 user1@exemple.com",
        ".exit",
    ]);

    let expected = vec![
        "db > Executed.",
        "db > ",
    ];
    assert_eq!(result, expected);

    let result2 = run_script(&[
        "select",
        ".exit",
    ]);

    let expected2 = vec![
        "db > (1, user1, person1@example.com)",
        "Executed.",
        "db > ",
    ];

    assert_eq!(result2, expected2);
}