//! The grammar [`describe_vocabulary`] advertises, generated from the one node
//! registry the format documents. A coherence test builds every advertised
//! node, so the vocabulary can never claim a node the engine cannot build.

use serde::Serialize;

use crate::color::COLOR_ROLES;
use crate::format::SCHEMA_V1;

/// One node type's documentation: its tag, a one-line summary, and a minimal
/// example body (the JSON value that follows the tag key).
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct NodeDoc {
    /// The externally-tagged variant key, e.g. `"button"`.
    pub tag: String,
    /// A one-line description of the node.
    pub summary: String,
    /// A minimal example body: `{"<tag>": <example>}` is a valid node.
    pub example: String,
}

/// The full grammar an agent can request to learn the format up front.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
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
    // ── Layout containers ─────────────────────────────────────────────────
    ("row", "Horizontal flex container.", r#"{"children":[]}"#),
    ("col", "Vertical flex container.", r#"{"children":[]}"#),
    ("div", "Generic flex container.", r#"{"children":[]}"#),
    ("stack", "Z-stacked / grid container.", r#"{"children":[]}"#),
    (
        "card",
        "Raised-surface content card (vertical flex, SP6 padding, rounded).",
        r#"{"children":[]}"#,
    ),
    // ── Text ──────────────────────────────────────────────────────────────
    (
        "text",
        "A run of text. Supports `on_click` for clickable labels.",
        r#"{"content":"Hello"}"#,
    ),
    // ── Form controls ──────────────────────────────────────────────────────
    (
        "button",
        "Activatable button. `variant`: primary | secondary | ghost | danger. `bind` a bool state key for toggle behavior.",
        r#"{"label":"Add","on_click":"add","variant":"primary"}"#,
    ),
    (
        "checkbox",
        "Two-state checkbox. `bind` a root `state` key for a framework-owned toggle.",
        r#"{"checked":false,"label":"Accept","bind":"accepted"}"#,
    ),
    (
        "switch",
        "On/off switch. `bind` a root `state` key for a framework-owned toggle.",
        r#"{"on":false,"label":"Wi-Fi","bind":"wifi"}"#,
    ),
    (
        "radio",
        "One option of a radio group. Use `group` + `value` for group binding.",
        r#"{"selected":false,"label":"One","group":"lang","value":"one"}"#,
    ),
    (
        "slider",
        "Numeric slider over 0.0..=1.0 with an optional `step`. `bind` a root `state` key so dragging updates it.",
        r#"{"value":0.5,"step":0.1,"bind":"volume"}"#,
    ),
    (
        "text_input",
        "Single-line text field. `bind` a root `state` key so typing echoes back.",
        r#"{"value":"","placeholder":"Search","bind":"query"}"#,
    ),
    (
        "text_area",
        "Multi-line text field. `bind` a root `state` key so typing echoes back.",
        r#"{"value":"","placeholder":"Notes","bind":"notes"}"#,
    ),
    (
        "select",
        "Drop-down selector. `bind` a root `state` number key for the selected index.",
        r#"{"options":["One","Two"],"selected":0}"#,
    ),
    (
        "spin_button",
        "Compact number stepper (value between − / + buttons). `on_decrement`/`on_increment` fire intents; `can_decrement`/`can_increment` gate the ends.",
        r#"{"value":"3"}"#,
    ),
    // ── Navigation ────────────────────────────────────────────────────────
    (
        "tabs",
        "Underline tab strip. `bind` a root `state` number key for the active index.",
        r#"{"labels":["Overview","Settings"],"active":0}"#,
    ),
    (
        "segmented",
        "Compact single-select view switcher. `bind` a root `state` number key for the active index.",
        r#"{"labels":["List","Board"],"active":0}"#,
    ),
    (
        "breadcrumbs",
        "Breadcrumb trail; the last item is the current page. `bind`/`on_change` fire with the selected ancestor index.",
        r#"{"items":["Home","Library","Charts"]}"#,
    ),
    (
        "pagination",
        "Numbered pagination strip. `bind` a root `state` number key for the current page (1-based).",
        r#"{"count":10,"page":3}"#,
    ),
    (
        "stepper",
        "Horizontal step indicator. `bind` a root `state` number key for the active step (0-based).",
        r#"{"steps":["Account","Shipping","Payment"],"current":1}"#,
    ),
    // ── Display / feedback ─────────────────────────────────────────────────
    (
        "badge",
        "Status pill. `status`: accent (default) | danger | warning | success.",
        r#"{"label":"New","status":"accent"}"#,
    ),
    (
        "callout",
        "Status callout: tinted background, status border, icon, and message.",
        r#"{"status":"warning","message":"Trial ends in 3 days."}"#,
    ),
    (
        "stat_card",
        "Metric card with muted label, large value, and optional `delta` badge.",
        r#"{"label":"Revenue","value":"$48k","delta":"+12%","delta_status":"success"}"#,
    ),
    (
        "avatar",
        "Circular initials avatar in the accent tint.",
        r#"{"initials":"JD"}"#,
    ),
    (
        "status",
        "Status dot + label indicator. `live:true` adds a pulsing sonar ring.",
        r#"{"label":"Operational","status":"success"}"#,
    ),
    (
        "kbd",
        "Keyboard key-cap chord. `raised:true` for 3D keycap style. Modifier names map to platform glyphs.",
        r#"{"keys":["cmd","K"]}"#,
    ),
    (
        "progress",
        "4px progress bar, `value` 0..=1. `indeterminate:true` for the sweep animation.",
        r#"{"value":0.5}"#,
    ),
    (
        "meter",
        "Measurement bar within `min`..=`max`. With `low`/`high`/`optimum` set, the fill colours by zone (success/warning/danger). `bind` a state number for the value.",
        r#"{"value":62,"min":0,"max":100,"label":"Storage"}"#,
    ),
    (
        "accordion",
        "Stack of expandable disclosure sections. `open` (or `bind` a state number) selects the expanded item by index.",
        r#"{"items":[{"title":"Shipping","body":{"text":{"content":"Ships in two days."}}}],"open":0}"#,
    ),
    (
        "spinner",
        "Rotating arc activity indicator (no parameters).",
        r#"{}"#,
    ),
    (
        "skeleton",
        "Loading placeholder. `kind`: rect (default) | circle | text. `rect`/`circle` use `w`/`h`; `text` uses `lines`.",
        r#"{"w":120,"h":16,"kind":"rect"}"#,
    ),
    (
        "icon",
        "Named Lucide icon (24x24, stroked). Known names: alert-triangle, arrow-left, arrow-right, bell, calendar, check, chevron-down, chevron-left, chevron-right, chevron-up, clock, copy, download, external-link, eye, file, folder, home, info, link, lock, log-out, mail, menu, minus, moon, pencil, plus, refresh-cw, save, search, settings, star, sun, trash, upload, user, x.",
        r#"{"name":"plus"}"#,
    ),
    // ── Overlays ──────────────────────────────────────────────────────────
    (
        "modal",
        "Centered modal dialog with title, children, and optional `on_close` intent.",
        r#"{"title":"Confirm","on_close":"dismiss","children":[]}"#,
    ),
    (
        "tooltip",
        "Hover tooltip wrapping a `target` node.",
        r#"{"label":"Helpful info","target":{"text":{"content":"Hover me"}}}"#,
    ),
    // ── Decoration ────────────────────────────────────────────────────────
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
