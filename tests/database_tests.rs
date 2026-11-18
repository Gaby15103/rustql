use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDb {
    path: PathBuf,
}

impl TestDb {
    fn new(test_name: &str) -> Self {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut path = std::env::temp_dir();
        path.push(format!("rustql_{}_{}_{}.db", test_name, std::process::id(), id));
        let _ = fs::remove_file(&path);
        Self { path }
    }

    fn run(&self, commands: &[&str]) -> Vec<String> {
        let bin_path = env!("CARGO_BIN_EXE_rustql");

        let mut child = Command::new(bin_path)
            .arg(&self.path)
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
}

impl Drop for TestDb {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[test]
fn inserts_and_retrieves_a_row() {
    let db = TestDb::new("insert_retrieve");
    let result = db.run(&[
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
fn insert_strings_that_are_the_maximum_length() {
    let db = TestDb::new("max_length");
    let long_username = "a".repeat(32);
    let long_email = "a".repeat(255);

    let insert_cmd = format!("insert 1 {long_username} {long_email}");
    let expected_row = format!("db > 1,{long_username},{long_email}");

    let result = db.run(&[&insert_cmd, "select", ".exit"]);

    let expected = vec![
        "db > Executed.",
        &expected_row,
        "Executed.",
        "db > ",
    ];

    assert_eq!(result, expected);
}

#[test]
fn keep_data_after_closing_connection() {
    let db = TestDb::new("persistence");

    let result1 = db.run(&[
        "insert 1 user1 person1@example.com",
        ".exit",
    ]);
    assert_eq!(result1, vec!["db > Executed.", "db > "]);

    let result2 = db.run(&[
        "select",
        ".exit",
    ]);
    assert_eq!(
        result2,
        vec![
            "db > 1,user1,person1@example.com",
            "Executed.",
            "db > ",
        ]
    );
}

#[test]
fn prints_error_message_if_there_is_a_duplicate_id() {
    let db = TestDb::new("duplicate_id");

    let result = db.run(&[
        "insert 1 user1 person1@example.com",
        "insert 1 user1 person1@example.com",
        "select",
        ".exit",
    ]);

    assert_eq!(
        result,
        vec![
            "db > Executed.",
            "db > Error: Duplicate key.",
            "db > 1,user1,person1@example.com",
            "Executed.",
            "db > ",
        ]
    );
}

#[test]
fn allows_printing_out_the_structure_of_a_one_node_btree() {
    let db = TestDb::new("one_node_btree");

    let result = db.run(&[
        "insert 3 user3 person3@example.com",
        "insert 1 user1 person1@example.com",
        "insert 2 user2 person2@example.com",
        ".btree",
        ".exit",
    ]);

    assert_eq!(
        result,
        vec![
            "db > Executed.",
            "db > Executed.",
            "db > Executed.",
            "db > Tree:",
            "leaf (size 3)",
            "  - 1",
            "  - 2",
            "  - 3",
            "db > ",
        ]
    );
}

#[test]
fn allows_printing_out_the_structure_of_a_3_leaf_node_btree() {
    let db = TestDb::new("split_leaf_btree");
    let mut commands = Vec::new();
    for i in 1..=14 {
        commands.push(format!("insert {i} user{i} person{i}@example.com"));
    }
    commands.push(".btree".to_string());
    commands.push(".exit".to_string());

    let cmd_refs: Vec<&str> = commands.iter().map(String::as_str).collect();
    let result = db.run(&cmd_refs);

    let tree_start = result
        .iter()
        .position(|line| line == "db > Tree:")
        .expect("Did not find .btree output");

    assert_eq!(
        &result[tree_start..],
        &[
            "db > Tree:",
            "internal (size 1)",
            "  leaf (size 7)",
            "    - 1",
            "    - 2",
            "    - 3",
            "    - 4",
            "    - 5",
            "    - 6",
            "    - 7",
            "  - key 7",
            "  leaf (size 7)",
            "    - 8",
            "    - 9",
            "    - 10",
            "    - 11",
            "    - 12",
            "    - 13",
            "    - 14",
            "db > ",
        ]
    );
}
