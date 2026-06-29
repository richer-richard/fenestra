//! The structural engine: access tree (with stable refs) and semantic query
//! (with nearest-candidate suggestions on a miss).

use fenestra_core::Theme;
use fenestra_describe::dto::AccessNodeDto;
use fenestra_describe::format::Description;
use fenestra_describe::inspect::{
    AriaMode, Selector, access_tree, aria_snapshot, check_a11y, focus_order, layout_report,
    match_aria, query, tree_layout_report,
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
    // And it is surfaced in text_contrast_failures even though the theme verdict
    // stays legible — no longer a silent false negative.
    assert!(report.legible, "theme roles are unaffected: {report:?}");
    assert!(
        report
            .text_contrast_failures
            .iter()
            .any(|l| l.text == "ghost"),
        "authored low-contrast text is surfaced: {:?}",
        report.text_contrast_failures
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

#[test]
fn selector_matches_by_state() {
    // Query "the CHECKED checkbox" — by state, not just role/name.
    let d = desc(
        r#"{"schema":"fenestra/1","root":{"col":{"children":[
            {"checkbox":{"checked":true,"label":"A"}},
            {"checkbox":{"checked":false,"label":"B"}}
        ]}}}"#,
    );
    let sel = Selector {
        role: Some("checkbox".into()),
        checked: Some(true),
        ..Default::default()
    };
    let res = query(&d, &Theme::light(), (400, 300), &sel).unwrap();
    assert_eq!(res.matches.len(), 1, "{:?}", res.matches);
    assert_eq!(res.matches[0].name.as_deref(), Some("A"));

    // A state-only selector (no role/name) is a valid, non-empty query.
    let state_only = Selector {
        checked: Some(true),
        ..Default::default()
    };
    let res2 = query(&d, &Theme::light(), (400, 300), &state_only).unwrap();
    assert_eq!(res2.matches.len(), 1, "state-only selector: {:?}", res2.matches);
    assert_eq!(res2.matches[0].name.as_deref(), Some("A"));

    // Range threshold: sliders at or above 0.5.
    let d3 = desc(
        r#"{"schema":"fenestra/1","root":{"col":{"children":[
            {"slider":{"value":0.2}},
            {"slider":{"value":0.8}}
        ]}}}"#,
    );
    let sel3 = Selector {
        role: Some("slider".into()),
        value_gte: Some(0.5),
        ..Default::default()
    };
    let res3 = query(&d3, &Theme::light(), (400, 300), &sel3).unwrap();
    assert_eq!(res3.matches.len(), 1, "{:?}", res3.matches);
    let v = res3.matches[0].value_now.expect("slider value");
    assert!((v - 0.8).abs() < 1e-6, "got {v}");
}

#[test]
fn every_kit_lucide_icon_is_authorable() {
    // Anti-drift: every icon the kit ships must be nameable in fenestra/1 JSON. If
    // the kit gains an icon the describe parser doesn't map, authoring it errors —
    // this catches that the moment it happens.
    use fenestra_kit::icons::lucide;
    for (name, _el) in lucide::all::<()>() {
        let json = format!(r#"{{"schema":"fenestra/1","root":{{"icon":{{"name":"{name}"}}}}}}"#);
        let d: Description = serde_json::from_str(&json).expect("valid json");
        assert!(
            access_tree(&d, &Theme::light(), (64, 64)).is_ok(),
            "kit lucide icon {name:?} must be authorable in fenestra/1 JSON"
        );
    }
}

#[test]
fn tree_layout_report_flags_small_targets_and_offscreen() {
    use fenestra_describe::dto::Bounds;
    fn node(
        ref_: &str,
        role: &str,
        name: Option<&str>,
        focusable: bool,
        b: Bounds,
        children: Vec<AccessNodeDto>,
    ) -> AccessNodeDto {
        AccessNodeDto {
            ref_: ref_.into(),
            role: role.into(),
            name: name.map(Into::into),
            value: None,
            checked: None,
            selected: None,
            value_now: None,
            value_min: None,
            value_max: None,
            mixed: None,
            focusable,
            invalid: false,
            live: false,
            selection: None,
            bounds: b,
            children,
        }
    }
    let b = |x, y, w, h| Bounds { x, y, w, h };
    let tree = node(
        "/",
        "generic",
        None,
        false,
        b(0.0, 0.0, 800.0, 600.0),
        vec![
            node("tiny", "button", Some("tiny"), true, b(10.0, 10.0, 16.0, 16.0), vec![]),
            node("ok", "button", Some("ok"), true, b(10.0, 40.0, 80.0, 32.0), vec![]),
            node("off", "button", Some("offscreen"), true, b(900.0, 10.0, 80.0, 32.0), vec![]),
        ],
    );
    let report = tree_layout_report(&tree, (800, 600));
    assert!(
        report.small_targets.iter().any(|f| f.name.as_deref() == Some("tiny")),
        "tiny target flagged: {:?}",
        report.small_targets
    );
    assert!(
        !report.small_targets.iter().any(|f| f.name.as_deref() == Some("ok")),
        "an adequately sized control is not flagged"
    );
    assert!(
        report.offscreen.iter().any(|f| f.name.as_deref() == Some("offscreen")),
        "off-window node flagged: {:?}",
        report.offscreen
    );
    assert!(
        !report.offscreen.iter().any(|f| f.name.as_deref() == Some("ok")),
        "an on-screen control is not flagged"
    );
}

#[test]
fn layout_report_clean_on_a_normal_layout() {
    // The desc -> tree -> report path runs, and a normal centered button is not
    // off-screen (no false positives on ordinary layouts).
    let d = desc(r#"{"schema":"fenestra/1","root":{"button":{"label":"Save","on_click":"save"}}}"#);
    let report = layout_report(&d, &Theme::light(), (800, 600)).unwrap();
    assert!(report.offscreen.is_empty(), "{:?}", report.offscreen);
}

#[test]
fn focus_order_lists_tabbable_refs_in_order() {
    // Tab visits Email -> Password -> Sign in, in tree order, by stable ref.
    let d = desc(
        r#"{"schema":"fenestra/1","root":{"col":{"children":[
            {"text_input":{"value":"","placeholder":"Email","id":"email"}},
            {"text_input":{"value":"","placeholder":"Password","id":"password"}},
            {"button":{"label":"Sign in","on_click":"submit","id":"signin"}}
        ]}}}"#,
    );
    let order = focus_order(&d, &Theme::light(), (400, 300)).unwrap();
    assert_eq!(order, vec!["email", "password", "signin"], "{order:?}");
}
