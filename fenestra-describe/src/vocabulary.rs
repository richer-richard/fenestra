//! The grammar [`describe_vocabulary`] advertises, generated from the one node
//! registry the format documents. A coherence test renders every advertised
//! node, so the vocabulary can never claim a node the engine cannot build.

use serde::Serialize;

use crate::color::COLOR_ROLES;
use crate::format::SCHEMA_V1;

/// One node type's documentation: its tag, a one-line summary, and a minimal
/// example body (the JSON value that follows the tag key).
#[derive(Debug, Clone, Serialize)]
pub struct NodeDoc {
    /// The externally-tagged variant key, e.g. `"button"`.
    pub tag: String,
    /// A one-line description of the node.
    pub summary: String,
    /// A minimal example body: `{"<tag>": <example>}` is a valid node.
    pub example: String,
}

/// The full grammar an agent can request to learn the format up front.
#[derive(Debug, Clone, Serialize)]
pub struct Vocabulary {
    /// The schema tag every description must carry.
    pub schema: String,
    /// Every node type, with a minimal example.
    pub nodes: Vec<NodeDoc>,
    /// The color roles a `ColorSpec` may name (besides the `oklch` hatch).
    pub color_roles: Vec<String>,
}

/// `(tag, summary, minimal example body)` for every node the parser handles —
/// the single registry the vocabulary is generated from.
const NODE_REGISTRY: &[(&str, &str, &str)] = &[
    ("row", "Horizontal flex container.", r#"{"children":[]}"#),
    ("col", "Vertical flex container.", r#"{"children":[]}"#),
    ("div", "Generic flex container.", r#"{"children":[]}"#),
    ("stack", "Z-stacked / grid container.", r#"{"children":[]}"#),
    ("text", "A run of text.", r#"{"content":"Hello"}"#),
    (
        "button",
        "Activatable button.",
        r#"{"label":"Add","on_click":"add"}"#,
    ),
    (
        "checkbox",
        "Two-state checkbox.",
        r#"{"checked":false,"label":"Accept"}"#,
    ),
    (
        "switch",
        "On/off switch.",
        r#"{"on":false,"label":"Wi-Fi"}"#,
    ),
    (
        "radio",
        "One option of a radio group.",
        r#"{"selected":false,"label":"One"}"#,
    ),
    (
        "slider",
        "Numeric slider over 0.0..=1.0.",
        r#"{"value":0.5}"#,
    ),
    (
        "text_input",
        "Single-line text field.",
        r#"{"value":"","placeholder":"Search"}"#,
    ),
    (
        "text_area",
        "Multi-line text field.",
        r#"{"value":"","placeholder":"Notes"}"#,
    ),
    ("divider", "Themed hairline rule.", r#"{}"#),
    ("spacer", "Flexible empty space.", r#"{}"#),
];

/// Returns the grammar: the schema tag, every node type with a minimal example,
/// and the color roles a `ColorSpec` may name. Generated from one registry, so
/// it cannot drift from what the parser accepts.
pub fn describe_vocabulary() -> Vocabulary {
    Vocabulary {
        schema: SCHEMA_V1.to_string(),
        nodes: NODE_REGISTRY
            .iter()
            .map(|(tag, summary, example)| NodeDoc {
                tag: (*tag).to_string(),
                summary: (*summary).to_string(),
                example: (*example).to_string(),
            })
            .collect(),
        color_roles: COLOR_ROLES.iter().map(|r| (*r).to_string()).collect(),
    }
}
