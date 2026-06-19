//! The structural engine: access tree (with stable refs) and semantic query
//! (with nearest-candidate suggestions on a miss).

use fenestra_core::Theme;
use fenestra_describe::dto::AccessNodeDto;
use fenestra_describe::format::Description;
use fenestra_describe::inspect::{Selector, access_tree, query};

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
