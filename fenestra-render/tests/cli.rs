//! End-to-end CLI: spawn the `fenestra` binary, feed a description on stdin, and
//! check the JSON output and exit codes. The structural subcommands (validate,
//! vocabulary, query, check) need no GPU; the `verify` subcommand renders, so it
//! exercises the pixel path.

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

#[test]
fn verify_login_scenario_over_stdin() {
    let scenario = include_str!("scenarios/login.json");
    let (code, stdout, stderr) = run(&["verify"], scenario);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("\"ok\": true"), "{stdout}");
    assert!(stdout.contains("\"emitted\""), "{stdout}");
}

/// `--bless` writes the post-interaction baseline; a later static (pre-click)
/// render then mismatches it, exits 1, and `--out` writes the diff image.
#[test]
fn verify_bless_then_diff_on_mismatch() {
    let dir = std::env::temp_dir();
    let baseline = dir.join("fenestra_cli_bless_baseline.png");
    let diff = dir.join("fenestra_cli_bless_diff.png");
    let _ = std::fs::remove_file(&baseline);
    let _ = std::fs::remove_file(&diff);
    let b = baseline.to_str().unwrap();

    // A bound checkbox clicked on: bless captures the *checked* render.
    let driven = format!(
        r#"{{"schema":"fenestra/1","description":{{"schema":"fenestra/1","state":{{"on":false}},"root":{{"checkbox":{{"bind":"on","label":"On","id":"c"}}}}}},"size":"160x70","steps":[{{"click":{{"id":"c"}}}}],"expect":{{"screenshot":{{"baseline":"{b}","tolerance":3,"budget":0.002}}}}}}"#
    );
    let (code, _o, e) = run(&["verify", "--bless"], &driven);
    assert_eq!(code, 0, "bless stderr: {e}");
    assert!(baseline.exists(), "bless writes the baseline");

    // The same UI without the click (unchecked) mismatches the checked baseline.
    let static_ = format!(
        r#"{{"schema":"fenestra/1","description":{{"schema":"fenestra/1","state":{{"on":false}},"root":{{"checkbox":{{"bind":"on","label":"On","id":"c"}}}}}},"size":"160x70","expect":{{"screenshot":{{"baseline":"{b}","tolerance":3,"budget":0.002}}}}}}"#
    );
    let dstr = diff.to_str().unwrap();
    let (code, out, _e) = run(&["verify", "--out", dstr], &static_);
    assert_eq!(code, 1, "a verification mismatch is exit 1: {out}");
    assert!(out.contains("\"ok\": false"), "{out}");
    assert!(diff.exists(), "the diff image is written on mismatch");

    let _ = std::fs::remove_file(&baseline);
    let _ = std::fs::remove_file(&diff);
}

/// `match-png --mask x,y,w,h` excludes a rectangle from the pixel diff: the
/// same region that fails unmasked passes once it is masked out.
#[test]
fn match_png_mask_ignores_region() {
    let dir = std::env::temp_dir();
    let baseline = dir.join("fenestra_cli_mask_baseline.png");
    let _ = std::fs::remove_file(&baseline);
    let b = baseline.to_str().unwrap();

    let (code, _out, stderr) = run(&["render", "--size", "300x120", "--out", b], GOOD);
    assert_eq!(code, 0, "render stderr: {stderr}");
    assert!(baseline.exists(), "render writes the baseline");

    let other = r#"{"schema":"fenestra/1","root":{"button":{"label":"Different"}}}"#;

    // Unmasked: a different label makes the two renders differ.
    let (code, out, stderr) = run(
        &[
            "match-png",
            "--baseline",
            b,
            "--size",
            "300x120",
            "--tolerance",
            "3",
            "--budget",
            "0.002",
        ],
        other,
    );
    assert_eq!(code, 1, "unmasked mismatch is exit 1: {out}\n{stderr}");
    assert!(out.contains("\"ok\": false"), "{out}");

    // Masked over the whole frame: the same mismatch is now ignored.
    let (code, out, stderr) = run(
        &[
            "match-png",
            "--baseline",
            b,
            "--size",
            "300x120",
            "--tolerance",
            "3",
            "--budget",
            "0.002",
            "--mask",
            "0,0,300,120",
        ],
        other,
    );
    assert_eq!(code, 0, "masked match is exit 0: {out}\n{stderr}");
    assert!(out.contains("\"ok\": true"), "{out}");

    let _ = std::fs::remove_file(&baseline);
}

/// Hostile `--mask` values (malformed, non-numeric, non-finite, negative) are
/// rejected with a self-explaining exit-3 error, never a panic — the baseline
/// here doesn't even exist, proving mask validation runs before the file read.
#[test]
fn match_png_hostile_mask_is_rejected() {
    let missing = "/nonexistent/fenestra_cli_no_such_baseline.png";

    let (code, _out, stderr) = run(
        &["match-png", "--baseline", missing, "--mask", "1,2,3"],
        GOOD,
    );
    assert_eq!(code, 3, "wrong field count is exit 3: {stderr}");
    assert!(stderr.contains("--mask"), "{stderr}");

    let (code, _out, stderr) = run(
        &["match-png", "--baseline", missing, "--mask", "x,0,10,10"],
        GOOD,
    );
    assert_eq!(code, 3, "non-numeric field is exit 3: {stderr}");
    assert!(stderr.contains("--mask"), "{stderr}");

    let (code, _out, stderr) = run(
        &["match-png", "--baseline", missing, "--mask", "0,0,-10,10"],
        GOOD,
    );
    assert_eq!(code, 3, "negative width is exit 3: {stderr}");
    assert!(stderr.contains("negative"), "{stderr}");

    let (code, _out, stderr) = run(
        &["match-png", "--baseline", missing, "--mask", "NaN,0,10,10"],
        GOOD,
    );
    assert_eq!(code, 3, "non-finite coordinate is exit 3: {stderr}");
    assert!(stderr.contains("finite"), "{stderr}");
}

/// A `verify` setup error (an unreadable baseline, no `--bless`) exits 3 with a
/// self-explaining message — distinct from a check failure (exit 1).
#[test]
fn verify_missing_baseline_exits_three() {
    let missing = std::env::temp_dir().join("fenestra_cli_no_such_baseline.png");
    let _ = std::fs::remove_file(&missing);
    let scenario = format!(
        r#"{{"schema":"fenestra/1","description":{{"schema":"fenestra/1","root":{{"button":{{"label":"Go"}}}}}},"size":"120x60","expect":{{"screenshot":{{"baseline":"{}","tolerance":3,"budget":0.002}}}}}}"#,
        missing.to_str().unwrap()
    );
    let (code, _out, stderr) = run(&["verify"], &scenario);
    assert_eq!(code, 3, "a setup error is exit 3; stderr: {stderr}");
    assert!(stderr.contains("cannot read baseline"), "{stderr}");
}
