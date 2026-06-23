//! The unified verify loop: a `Scenario` bundles a description, optional steps,
//! and expectations, and `verify` folds every check into one report. These
//! exercise the render path, so they need a GPU. The headline case proves a
//! screenshot expectation compares the *post-interaction* pixels.

use std::path::PathBuf;

use fenestra_render::engine::EngineError;
use fenestra_render::{Scenario, bless, verify};

fn scenario(json: &str) -> Scenario {
    serde_json::from_str(json).expect("valid scenario")
}

/// A unique temp baseline path for a test (no two tests share one).
fn temp_png(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("fenestra_scn_{name}.png"))
}

/// A static (no-steps) form passes a11y + aria + query checks in one verify.
#[test]
fn verify_static_form_all_checks_pass() {
    let s = scenario(
        r#"{
        "schema": "fenestra/1",
        "description": { "schema": "fenestra/1", "root": { "col": { "children": [
            { "button": { "label": "Add", "on_click": "add" } },
            { "checkbox": { "checked": false, "label": "Done" } }
        ] } } },
        "size": "400x300",
        "expect": {
            "a11y": true,
            "aria": { "snapshot": "- button \"Add\"" },
            "queries": [ { "selector": { "role": "button" }, "count": 1 } ]
        }
    }"#,
    );
    let out = verify(&s).expect("scenario runs");
    assert!(
        out.report.ok,
        "all checks should pass: {:?}",
        out.report.checks
    );
    let names: Vec<&str> = out.report.checks.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"a11y"), "{names:?}");
    assert!(names.contains(&"aria"), "{names:?}");
    assert!(
        out.report.checks.iter().all(|c| c.ok),
        "{:?}",
        out.report.checks
    );
}

/// Driving a click emits the author intent, and the `emitted` check passes.
#[test]
fn verify_emitted_intent_from_click() {
    let s = scenario(
        r#"{
        "schema": "fenestra/1",
        "description": { "schema": "fenestra/1", "root": {
            "button": { "label": "Add", "on_click": "add" }
        } },
        "size": "300x120",
        "steps": [ { "click": { "role": "button", "name": "Add" } } ],
        "expect": { "emitted": ["add"] }
    }"#,
    );
    let out = verify(&s).expect("scenario runs");
    assert!(out.report.ok, "{:?}", out.report.checks);
    assert_eq!(out.report.emitted, vec!["add".to_string()]);
    let emitted = out
        .report
        .checks
        .iter()
        .find(|c| c.name == "emitted")
        .expect("an emitted check");
    assert!(emitted.ok);
}

/// A wrong expectation makes the overall report fail, with that check flagged and
/// a non-empty detail — the agent reads what to fix.
#[test]
fn verify_reports_failing_check() {
    let s = scenario(
        r#"{
        "schema": "fenestra/1",
        "description": { "schema": "fenestra/1", "root": {
            "button": { "label": "Add", "on_click": "add" }
        } },
        "size": "300x120",
        "steps": [ { "click": { "role": "button", "name": "Add" } } ],
        "expect": { "emitted": ["WRONG"] }
    }"#,
    );
    let out = verify(&s).expect("scenario runs");
    assert!(!out.report.ok, "the overall report should fail");
    let emitted = out
        .report
        .checks
        .iter()
        .find(|c| c.name == "emitted")
        .expect("an emitted check");
    assert!(!emitted.ok);
    assert!(!emitted.detail.is_empty(), "the miss should explain itself");
}

/// The headline: a screenshot expectation compares the *post-interaction* pixels.
/// We bless the baseline from a scenario that clicks a bound checkbox (so the
/// baseline is the *checked* render), then: (1) the same scenario verifies clean,
/// and (2) the identical description *without* the click fails against that same
/// baseline — proving the comparison sees the driven state, not the static one.
#[test]
fn verify_screenshot_compares_post_interaction_pixels() {
    let baseline = temp_png("post_interaction");
    let _ = std::fs::remove_file(&baseline);
    let baseline_str = baseline.to_str().unwrap();

    let driven = scenario(&format!(
        r#"{{
        "schema": "fenestra/1",
        "description": {{ "schema": "fenestra/1", "state": {{ "agreed": false }}, "root": {{
            "checkbox": {{ "bind": "agreed", "label": "Agree", "id": "cb" }}
        }} }},
        "size": "200x80",
        "steps": [ {{ "click": {{ "id": "cb" }} }} ],
        "expect": {{ "screenshot": {{ "baseline": "{baseline_str}", "tolerance": 3, "budget": 0.002 }} }}
    }}"#,
    ));

    // Bless captures the *after-click* (checked) render as the baseline.
    let written = bless(&driven).expect("bless writes the baseline");
    assert_eq!(written, baseline);
    assert!(baseline.exists(), "baseline file should exist");

    // The driven scenario matches its own post-interaction baseline, and the
    // screenshot check genuinely ran (a passing scenario carries no diff image).
    let ok = verify(&driven).expect("scenario runs");
    assert!(
        ok.report.ok,
        "driven render should match its blessed baseline: {:?}",
        ok.report.checks
    );
    assert!(
        ok.report
            .checks
            .iter()
            .any(|c| c.name == "screenshot" && c.ok),
        "the screenshot check ran and passed: {:?}",
        ok.report.checks
    );
    assert!(ok.diff_png.is_none(), "a passing screenshot yields no diff");

    // Direct proof that `bless` captured *post-interaction* bytes: blessing the
    // same description WITHOUT the click writes a different baseline file.
    let static_baseline = temp_png("post_interaction_static");
    let _ = std::fs::remove_file(&static_baseline);
    let static_baseline_str = static_baseline.to_str().unwrap();
    let static_ = scenario(&format!(
        r#"{{
        "schema": "fenestra/1",
        "description": {{ "schema": "fenestra/1", "state": {{ "agreed": false }}, "root": {{
            "checkbox": {{ "bind": "agreed", "label": "Agree", "id": "cb" }}
        }} }},
        "size": "200x80",
        "expect": {{ "screenshot": {{ "baseline": "{static_baseline_str}", "tolerance": 3, "budget": 0.002 }} }}
    }}"#,
    ));
    bless(&static_).expect("bless the static baseline");
    assert_ne!(
        std::fs::read(&baseline).unwrap(),
        std::fs::read(&static_baseline).unwrap(),
        "driven (checked) and static (unchecked) baselines must differ on disk"
    );
    let _ = std::fs::remove_file(&static_baseline);

    // And the static scenario verified against the *checked* baseline must NOT
    // match — the screenshot check is comparing post-interaction pixels.
    let static_ = scenario(&format!(
        r#"{{
        "schema": "fenestra/1",
        "description": {{ "schema": "fenestra/1", "state": {{ "agreed": false }}, "root": {{
            "checkbox": {{ "bind": "agreed", "label": "Agree", "id": "cb" }}
        }} }},
        "size": "200x80",
        "expect": {{ "screenshot": {{ "baseline": "{baseline_str}", "tolerance": 3, "budget": 0.002 }} }}
    }}"#,
    ));
    let mismatch = verify(&static_).expect("scenario runs");
    assert!(
        !mismatch.report.ok,
        "static unchecked render should differ from the checked baseline"
    );
    let shot = mismatch
        .report
        .checks
        .iter()
        .find(|c| c.name == "screenshot")
        .expect("a screenshot check");
    assert!(!shot.ok);
    assert!(
        mismatch.diff_png.is_some(),
        "a screenshot mismatch yields a diff image"
    );

    let _ = std::fs::remove_file(&baseline);
}

/// Blessing a scenario with no screenshot expectation is a setup error.
#[test]
fn bless_without_screenshot_is_error() {
    let s = scenario(
        r#"{
        "schema": "fenestra/1",
        "description": { "schema": "fenestra/1", "root": { "button": { "label": "Go" } } },
        "expect": { "a11y": true }
    }"#,
    );
    let err = bless(&s).expect_err("nothing to bless");
    assert!(matches!(err, EngineError::Scenario(_)), "{err:?}");
}

/// An empty-label button parses fine (it is just unlabeled); the verify path runs
/// and a role query still finds it.
#[test]
fn verify_finds_button_with_empty_label() {
    let s = scenario(
        r#"{
        "schema": "fenestra/1",
        "description": { "schema": "fenestra/1", "root": { "button": { "label": "" } } },
        "expect": { "queries": [ { "selector": { "role": "button" }, "count": 1 } ] }
    }"#,
    );
    let out = verify(&s).expect("scenario runs");
    let q = out
        .report
        .checks
        .iter()
        .find(|c| c.name.starts_with("query:"))
        .expect("a query check");
    assert!(q.ok, "the button is found: {}", q.detail);
}

/// A description that does not parse (an unknown color role) surfaces as a parse
/// error, not a failed check.
#[test]
fn verify_parse_error_surfaces() {
    let s = scenario(
        r#"{
        "schema": "fenestra/1",
        "description": { "schema": "fenestra/1", "root": { "col": {
            "style": { "bg": "chartreuse" },
            "children": [ { "text": { "content": "Hi" } } ]
        } } },
        "expect": {}
    }"#,
    );
    let err = verify(&s).expect_err("an unknown color role is a parse error");
    assert!(matches!(err, EngineError::Parse(_)), "{err:?}");
}

/// A step whose target does not resolve is a self-explaining step error carrying
/// the access tree.
#[test]
fn verify_step_miss_is_self_explaining() {
    let s = scenario(
        r#"{
        "schema": "fenestra/1",
        "description": { "schema": "fenestra/1", "root": {
            "button": { "label": "Add", "on_click": "add" }
        } },
        "steps": [ { "click": { "role": "button", "name": "Nope" } } ],
        "expect": {}
    }"#,
    );
    match verify(&s).expect_err("the step misses") {
        EngineError::Step { index, tree, .. } => {
            assert_eq!(index, 0);
            assert!(tree.contains("button"), "tree carried for self-correction");
        }
        other => panic!("expected a Step error, got {other:?}"),
    }
}

/// An unrecognized schema tag is a setup error, not a silent default.
#[test]
fn verify_unknown_schema_is_error() {
    let s = scenario(
        r#"{
        "schema": "fenestra/2",
        "description": { "schema": "fenestra/1", "root": { "button": { "label": "Go" } } },
        "expect": {}
    }"#,
    );
    let err = verify(&s).expect_err("unknown schema");
    assert!(matches!(err, EngineError::Scenario(_)), "{err:?}");
}

/// An empty `expect` is a smoke gate: the description parses and renders, so the
/// report is vacuously ok with no checks.
#[test]
fn verify_empty_expect_is_smoke_gate() {
    let s = scenario(
        r#"{
        "schema": "fenestra/1",
        "description": { "schema": "fenestra/1", "root": { "button": { "label": "Go" } } },
        "expect": {}
    }"#,
    );
    let out = verify(&s).expect("scenario runs");
    assert!(out.report.ok, "empty expect is vacuously ok");
    assert!(out.report.checks.is_empty(), "no checks requested");
}

/// The committed login fixture drives a full multi-step flow (focus, type, toggle,
/// submit) and asserts emitted + a11y + aria + two queries in one pass — the
/// unified verify loop on a realistic UI.
#[test]
fn golden_login_scenario_verifies() {
    let s: Scenario = serde_json::from_str(include_str!("scenarios/login.json"))
        .expect("the login fixture parses");
    let out = verify(&s).expect("scenario runs");
    assert!(
        out.report.ok,
        "the login scenario should pass: {:#?}",
        out.report.checks
    );
    assert_eq!(out.report.emitted, vec!["submit".to_string()]);
    // Every requested check is present and green.
    let names: Vec<&str> = out.report.checks.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"emitted"), "{names:?}");
    assert!(names.contains(&"a11y"), "{names:?}");
    assert!(names.contains(&"aria"), "{names:?}");
    assert_eq!(
        out.report
            .checks
            .iter()
            .filter(|c| c.name.starts_with("query:"))
            .count(),
        2,
        "two query checks"
    );
}

// ---- negative paths: each non-screenshot check type must be able to FAIL, so a
// ---- stuck-on-pass regression cannot ship green.

/// The a11y check FAILS for an unlabeled interactive control.
#[test]
fn verify_a11y_flags_unlabeled_control() {
    let s = scenario(
        r#"{
        "schema": "fenestra/1",
        "description": { "schema": "fenestra/1", "root": { "button": { "label": "" } } },
        "size": "200x80",
        "expect": { "a11y": true }
    }"#,
    );
    let out = verify(&s).expect("scenario runs");
    assert!(!out.report.ok, "an unlabeled button should fail a11y");
    let a = out
        .report
        .checks
        .iter()
        .find(|c| c.name == "a11y")
        .expect("an a11y check");
    assert!(!a.ok);
    assert!(
        a.detail.contains("unlabeled"),
        "detail names the problem: {}",
        a.detail
    );
}

/// The aria check FAILS when the expected snapshot line is absent.
#[test]
fn verify_aria_mismatch_fails() {
    let s = scenario(
        r#"{
        "schema": "fenestra/1",
        "description": { "schema": "fenestra/1", "root": { "button": { "label": "Add", "on_click": "add" } } },
        "size": "200x80",
        "expect": { "aria": { "snapshot": "- button \"Nope\"" } }
    }"#,
    );
    let out = verify(&s).expect("scenario runs");
    assert!(!out.report.ok);
    let aria = out
        .report
        .checks
        .iter()
        .find(|c| c.name == "aria")
        .expect("an aria check");
    assert!(!aria.ok && !aria.detail.is_empty());
}

/// The aria `mode` is threaded: a one-line snapshot passes `partial` but fails
/// `strict` against a multi-node tree.
#[test]
fn verify_aria_strict_mode_is_threaded() {
    let desc = r#""description": { "schema": "fenestra/1", "root": { "col": { "children": [
        { "button": { "label": "Add", "on_click": "add" } },
        { "button": { "label": "More", "on_click": "more" } }
    ] } } }, "size": "200x140""#;
    let partial = scenario(&format!(
        r#"{{ "schema": "fenestra/1", {desc},
        "expect": {{ "aria": {{ "snapshot": "- button \"Add\"", "mode": "partial" }} }} }}"#
    ));
    assert!(
        verify(&partial)
            .unwrap()
            .report
            .checks
            .iter()
            .find(|c| c.name == "aria")
            .unwrap()
            .ok,
        "partial matches the subsequence"
    );
    let strict = scenario(&format!(
        r#"{{ "schema": "fenestra/1", {desc},
        "expect": {{ "aria": {{ "snapshot": "- button \"Add\"", "mode": "strict" }} }} }}"#
    ));
    assert!(
        !verify(&strict)
            .unwrap()
            .report
            .checks
            .iter()
            .find(|c| c.name == "aria")
            .unwrap()
            .ok,
        "strict requires the whole tree, not one line"
    );
}

/// A malformed regex in an aria expectation is a setup error, not a failed check.
#[test]
fn verify_aria_bad_regex_is_scenario_error() {
    let s = scenario(
        r#"{
        "schema": "fenestra/1",
        "description": { "schema": "fenestra/1", "root": { "button": { "label": "Add", "on_click": "add" } } },
        "expect": { "aria": { "snapshot": "- button \"(\"", "mode": "regex" } }
    }"#,
    );
    let err = verify(&s).expect_err("a bad regex is a scenario error");
    assert!(
        matches!(&err, EngineError::Scenario(m) if m.contains("expect.aria")),
        "{err:?}"
    );
}

/// A query miss reports the nearest candidates so the author can correct it.
#[test]
fn verify_query_miss_reports_nearest() {
    let s = scenario(
        r#"{
        "schema": "fenestra/1",
        "description": { "schema": "fenestra/1", "root": { "checkbox": { "label": "Done" } } },
        "size": "200x80",
        "expect": { "queries": [ { "selector": { "role": "button" }, "count": 1 } ] }
    }"#,
    );
    let out = verify(&s).expect("scenario runs");
    assert!(!out.report.ok);
    let q = out
        .report
        .checks
        .iter()
        .find(|c| c.name.starts_with("query:"))
        .expect("a query check");
    assert!(!q.ok);
    assert!(q.detail.contains("found 0, expected 1"), "{}", q.detail);
    assert!(
        q.detail.contains("nearest"),
        "names nearby nodes: {}",
        q.detail
    );
}

/// A query with the wrong expected count fails with a count detail.
#[test]
fn verify_query_wrong_count_fails() {
    let s = scenario(
        r#"{
        "schema": "fenestra/1",
        "description": { "schema": "fenestra/1", "root": { "col": { "children": [
            { "button": { "label": "A", "on_click": "a" } },
            { "button": { "label": "B", "on_click": "b" } }
        ] } } },
        "size": "200x140",
        "expect": { "queries": [ { "selector": { "role": "button" }, "count": 5 } ] }
    }"#,
    );
    let out = verify(&s).expect("scenario runs");
    assert!(!out.report.ok);
    let q = out
        .report
        .checks
        .iter()
        .find(|c| c.name.starts_with("query:"))
        .expect("a query check");
    assert!(q.detail.contains("found 2, expected 5"), "{}", q.detail);
}

/// Verifying against a baseline that does not exist (and no `--bless`) is a setup
/// error, not a silent pass.
#[test]
fn verify_missing_baseline_is_scenario_error() {
    let missing = temp_png("definitely_missing");
    let _ = std::fs::remove_file(&missing);
    let s = scenario(&format!(
        r#"{{
        "schema": "fenestra/1",
        "description": {{ "schema": "fenestra/1", "root": {{ "button": {{ "label": "Go" }} }} }},
        "size": "120x60",
        "expect": {{ "screenshot": {{ "baseline": "{}", "tolerance": 3, "budget": 0.002 }} }}
    }}"#,
        missing.to_str().unwrap()
    ));
    let err = verify(&s).expect_err("an unreadable baseline is a setup error");
    assert!(
        matches!(&err, EngineError::Scenario(m) if m.contains("cannot read baseline")),
        "{err:?}"
    );
}

/// A baseline whose dimensions differ from the render fails with a size-mismatch
/// detail and no diff image (a different-size comparison has no pixel overlay).
#[test]
fn verify_dimension_mismatch_reports_size_not_pixels() {
    let baseline = temp_png("dim_mismatch");
    let _ = std::fs::remove_file(&baseline);
    let b = baseline.to_str().unwrap();
    let small = scenario(&format!(
        r#"{{ "schema": "fenestra/1",
        "description": {{ "schema": "fenestra/1", "root": {{ "button": {{ "label": "Go" }} }} }},
        "size": "200x80",
        "expect": {{ "screenshot": {{ "baseline": "{b}", "tolerance": 3, "budget": 0.002 }} }} }}"#
    ));
    bless(&small).expect("bless a 200x80 baseline");
    let tall = scenario(&format!(
        r#"{{ "schema": "fenestra/1",
        "description": {{ "schema": "fenestra/1", "root": {{ "button": {{ "label": "Go" }} }} }},
        "size": "200x120",
        "expect": {{ "screenshot": {{ "baseline": "{b}", "tolerance": 3, "budget": 0.002 }} }} }}"#
    ));
    let out = verify(&tall).expect("scenario runs");
    assert!(!out.report.ok);
    let shot = out
        .report
        .checks
        .iter()
        .find(|c| c.name == "screenshot")
        .expect("a screenshot check");
    assert!(!shot.ok);
    assert!(
        shot.detail.contains("size mismatch"),
        "names the size mismatch: {}",
        shot.detail
    );
    assert!(
        out.diff_png.is_none(),
        "a size mismatch has no per-pixel overlay"
    );
    let _ = std::fs::remove_file(&baseline);
}

/// Screenshot masks are threaded through the scenario: masking the whole canvas
/// makes even a genuinely different render pass.
#[test]
fn verify_screenshot_masks_are_threaded() {
    let baseline = temp_png("masked");
    let _ = std::fs::remove_file(&baseline);
    let b = baseline.to_str().unwrap();
    // Bless the checked (post-click) render.
    let driven = scenario(&format!(
        r#"{{ "schema": "fenestra/1",
        "description": {{ "schema": "fenestra/1", "state": {{ "on": false }}, "root": {{
            "checkbox": {{ "bind": "on", "label": "On", "id": "c" }}
        }} }},
        "size": "200x80",
        "steps": [ {{ "click": {{ "id": "c" }} }} ],
        "expect": {{ "screenshot": {{ "baseline": "{b}", "tolerance": 3, "budget": 0.002 }} }} }}"#
    ));
    bless(&driven).expect("bless the checked baseline");
    // The static (unchecked) render WOULD differ at tol/budget 0, but a full-canvas
    // mask ignores every pixel — so this passes only if masks are wired through.
    let masked = scenario(&format!(
        r#"{{ "schema": "fenestra/1",
        "description": {{ "schema": "fenestra/1", "state": {{ "on": false }}, "root": {{
            "checkbox": {{ "bind": "on", "label": "On", "id": "c" }}
        }} }},
        "size": "200x80",
        "expect": {{ "screenshot": {{ "baseline": "{b}", "tolerance": 0, "budget": 0.0,
            "masks": [ {{ "x": 0, "y": 0, "w": 200, "h": 80 }} ] }} }} }}"#
    ));
    let out = verify(&masked).expect("scenario runs");
    assert!(
        out.report.ok,
        "a full-canvas mask ignores all differences: {:?}",
        out.report.checks
    );
    let _ = std::fs::remove_file(&baseline);
}

/// Steps with an empty `expect` is a post-interaction smoke gate: it drives the
/// harness, emits intents, and is vacuously ok with no checks.
#[test]
fn verify_steps_with_empty_expect_is_smoke_gate() {
    let s = scenario(
        r#"{
        "schema": "fenestra/1",
        "description": { "schema": "fenestra/1", "root": { "button": { "label": "Go", "on_click": "go" } } },
        "size": "200x80",
        "steps": [ { "click": { "role": "button", "name": "Go" } } ],
        "expect": {}
    }"#,
    );
    let out = verify(&s).expect("scenario runs");
    assert!(out.report.ok, "no expectations is vacuously ok");
    assert!(out.report.checks.is_empty());
    assert_eq!(
        out.report.emitted,
        vec!["go".to_string()],
        "the click still emitted its intent"
    );
}
