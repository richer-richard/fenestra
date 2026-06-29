//! The structural engine: access tree (with stable refs) and semantic query
//! (with nearest-candidate suggestions on a miss).

use fenestra_core::Theme;
use fenestra_describe::dto::AccessNodeDto;
use fenestra_describe::format::Description;
use fenestra_describe::inspect::{
    AriaMode, Selector, access_tree, aria_snapshot, check_a11y, match_aria, query,
};

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
fn access_tree_has_button_and_checkbox_with_refs() {
    let tree = access_tree(&desc(FORM), &Theme::light(), (400, 300)).unwrap();
    let mut all = Vec::new();
    roles(&tree, &mut all);
    assert!(all.iter().any(|r| r == "button"), "{all:?}");
    assert!(all.iter().any(|r| r == "checkbox"), "{all:?}");
    // Root has a structural ref.
    assert_eq!(tree.ref_, "/");
}

#[test]
fn query_by_role_finds_the_button() {
    let sel = Selector {
        role: Some("button".into()),
        ..Default::default()
    };
    let res = query(&desc(FORM), &Theme::light(), (400, 300), &sel).unwrap();
    assert_eq!(res.matches.len(), 1, "{:?}", res.matches);
    assert_eq!(res.matches[0].name.as_deref(), Some("Add"));
    assert!(res.nearest.is_empty());
}

#[test]
fn query_miss_returns_nearest_candidates() {
    let sel = Selector {
        role: Some("slider".into()),
        ..Default::default()
    };
    let res = query(&desc(FORM), &Theme::light(), (400, 300), &sel).unwrap();
    assert!(res.matches.is_empty());
    assert!(
        !res.nearest.is_empty(),
        "a miss should suggest nearby nodes, got none"
    );
    // The suggestions are signal-bearing (named or roled), not generic boxes.
    assert!(
        res.nearest
            .iter()
            .all(|n| n.role != "generic" || n.name.is_some())
    );
}

#[test]
fn empty_selector_is_rejected() {
    let res = query(
        &desc(FORM),
        &Theme::light(),
        (400, 300),
        &Selector::default(),
    );
    assert!(res.is_err());
}

#[test]
fn aria_snapshot_and_partial_and_strict() {
    let d = desc(FORM);
    let snap = aria_snapshot(&d, &Theme::light(), (400, 300)).unwrap();
    assert!(snap.contains(r#"button "Add""#), "{snap}");

    // Partial: a subset of lines matches.
    let partial = match_aria(
        &d,
        &Theme::light(),
        (400, 300),
        r#"- button "Add""#,
        AriaMode::Partial,
    )
    .unwrap();
    assert!(partial.ok, "{}", partial.diff);

    // Strict: one line is not the whole tree.
    let strict = match_aria(
        &d,
        &Theme::light(),
        (400, 300),
        r#"- button "Add""#,
        AriaMode::Strict,
    )
    .unwrap();
    assert!(!strict.ok);
    assert!(!strict.diff.is_empty());
}

#[test]
fn aria_regex_match() {
    let d = desc(FORM);
    let ok = match_aria(
        &d,
        &Theme::light(),
        (400, 300),
        r#"- button "A\w+""#,
        AriaMode::Regex,
    )
    .unwrap();
    assert!(ok.ok, "{}", ok.diff);
    let miss = match_aria(
        &d,
        &Theme::light(),
        (400, 300),
        r#"- button "Z\w+""#,
        AriaMode::Regex,
    )
    .unwrap();
    assert!(!miss.ok);
    // An invalid pattern is a reported error, not a panic.
    let bad = match_aria(
        &d,
        &Theme::light(),
        (400, 300),
        "- button (",
        AriaMode::Regex,
    );
    assert!(bad.is_err());
}

#[test]
fn check_a11y_clean_form_is_labeled_and_legible() {
    let report = check_a11y(&desc(FORM), &Theme::light(), (400, 300)).unwrap();
    assert!(
        report.unlabeled.is_empty(),
        "button + checkbox are labeled: {:?}",
        report.unlabeled
    );
    assert!(
        !report.node_legibility.is_empty(),
        "the labels should be measured"
    );
    assert!(
        report.legible,
        "the default theme renders legibly: {report:?}"
    );
}

#[test]
fn check_a11y_flags_unlabeled_button() {
    let d = desc(r#"{"schema":"fenestra/1","root":{"button":{"label":""}}}"#);
    let report = check_a11y(&d, &Theme::light(), (300, 120)).unwrap();
    assert_eq!(
        report.unlabeled.len(),
        1,
        "an empty-label button is unlabeled"
    );
    assert_eq!(report.unlabeled[0].role, "button");
}

#[test]
fn node_legibility_catches_custom_low_contrast() {
    // A custom near-invisible text color (not a theme role): the theme's contrast
    // contract cannot see it, but per-node legibility measures it as it renders.
    let d = desc(
        r#"{"schema":"fenestra/1","root":{"col":{"style":{"bg":{"oklch":[0.97,0.0,0.0]}},"children":[
            {"text":{"content":"ghost","style":{"color":{"oklch":[0.95,0.0,0.0]}}}}
        ]}}}"#,
    );
    let report = check_a11y(&d, &Theme::light(), (300, 120)).unwrap();
    let ghost = report
        .node_legibility
        .iter()
        .find(|l| l.text == "ghost")
        .expect("the text was measured");
    assert!(
        !ghost.passes_apca,
        "custom low-contrast text should fail strict APCA: {ghost:?}"
    );
}

#[test]
fn access_tree_carries_numeric_widget_values() {
    // An agent must read range-widget VALUES off the typed tree, not by regexing
    // the aria string. Slider, meter (and spinbutton/progressbar) carry value/
    // min/max into the DTO.
    fn flatten(n: &AccessNodeDto, out: &mut Vec<AccessNodeDto>) {
        out.push(n.clone());
        for c in &n.children {
            flatten(c, out);
        }
    }
    let d = desc(
        r#"{"schema":"fenestra/1","root":{"col":{"children":[
            {"slider":{"value":0.5}},
            {"meter":{"value":62,"min":0,"max":100,"label":"Storage"}}
        ]}}}"#,
    );
    let tree = access_tree(&d, &Theme::light(), (400, 300)).unwrap();
    let mut flat = Vec::new();
    flatten(&tree, &mut flat);
    let slider = flat
        .iter()
        .find(|n| n.role == "slider")
        .expect("a slider node");
    assert_eq!(slider.value_now, Some(0.5), "{slider:?}");
    assert_eq!(slider.value_min, Some(0.0));
    assert_eq!(slider.value_max, Some(1.0));
    let meter = flat.iter().find(|n| n.role == "meter").expect("a meter node");
    assert_eq!(meter.value_now, Some(62.0), "{meter:?}");
    assert_eq!(meter.value_min, Some(0.0));
    assert_eq!(meter.value_max, Some(100.0));
}
