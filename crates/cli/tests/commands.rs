use std::process::Command;

#[test]
fn unimplemented_commands_remain_explicit() {
    for command in ["run", "check", "info", "list", "mask", "convert", "record"] {
        let output = Command::new(env!("CARGO_BIN_EXE_dmd"))
            .arg(command)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains(&format!("{command}: not implemented"))
        );
    }
}

#[test]
fn help_succeeds_and_lists_the_surface() {
    let output = Command::new(env!("CARGO_BIN_EXE_dmd"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    for option in [
        "run",
        "check",
        "info",
        "list",
        "mask",
        "convert",
        "record",
        "--search-path",
        "--events",
    ] {
        assert!(help.contains(option));
    }
}

#[test]
fn removed_backend_is_rejected_before_greeting() {
    let output = Command::new(env!("CARGO_BIN_EXE_dmd"))
        .args(["run", "--backend", "replay"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}
#[test]
fn prestart_eof_finishes_without_setup() {
    let output = Command::new(env!("CARGO_BIN_EXE_dmd"))
        .args(["run", "--message-format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 2);
    assert!(stdout.contains("\"reason\":\"stopped\""));
}

#[test]
fn malformed_prestart_lines_keep_fifo_and_stop_never_opens_recording() {
    use std::{io::Write, process::Stdio};
    let mut child = Command::new(env!("CARGO_BIN_EXE_dmd"))
        .args(["run", "--message-format", "json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"{\"command\":\"pause\"}\n{\"command\":\"pause\",\"command\":\"stop\"}\n{\"command\":\"stop\"}\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let events: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(events.len(), 4);
    assert_eq!(events[1]["fatal"], false);
    assert_eq!(events[2]["fatal"], false);
    assert_eq!(events[3]["reason"], "stopped");
}

#[test]
fn oversized_command_is_fatal_and_terminal() {
    use std::{io::Write, process::Stdio};
    let mut child = Command::new(env!("CARGO_BIN_EXE_dmd"))
        .args(["run", "--message-format", "json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let oversized = vec![b'x'; dmd_protocol::MAX_LINE_BYTES + 2];
    let _ = child.stdin.take().unwrap().write_all(&oversized);
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let events: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(events.len(), 3);
    assert_eq!(events[1]["fatal"], true);
    assert_eq!(events[2]["reason"], "error");
}
