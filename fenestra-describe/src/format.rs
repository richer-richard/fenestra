//! The `Description` format: a JSON mirror of a fenestra element tree.
//!
//! A description is `{ "schema": "fenestra/1", "root": <node> }`. Each node is
//! an externally tagged object — exactly one variant key, e.g.
//! `{ "col": { "children": [...] } }` or `{ "button": { "label": "Add" } }`.
//!
//! Three rules make the format safe for machine authors:
//! - **Strict at author time**: every struct is `deny_unknown_fields`, so a
//!   typo (`gapp` for `gap`) is an error, not a silently dropped field.
//! - **Colors by role**: a [`ColorSpec`] is a theme role name (`"surface"`,
//!   `"accent"`, …) or an explicit OKLCH escape hatch — never a raw hex string.
//! - **Inert handlers**: every handler is a plain intent [`String`], carrying no
//!   logic or expressions across the boundary.
//!
//! Style lives in a nested `"style"` object rather than flattened onto the node,
//! because serde's `deny_unknown_fields` and `#[serde(flatten)]` are mutually
//! exclusive and strictness wins.

use serde::{Deserialize, Serialize};

/// The only schema tag v1 accepts. Additive minor revisions keep this string;
/// a breaking change would bump it.
pub const SCHEMA_V1: &str = "fenestra/1";

/// A complete serialized UI: a schema tag, the root node, and an optional theme.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Description {
    /// Must equal [`SCHEMA_V1`]. Load-bearing from day one.
    pub schema: String,
    /// The root of the element tree.
    pub root: Node,
    /// Optional theme recipe: a `fenestra_core::ThemeSpec` JSON object, or a
    /// preset selector `{ "preset": "light" | "dark" }`. Resolved by the
    /// `fenestra-render` engine, not here, so this crate stays render-agnostic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<serde_json::Value>,
}

/// One node in the tree. Externally tagged: each node object carries exactly
/// one variant key. Unknown keys are rejected as unknown variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Node {
    /// A horizontal flex container.
    Row(Container),
    /// A vertical flex container.
    Col(Container),
    /// A generic (row) flex container.
    Div(Container),
    /// A z-stacked / grid container.
    Stack(Container),
    /// A run of text.
    Text(TextNode),
    /// An activatable button.
    Button(ButtonNode),
    /// A two-state checkbox.
    Checkbox(CheckboxNode),
    /// An on/off switch.
    Switch(SwitchNode),
    /// One option of a radio group.
    Radio(RadioNode),
    /// A numeric slider.
    Slider(SliderNode),
    /// A single-line editable text field.
    TextInput(InputNode),
    /// A multi-line editable text field.
    TextArea(InputNode),
    /// A themed hairline rule.
    Divider(Leaf),
    /// Flexible empty space.
    Spacer(Leaf),
}

/// A flex/grid container: children plus a style block.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Container {
    /// Child nodes, in paint order.
    #[serde(default)]
    pub children: Vec<Node>,
    /// Layout and appearance.
    #[serde(default)]
    pub style: Style,
    /// Stable key (the query/test-id escape hatch).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Degrade-to-text trace if this node ever fails to realize. Reserved; the
    /// parser never panics, so today this only annotates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A run of text and its type styling.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextNode {
    /// The text to display.
    pub content: String,
    /// Type and color styling (`color`, `size_px`, `weight`).
    #[serde(default)]
    pub style: Style,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// An activatable button.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ButtonNode {
    /// The visible, accessible label.
    pub label: String,
    /// Intent string emitted on click.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_click: Option<String>,
    /// Whether the button is disabled.
    #[serde(default)]
    pub disabled: bool,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A two-state checkbox.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckboxNode {
    /// Checked state.
    #[serde(default)]
    pub checked: bool,
    /// Accessible label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Intent string emitted when toggled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_change: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// An on/off switch.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwitchNode {
    /// On state.
    #[serde(default)]
    pub on: bool,
    /// Accessible label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Intent string emitted when toggled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_change: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// One option of a radio group.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadioNode {
    /// Selected state.
    #[serde(default)]
    pub selected: bool,
    /// Accessible label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Intent string emitted when chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_change: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A numeric slider over the normalized range `0.0..=1.0`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SliderNode {
    /// Current value, `0.0..=1.0`.
    #[serde(default)]
    pub value: f32,
    /// Intent string emitted on change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_change: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A text field (single- or multi-line, by node variant).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputNode {
    /// Current text value.
    #[serde(default)]
    pub value: String,
    /// Placeholder shown when empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Intent string emitted on edit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_input: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A childless decorative node (divider, spacer).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Leaf {
    /// Layout and appearance.
    #[serde(default)]
    pub style: Style,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// Layout and appearance props shared by containers, text, and leaves. Every
/// field is optional; spacing is in logical pixels. `color`/`size_px`/`weight`
/// apply only to text nodes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Style {
    /// Padding on all sides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p: Option<f32>,
    /// Horizontal padding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub px: Option<f32>,
    /// Vertical padding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub py: Option<f32>,
    /// Gap between children.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<f32>,
    /// Fixed width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<f32>,
    /// Fixed height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<f32>,
    /// Background fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg: Option<ColorSpec>,
    /// Corner radius.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rounded: Option<f32>,
    /// Border (width + color).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<Border>,
    /// Cross-axis alignment: `start` | `center` | `end` | `baseline`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// Main-axis distribution: `start` | `center` | `end` | `between`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub justify: Option<String>,
    /// Text color (text nodes only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorSpec>,
    /// Text size in pixels (text nodes only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_px: Option<f32>,
    /// Text weight 100–900 (text nodes only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<u16>,
}

/// A color reference: a theme role name, or an explicit OKLCH triple. Raw hex
/// is intentionally not representable — colors come from the theme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ColorSpec {
    /// A theme role name, e.g. `"surface"`, `"accent"`, `"text"`, `"danger"`.
    Role(String),
    /// An explicit OKLCH escape hatch: `{ "oklch": [l, c, h] }`.
    Oklch(OklchColor),
}

/// The OKLCH escape hatch payload: `{ "oklch": [lightness, chroma, hue] }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OklchColor {
    /// `[lightness 0..=1, chroma, hue degrees]`.
    pub oklch: [f32; 3],
}

/// A border: a width in logical pixels and a color.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Border {
    /// Stroke width in logical pixels.
    pub width: f32,
    /// Stroke color.
    pub color: ColorSpec,
}
