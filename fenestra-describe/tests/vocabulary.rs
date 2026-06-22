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
        // Layout containers
        "row",
        "col",
        "div",
        "stack",
        "card",
        // Text
        "text",
        // Form controls
        "button",
        "checkbox",
        "switch",
        "radio",
        "slider",
        "text_input",
        "text_area",
        "select",
        // Navigation
        "tabs",
        "segmented",
        "breadcrumbs",
        "pagination",
        "stepper",
        // Display / feedback
        "badge",
        "callout",
        "stat_card",
        "avatar",
        "status",
        "kbd",
        "progress",
        "spinner",
        "skeleton",
        "icon",
        // Overlays
        "modal",
        "tooltip",
        // Decoration
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
        // Layout containers
        Node::Row(_) => "row",
        Node::Col(_) => "col",
        Node::Div(_) => "div",
        Node::Stack(_) => "stack",
        Node::Card(_) => "card",
        // Text
        Node::Text(_) => "text",
        // Form controls
        Node::Button(_) => "button",
        Node::Checkbox(_) => "checkbox",
        Node::Switch(_) => "switch",
        Node::Radio(_) => "radio",
        Node::Slider(_) => "slider",
        Node::TextInput(_) => "text_input",
        Node::TextArea(_) => "text_area",
        Node::Select(_) => "select",
        // Navigation
        Node::Tabs(_) => "tabs",
        Node::Segmented(_) => "segmented",
        Node::Breadcrumbs(_) => "breadcrumbs",
        Node::Pagination(_) => "pagination",
        Node::Stepper(_) => "stepper",
        // Display / feedback
        Node::Badge(_) => "badge",
        Node::Callout(_) => "callout",
        Node::StatCard(_) => "stat_card",
        Node::Avatar(_) => "avatar",
        Node::Status(_) => "status",
        Node::Kbd(_) => "kbd",
        Node::Progress(_) => "progress",
        Node::Spinner(_) => "spinner",
        Node::Skeleton(_) => "skeleton",
        Node::Icon(_) => "icon",
        // Overlays
        Node::Modal(_) => "modal",
        Node::Tooltip(_) => "tooltip",
        // Decoration
        Node::Divider(_) => "divider",
        Node::Spacer(_) => "spacer",
    }
}
