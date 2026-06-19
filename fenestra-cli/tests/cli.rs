//! End-to-end CLI: spawn the `fenestra` binary, feed a description on stdin, and
//! check the JSON output and exit codes. These cover the structural subcommands
//! (no GPU): validate, vocabulary, query, check.

use std::io::Write;
use std::process::{Command, Stdio};

fn run(args: &[&str], stdin: &str) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fenestra"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn fenestra");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

const GOOD: &str = r#"{"schema":"fenestra/1","root":{"button":{"label":"Go"}}}"#;
const BAD: &str = r#"{"schema":"fenestra/1","root":{"col":{"kids":[]}}}"#;

#[test]
fn validate_good_exits_zero() {
    let (code, stdout, _) = run(&["validate"], GOOD);
    assert_eq!(code, 0);
    assert!(stdout.contains("ok"), "{stdout}");
}

#[test]
fn validate_bad_exits_three_with_path() {
    let (code, _, stderr) = run(&["validate"], BAD);
    assert_eq!(code, 3, "stderr: {stderr}");
    assert!(stderr.contains("unknown field"), "{stderr}");
}

#[test]
fn vocabulary_lists_nodes_and_roles() {
    let (code, stdout, _) = run(&["vocabulary"], "");
    assert_eq!(code, 0);
    assert!(stdout.contains("button"), "{stdout}");
    assert!(stdout.contains("checkbox"), "{stdout}");
    assert!(stdout.contains("color_roles"), "{stdout}");
}

#[test]
fn query_by_role_over_stdin() {
    let (code, stdout, stderr) = run(&["query", "--selector", r#"{"role":"button"}"#], GOOD);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("\"role\": \"button\""), "{stdout}");
    assert!(stdout.contains("\"Go\""), "{stdout}");
}

#[test]
fn check_clean_form_exits_zero() {
    let (code, stdout, stderr) = run(&["check", "--size", "300x120"], GOOD);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("legible"), "{stdout}");
}
