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
        path.push(format!(
            "rustql_{}_{}_{}.db",
            test_name,
            std::process::id(),
            id
        ));
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

        assert!(
            output.status.success(),
            "database process failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

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
    let result = db.run(&["insert 1 user1 person1@example.com", "select", ".exit"]);

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

    let expected = vec!["db > Executed.", &expected_row, "Executed.", "db > "];

    assert_eq!(result, expected);
}

#[test]
fn keep_data_after_closing_connection() {
    let db = TestDb::new("persistence");

    let result1 = db.run(&["insert 1 user1 person1@example.com", ".exit"]);
    assert_eq!(result1, vec!["db > Executed.", "db > "]);

    let result2 = db.run(&["select", ".exit"]);
    assert_eq!(
        result2,
        vec!["db > 1,user1,person1@example.com", "Executed.", "db > ",]
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
#[test]
fn prints_all_rows_in_a_multi_level_tree() {
    let db = TestDb::new("multi_level_select");

    let mut commands = Vec::new();
    for i in 1..=15 {
        commands.push(format!("insert {i} user{i} person{i}@example.com"));
    }
    commands.push("select".to_string());
    commands.push(".exit".to_string());

    let cmd_refs: Vec<&str> = commands.iter().map(String::as_str).collect();
    let result = db.run(&cmd_refs);

    // Grab the 15 rows printed after "select"
    let select_start = result
        .iter()
        .position(|line| line.starts_with("db > 1,user1,"))
        .expect("Did not find select output");

    let mut expected = vec![format!("db > 1,user1,person1@example.com")];
    for i in 2..=15 {
        expected.push(format!("{i},user{i},person{i}@example.com"));
    }
    expected.push("Executed.".to_string());
    expected.push("db > ".to_string());

    assert_eq!(&result[select_start..], expected.as_slice());
}

#[test]
fn stress_test_1400_rows_out_of_order_and_persisted() {
    let db = TestDb::new("stress_1400");

    let total_rows: u32 = 1400;
    let mut ids: Vec<u32> = (1..=total_rows).collect();
    for i in 0..ids.len() {
        let swap_idx = (i * 7919 + 104729) % ids.len();
        ids.swap(i, swap_idx);
    }

    let mut insert_commands: Vec<String> = ids
        .iter()
        .map(|id| format!("insert {id} user{id} person{id}@example.com"))
        .collect();
    insert_commands.push(".exit".to_string());

    let insert_refs: Vec<&str> = insert_commands.iter().map(String::as_str).collect();
    let insert_output = db.run(&insert_refs);

    assert_eq!(insert_output.len(), (total_rows + 1) as usize);
    for line in &insert_output[..total_rows as usize] {
        assert_eq!(line, "db > Executed.");
    }

    let select_output = db.run(&["select", ".exit"]);

    println!("Total lines returned by select: {}", select_output.len());
    println!("Lines 170..177: {:?}", &select_output[170..177]);
    println!(
        "Last 5 lines: {:?}",
        &select_output[select_output.len() - 5..]
    );

    assert_eq!(select_output.len(), (total_rows + 2) as usize);
    assert_eq!(select_output[0], "db > 1,user1,person1@example.com");

    for id in 2..=total_rows {
        let expected_line = format!("{id},user{id},person{id}@example.com");
        assert_eq!(select_output[(id - 1) as usize], expected_line);
    }

    assert_eq!(select_output[total_rows as usize], "Executed.");
    assert_eq!(select_output[(total_rows + 1) as usize], "db > ");
}
