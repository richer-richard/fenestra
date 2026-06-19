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
    }
}

#[test]
fn match_screenshot_identical_is_ok() {
    let theme = resolve_theme(None).unwrap();
    let baseline = render(&desc(FORM), &theme, (400, 300)).unwrap().png;
    let diff = match_screenshot(&desc(FORM), &theme, (400, 300), &baseline, 0, 0.0, &[]).unwrap();
    assert!(
        diff.ok,
        "identical render should match: {} differ",
        diff.differing
    );
    assert_eq!(diff.differing, 0);
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
