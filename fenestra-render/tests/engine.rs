//! The cli engine: render (tree + png + warnings), interact (emitted intents +
//! after-tree, self-explaining misses), and match_screenshot (tolerance + masks).
//! These exercise the render path, so they need a GPU.

use fenestra_describe::dto::{AccessNodeDto, Bounds};
use fenestra_describe::format::Description;
use fenestra_describe::inspect::Selector;
use fenestra_render::engine::EngineError;
use fenestra_render::{Step, interact, match_screenshot, render, resolve_theme};

const FORM: &str = r#"{"schema":"fenestra/1","root":{"col":{"children":[
    {"button":{"label":"Add","on_click":"add"}},
    {"checkbox":{"checked":false,"label":"Done"}}
]}}}"#;

fn desc(json: &str) -> Description {
    serde_json::from_str(json).expect("valid description")
}

fn roles(node: &AccessNodeDto, out: &mut Vec<String>) {
    out.push(node.role.clone());
    for child in &node.children {
        roles(child, out);
    }
}

#[test]
fn render_returns_tree_png_and_warnings() {
    let theme = resolve_theme(None).unwrap();
    let out = render(&desc(FORM), &theme, (400, 300)).unwrap();
    let mut all = Vec::new();
    roles(&out.tree, &mut all);
    assert!(all.iter().any(|r| r == "button"), "{all:?}");
    assert_eq!(out.png.dimensions(), (400, 300));
    assert!(out.warnings.unlabeled.is_empty(), "the form is labeled");
}

#[test]
fn interact_emits_intent_and_returns_tree() {
    let theme = resolve_theme(None).unwrap();
    let steps = vec![Step::Click(Selector {
        role: Some("button".into()),
        name: Some("Add".into()),
        ..Default::default()
    })];
    let out = interact(&desc(FORM), &theme, (400, 300), &steps, false).unwrap();
    assert_eq!(out.emitted, vec!["add".to_string()]);
    assert!(out.png.is_none());
}

#[test]
fn interact_miss_is_self_explaining() {
    let theme = resolve_theme(None).unwrap();
    let steps = vec![Step::Click(Selector {
        role: Some("button".into()),
        name: Some("Nope".into()),
        ..Default::default()
    })];
    let err = interact(&desc(FORM), &theme, (400, 300), &steps, false)
        .err()
        .unwrap();
    match err {
        EngineError::Step { index, tree, .. } => {
            assert_eq!(index, 0);
            assert!(
                tree.contains("button"),
                "the tree should be included: {tree}"
            );
        }
        EngineError::Parse(e) => panic!("expected a Step error, got Parse: {e:?}"),
        EngineError::Scenario(m) => panic!("expected a Step error, got Scenario: {m}"),
    }
}

#[test]
fn match_screenshot_identical_is_ok() {
    let theme = resolve_theme(None).unwrap();
    let baseline = render(&desc(FORM), &theme, (400, 300)).unwrap().png;
    // Platform tolerance (3/255, 0.2%) — the same the goldens use: exact-byte
    // comparison is flaky under parallel access to the shared GPU. Byte-level
    // determinism is proven by describe's single-threaded determinism test.
    let diff = match_screenshot(&desc(FORM), &theme, (400, 300), &baseline, 3, 0.002, &[]).unwrap();
    assert!(
        diff.ok,
        "identical render should match within tolerance: {} differ",
        diff.differing
    );
}

#[test]
fn match_screenshot_mask_ignores_region() {
    let theme = resolve_theme(None).unwrap();
    let baseline = render(&desc(FORM), &theme, (400, 300)).unwrap().png;
    let other = r#"{"schema":"fenestra/1","root":{"col":{"children":[
        {"button":{"label":"Different","on_click":"x"}}
    ]}}}"#;
    let masks = vec![Bounds {
        x: 0.0,
        y: 0.0,
        w: 400.0,
        h: 300.0,
    }];
    let masked =
        match_screenshot(&desc(other), &theme, (400, 300), &baseline, 0, 0.0, &masks).unwrap();
    assert!(masked.ok, "fully masked comparison is ok");
    let unmasked =
        match_screenshot(&desc(other), &theme, (400, 300), &baseline, 0, 0.0, &[]).unwrap();
    assert!(!unmasked.ok, "unmasked difference is not ok");
    assert!(unmasked.diff_png.is_some());
}

/// Finds the first node of `role` in the tree.
fn find_role<'a>(node: &'a AccessNodeDto, role: &str) -> Option<&'a AccessNodeDto> {
    if node.role == role {
        return Some(node);
    }
    node.children.iter().find_map(|c| find_role(c, role))
}

#[test]
fn bound_input_echoes_typed_text() {
    let theme = resolve_theme(None).unwrap();
    let d = desc(
        r#"{"schema":"fenestra/1","state":{"name":""},"root":{"text_input":{"bind":"name","placeholder":"name","id":"inp"}}}"#,
    );
    let steps = vec![
        Step::Click(Selector {
            id: Some("inp".into()),
            ..Default::default()
        }),
        Step::Type("Ada".into()),
    ];
    let out = interact(&d, &theme, (300, 120), &steps, false).unwrap();
    // The after-tree textbox value reflects the typed text — the Elm wall is gone.
    let textbox = find_role(&out.tree, "textbox").expect("a textbox");
    assert_eq!(
        textbox.value.as_deref(),
        Some("Ada"),
        "bound input should echo typed text"
    );
    // And the runtime state carries it.
    assert_eq!(out.state.get("name").and_then(|v| v.as_str()), Some("Ada"));
}

#[test]
fn bound_checkbox_toggles_state_and_tree() {
    let theme = resolve_theme(None).unwrap();
    let d = desc(
        r#"{"schema":"fenestra/1","state":{"agreed":false},"root":{"checkbox":{"bind":"agreed","label":"Agree","id":"cb"}}}"#,
    );
    let steps = vec![Step::Click(Selector {
        id: Some("cb".into()),
        ..Default::default()
    })];
    let out = interact(&d, &theme, (300, 120), &steps, false).unwrap();
    assert_eq!(
        out.state.get("agreed").and_then(|v| v.as_bool()),
        Some(true),
        "clicking a bound checkbox flips its state"
    );
    let checkbox = find_role(&out.tree, "checkbox").expect("a checkbox");
    assert_eq!(
        checkbox.checked,
        Some(true),
        "the after-tree checkbox is checked"
    );
}
