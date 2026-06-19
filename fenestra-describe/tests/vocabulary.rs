//! `describe_vocabulary` must stay in lockstep with the parser: every advertised
//! node parses and renders, and the list covers every `Node` variant.

use std::collections::BTreeSet;

use fenestra_core::Theme;
use fenestra_describe::format::{Description, Node};
use fenestra_describe::parse::{to_element, validate};
use fenestra_describe::vocabulary::describe_vocabulary;

#[test]
fn every_advertised_node_parses_and_renders() {
    for doc in describe_vocabulary().nodes {
        let json = format!(
            r#"{{"schema":"fenestra/1","root":{{"{}":{}}}}}"#,
            doc.tag, doc.example
        );
        assert!(
            validate(&json).is_ok(),
            "{} failed validate: {:?}",
            doc.tag,
            validate(&json).err()
        );
        let desc: Description = serde_json::from_str(&json).expect("example deserializes");
        assert!(
            to_element(&desc, &Theme::light()).is_ok(),
            "{} failed to_element",
            doc.tag
        );
        assert_eq!(
            tag_of(&desc.root),
            doc.tag,
            "example body is not a {}",
            doc.tag
        );
    }
}

#[test]
fn vocabulary_covers_every_node_variant() {
    let tags: BTreeSet<String> = describe_vocabulary()
        .nodes
        .into_iter()
        .map(|n| n.tag)
        .collect();
    let expected: BTreeSet<String> = [
        "row",
        "col",
        "div",
        "stack",
        "text",
        "button",
        "checkbox",
        "switch",
        "radio",
        "slider",
        "text_input",
        "text_area",
        "divider",
        "spacer",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    assert_eq!(tags, expected);
}

/// Exhaustive over `Node`: a new variant forces a new arm here, prompting a
/// matching vocabulary entry and keeping the grammar honest.
fn tag_of(n: &Node) -> &'static str {
    match n {
        Node::Row(_) => "row",
        Node::Col(_) => "col",
        Node::Div(_) => "div",
        Node::Stack(_) => "stack",
        Node::Text(_) => "text",
        Node::Button(_) => "button",
        Node::Checkbox(_) => "checkbox",
        Node::Switch(_) => "switch",
        Node::Radio(_) => "radio",
        Node::Slider(_) => "slider",
        Node::TextInput(_) => "text_input",
        Node::TextArea(_) => "text_area",
        Node::Divider(_) => "divider",
        Node::Spacer(_) => "spacer",
    }
}
