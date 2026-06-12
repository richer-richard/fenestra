//! Scenario scripts end to end: JSON drives a real app — clicks, typing,
//! assertions, screenshots — and failures explain themselves.

use fenestra_core::{App, Element, Theme, col, raw_input, row, text};
use fenestra_shell::{Harness, run_scenario};

#[derive(Default)]
struct Todo {
    draft: String,
    items: Vec<String>,
}

#[derive(Clone)]
enum Msg {
    Draft(String),
    Add,
}

impl App for Todo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Draft(s) => self.draft = s,
            Msg::Add => {
                if !self.draft.is_empty() {
                    self.items.push(std::mem::take(&mut self.draft));
                }
            }
        }
    }

    fn view(&self) -> Element<Msg> {
        col().p(16.0).gap(8.0).children([
            row().gap(8.0).children([
                raw_input(&self.draft, "What needs doing?")
                    .on_input(|s| Msg::Draft(s.to_owned()))
                    .on_key(|k| matches!(k.key, fenestra_core::Key::Enter).then_some(Msg::Add))
                    .id("draft"),
                fenestra_core::div()
                    .p(8.0)
                    .focusable(true)
                    .on_click(Msg::Add)
                    .semantics(fenestra_core::Semantics::Button)
                    .label("Add")
                    .child(text("Add")),
            ]),
            col()
                .gap(4.0)
                .children(self.items.iter().map(|item| text(item.clone()))),
        ])
    }
}

fn shots_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("fenestra-scn-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn json_drives_the_app_and_asserts() {
    let mut h = Harness::new(Todo::default(), Theme::light(), (480, 320));
    let dir = shots_dir("todo");
    let report = run_scenario(
        &mut h,
        r#"{"steps": [
            {"tab": 1},
            {"type": "buy milk"},
            {"assert": {"value": {"target": {"id": "draft"}, "equals": "buy milk"}}},
            {"key": "enter"},
            {"assert": {"exists": {"label": "buy milk"}}},
            {"assert": {"value": {"target": {"id": "draft"}, "equals": ""}}},
            {"click": {"role": "button", "name": "Add"}},
            {"assert": {"count": {"target": {"label": "buy milk"}, "equals": 1}}},
            {"shot": "done"}
        ]}"#,
        &dir,
    )
    .expect("scenario runs");
    assert_eq!(report.steps_run, 9);
    assert_eq!(report.shots.len(), 1);
    assert!(dir.join("done.png").exists());
    assert_eq!(h.app().items, vec!["buy milk".to_owned()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failures_carry_the_step_and_the_tree() {
    let mut h = Harness::new(Todo::default(), Theme::light(), (480, 320));
    let err = run_scenario(
        &mut h,
        r#"{"steps": [
            {"tab": 1},
            {"click": {"role": "button", "name": "Remove"}}
        ]}"#,
        shots_dir("missing"),
    )
    .expect_err("target is missing");
    assert_eq!(err.step, Some(1));
    assert!(err.message.contains("no node matches"), "{}", err.message);
    assert!(
        err.message.contains("accessibility tree"),
        "tree included: {}",
        err.message
    );
}

#[test]
fn typos_fail_loudly_not_silently() {
    let mut h = Harness::new(Todo::default(), Theme::light(), (480, 320));
    // "clickk" is not a verb — deny_unknown_fields turns it into a
    // parse error instead of a silently skipped step.
    let err = run_scenario(
        &mut h,
        r#"{"steps": [{"clickk": {"id": "draft"}}]}"#,
        shots_dir("typo"),
    )
    .expect_err("unknown verb");
    assert_eq!(err.step, None);
    assert!(
        err.message.contains("invalid scenario JSON"),
        "{}",
        err.message
    );

    let err = run_scenario(
        &mut h,
        r#"{"steps": [{"click": {"role": "buton"}}]}"#,
        shots_dir("role"),
    )
    .expect_err("unknown role");
    assert_eq!(err.step, Some(0));
    assert!(err.message.contains("unknown role"), "{}", err.message);
}

#[test]
fn failed_asserts_explain_the_difference() {
    let mut h = Harness::new(Todo::default(), Theme::light(), (480, 320));
    let err = run_scenario(
        &mut h,
        r#"{"steps": [
            {"tab": 1},
            {"type": "x"},
            {"assert": {"value": {"target": {"id": "draft"}, "equals": "y"}}}
        ]}"#,
        shots_dir("assert"),
    )
    .expect_err("value differs");
    assert_eq!(err.step, Some(2));
    assert!(err.message.contains("\"x\" != \"y\""), "{}", err.message);
}
