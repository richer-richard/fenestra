//! `aria-invalid` surfaces end-to-end: a control marked invalid in a description
//! shows up as `[invalid]` in the aria snapshot and `invalid: true` in the typed
//! access tree — so a scenario can verify a control's validity state, not just
//! its visual ring.

use fenestra_core::Theme;
use fenestra_describe::dto::AccessNodeDto;
use fenestra_describe::format::Description;
use fenestra_describe::inspect::{access_tree, aria_snapshot};

fn desc(json: &str) -> Description {
    serde_json::from_str(json).expect("valid description")
}

fn find<'a>(node: &'a AccessNodeDto, role: &str) -> Option<&'a AccessNodeDto> {
    if node.role == role {
        return Some(node);
    }
    node.children.iter().find_map(|c| find(c, role))
}

#[test]
fn invalid_text_input_surfaces_in_aria_and_tree() {
    let d = desc(
        r#"{"schema":"fenestra/1","root":{"text_input":{"value":"nope","invalid":true,"placeholder":"Email"}}}"#,
    );

    let aria = aria_snapshot(&d, &Theme::light(), (300, 120)).unwrap();
    assert!(
        aria.contains("[invalid]"),
        "aria should mark the control invalid:\n{aria}"
    );

    let tree = access_tree(&d, &Theme::light(), (300, 120)).unwrap();
    let tb = find(&tree, "textbox").expect("a textbox");
    assert!(tb.invalid, "the access tree carries invalid: {tb:?}");
}

#[test]
fn valid_text_input_is_not_invalid() {
    let d = desc(
        r#"{"schema":"fenestra/1","root":{"text_input":{"value":"ok","placeholder":"Email"}}}"#,
    );

    let aria = aria_snapshot(&d, &Theme::light(), (300, 120)).unwrap();
    assert!(
        !aria.contains("[invalid]"),
        "a valid control is unmarked:\n{aria}"
    );

    let tree = access_tree(&d, &Theme::light(), (300, 120)).unwrap();
    let tb = find(&tree, "textbox").expect("a textbox");
    assert!(!tb.invalid);
    // The serialized tree omits the flag entirely when false (skip_serializing_if).
    let json = serde_json::to_string(&tree).unwrap();
    assert!(
        !json.contains("invalid"),
        "false invalid is omitted: {json}"
    );
}
