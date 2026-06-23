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

use crate::state::StateMap;

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
    /// Initial runtime state for declarative bindings (a widget's `bind`): keys
    /// to JSON values (bool / number / string).
    #[serde(default, skip_serializing_if = "StateMap::is_empty")]
    pub state: StateMap,
}

/// One node in the tree. Externally tagged: each node object carries exactly
/// one variant key. Unknown keys are rejected as unknown variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Node {
    // ── Layout containers ─────────────────────────────────────────────────
    /// A horizontal flex container.
    Row(Container),
    /// A vertical flex container.
    Col(Container),
    /// A generic (row) flex container.
    Div(Container),
    /// A z-stacked / grid container.
    Stack(Container),
    /// A raised-surface content card (vertical flex, SP6 padding, rounded).
    Card(Container),
    // ── Text ──────────────────────────────────────────────────────────────
    /// A run of text. Supports `on_click` for clickable labels.
    Text(TextNode),
    // ── Form controls ──────────────────────────────────────────────────────
    /// An activatable button. `variant`: primary | secondary | ghost | danger.
    /// `bind` a bool state key for toggle behavior.
    Button(ButtonNode),
    /// A two-state checkbox. `bind` a root `state` key for a framework-owned toggle.
    Checkbox(CheckboxNode),
    /// An on/off switch. `bind` a root `state` key for a framework-owned toggle.
    Switch(SwitchNode),
    /// One option of a radio group. Use `group` + `value` for group binding.
    Radio(RadioNode),
    /// A numeric slider over 0.0..=1.0 with an optional `step`.
    Slider(SliderNode),
    /// A single-line editable text field.
    TextInput(InputNode),
    /// A multi-line editable text field.
    TextArea(InputNode),
    /// A drop-down selector. `bind` a root `state` number key for the selected index.
    Select(SelectNode),
    /// A compact number stepper: a value flanked by − / + step buttons.
    SpinButton(SpinButtonNode),
    // ── Navigation ────────────────────────────────────────────────────────
    /// An underline tab strip. `bind` a root `state` number key for the active index.
    Tabs(TabsNode),
    /// A compact single-select view switcher. `bind` a root `state` number key.
    Segmented(SegmentedNode),
    /// A breadcrumb trail; the last item is the current page. `bind`/`on_change`
    /// fire with the selected ancestor's index.
    Breadcrumbs(BreadcrumbsNode),
    /// A numbered pagination strip. `bind` a root `state` number key for the page.
    Pagination(PaginationNode),
    /// A horizontal step indicator. `bind` a root `state` number key for the step.
    Stepper(StepperNode),
    /// A surface-framed bar grouping action controls (its `children`).
    Toolbar(ToolbarNode),
    /// An application menu bar: top-level `menus`, each a dropdown of items.
    Menubar(MenubarNode),
    // ── Display / feedback ─────────────────────────────────────────────────
    /// A status pill. `status`: accent (default) | danger | warning | success.
    Badge(BadgeNode),
    /// A status callout: tinted background, status border, icon, and message.
    Callout(CalloutNode),
    /// A metric card with muted label, large value, and optional delta badge.
    StatCard(StatCardNode),
    /// A circular initials avatar in the accent tint.
    Avatar(AvatarNode),
    /// A status dot + label indicator. `live:true` adds a pulsing sonar ring.
    Status(StatusNode),
    /// A keyboard key-cap chord. `raised:true` for 3D keycap style.
    Kbd(KbdNode),
    /// A 4px progress bar, `value` 0..=1. `indeterminate:true` for sweep animation.
    Progress(ProgressNode),
    /// A measurement meter, `value` within `min`..=`max`, with optional
    /// low/high/optimum zones (success/warning/danger).
    Meter(MeterNode),
    /// A stack of expandable disclosure sections. `open` (or `bind`) selects the
    /// expanded item by index.
    Accordion(AccordionNode),
    /// A rotating arc activity indicator (no parameters).
    Spinner(Leaf),
    /// A loading placeholder. `kind`: rect (default) | circle | text.
    Skeleton(SkeletonNode),
    /// A named Lucide icon (24×24, stroked).
    Icon(IconNode),
    // ── Overlays ──────────────────────────────────────────────────────────
    /// A centered modal dialog with title, children, and optional `on_close` intent.
    Modal(ModalNode),
    /// A hover tooltip wrapping a `target` node.
    Tooltip(TooltipNode),
    /// An edge-anchored drawer / sheet panel with a backdrop and `children`.
    Drawer(DrawerNode),
    // ── Decoration ────────────────────────────────────────────────────────
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
    /// Intent string emitted on click (makes the container an interactive region).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_click: Option<String>,
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
    /// Intent string emitted on click (makes the text interactive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_click: Option<String>,
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
    /// Bind a bool `state` key: click toggles `state[bind]` (takes priority over `on_click`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    /// Visual emphasis: `primary` (default) | `secondary` | `ghost` | `danger`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
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
    /// Bind the checked state to a `state` key (the framework toggles it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
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
    /// Bind the on state to a `state` key (the framework toggles it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
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
    /// Selected state (ignored when `group` + `value` are set — derived from state).
    #[serde(default)]
    pub selected: bool,
    /// Accessible label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// State key holding the currently-selected value in the group. When set,
    /// `selected` is derived by comparing `state[group] == value`, and clicking
    /// emits `SetText(group, value)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// This option's value within the group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Intent string emitted when chosen (used only when `group` is absent).
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
    /// Bind the value to a `state` key (the framework sets it on change).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    /// Snap increment, e.g. `0.1` (continuous when unset).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f32>,
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
    /// Bind the value to a `state` key (the framework sets it as the user types).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    /// Placeholder shown when empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Intent string emitted on edit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_input: Option<String>,
    /// Mark the control invalid (the danger ring + `aria-invalid`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invalid: Option<bool>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A drop-down selector.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectNode {
    /// The list of option labels (must be non-empty).
    pub options: Vec<String>,
    /// Currently selected index (0-based).
    #[serde(default)]
    pub selected: usize,
    /// Bind the selected index to a `state` number key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    /// Intent string emitted on selection change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_change: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// An underline tab strip.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabsNode {
    /// The list of tab labels (must be non-empty).
    pub labels: Vec<String>,
    /// Currently active index (0-based).
    #[serde(default)]
    pub active: usize,
    /// Bind the active index to a `state` number key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    /// Intent string emitted on tab selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_change: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A compact single-select view switcher (segmented control).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SegmentedNode {
    /// The list of segment labels (must be non-empty).
    pub labels: Vec<String>,
    /// Currently active index (0-based).
    #[serde(default)]
    pub active: usize,
    /// Bind the active index to a `state` number key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    /// Intent string emitted on segment selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_change: Option<String>,
    /// Whether the control is disabled.
    #[serde(default)]
    pub disabled: bool,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A breadcrumb trail of ancestor links ending in the current page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BreadcrumbsNode {
    /// The crumb labels in order; the last is the current (non-link) page.
    pub items: Vec<String>,
    /// Collapse trails longer than this to a single ellipsis (keeps the root
    /// and the last `max_items - 1` crumbs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
    /// Bind the selected ancestor's index to a `state` number key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    /// Intent string emitted when an ancestor crumb is selected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_change: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A numbered pagination strip with prev/next arrows.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaginationNode {
    /// Total number of pages (must be at least 1).
    pub count: usize,
    /// The current page (1-based).
    #[serde(default)]
    pub page: usize,
    /// Page numbers kept on each side of the current before collapsing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub siblings: Option<usize>,
    /// Bind the current page to a `state` number key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    /// Intent string emitted on page change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_change: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A horizontal step indicator for a multi-step flow.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepperNode {
    /// The step titles in order.
    pub steps: Vec<String>,
    /// Optional one-line descriptions, paired by index with `steps`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub descriptions: Vec<String>,
    /// The active step (0-based).
    #[serde(default)]
    pub current: usize,
    /// Bind the active step to a `state` number key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    /// Intent string emitted when a done/active step is selected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_change: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A compact number stepper (a value flanked by − / + step buttons).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpinButtonNode {
    /// The displayed value, app-formatted (`"3"`, `"$5.00"`, `"2.5×"`).
    pub value: String,
    /// Accessible name for the stepper (e.g. "Quantity").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Intent emitted when the − button is pressed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_decrement: Option<String>,
    /// Intent emitted when the + button is pressed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_increment: Option<String>,
    /// Whether the − button is enabled (default true; gate off at the minimum).
    #[serde(default = "default_true")]
    pub can_decrement: bool,
    /// Whether the + button is enabled (default true; gate off at the maximum).
    #[serde(default = "default_true")]
    pub can_increment: bool,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A measurement meter within a known range, with optional semantic zones.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeterNode {
    /// The measured value.
    pub value: f32,
    /// Range minimum (default 0).
    #[serde(default)]
    pub min: f32,
    /// Range maximum (default 1).
    #[serde(default = "default_one")]
    pub max: f32,
    /// Low-end threshold; with any threshold set, the fill colours by zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<f32>,
    /// High-end threshold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<f32>,
    /// Optimum end marking "good" (default max — higher is better; set at or
    /// below `low` for lower-is-better).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optimum: Option<f32>,
    /// Caption shown above the bar, paired with the value percentage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Bind the value to a `state` number key (read-only display).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// One section of an accordion: a header title and nested body content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccordionItemDto {
    /// The header title.
    pub title: String,
    /// The collapsible body content (any node).
    pub body: Box<Node>,
}

/// A stack of expandable disclosure sections.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccordionNode {
    /// The sections, in order.
    pub items: Vec<AccordionItemDto>,
    /// Index of the expanded section (none expanded if unset).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<usize>,
    /// Bind the expanded index to a `state` number key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    /// Intent emitted when a section header is toggled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_change: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A status pill badge.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BadgeNode {
    /// The label text.
    pub label: String,
    /// Status color: `accent` (default) | `danger` | `warning` | `success`.
    #[serde(default = "default_status")]
    pub status: String,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A status callout: tinted background, left status border, icon, and message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalloutNode {
    /// Status: `accent` | `danger` | `warning` | `success`.
    pub status: String,
    /// The message body.
    pub message: String,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A metric card with a muted label, large value, and optional delta badge.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatCardNode {
    /// Muted label (e.g. "Revenue").
    pub label: String,
    /// Prominent value (e.g. "$48k").
    pub value: String,
    /// Optional change indicator (e.g. "+12%").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<String>,
    /// Status for the delta badge: `accent` | `danger` | `warning` | `success`.
    #[serde(default = "default_status")]
    pub delta_status: String,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A circular initials avatar in the accent tint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AvatarNode {
    /// One or two initials to display (e.g. "JD").
    pub initials: String,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A status dot + label indicator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusNode {
    /// The label text (e.g. "Operational").
    pub label: String,
    /// Status color: `accent` | `danger` | `warning` | `success`.
    #[serde(default = "default_status")]
    pub status: String,
    /// When true, the status dot pulses with a sonar-ring animation.
    #[serde(default)]
    pub live: bool,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A keyboard key-cap chord.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KbdNode {
    /// The key(s) in the chord. Modifier names (`cmd`, `ctrl`, `opt`, `shift`,
    /// `win`) are resolved to platform glyphs.
    pub keys: Vec<String>,
    /// Use the 3D keycap style instead of the flat badge style.
    #[serde(default)]
    pub raised: bool,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A 4px progress bar.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressNode {
    /// Fill level, clamped to `0.0..=1.0`.
    pub value: f32,
    /// When true, renders the sweep indeterminate animation instead of the fill.
    #[serde(default)]
    pub indeterminate: bool,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A loading placeholder.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkeletonNode {
    /// Shape variant: `rect` (default) | `circle` | `text`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Width in logical px (used by `rect` and `circle`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<f32>,
    /// Height in logical px (used by `rect`; `circle` derives h from w).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<f32>,
    /// Number of text lines (used by `text`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<usize>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A named Lucide icon (24×24, stroked).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IconNode {
    /// The Lucide icon name (e.g. `"plus"`, `"check"`, `"settings"`). Unknown
    /// names degrade to an invisible spacer and record a parse error.
    pub name: String,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A centered modal dialog with title, children, and an optional close handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModalNode {
    /// The dialog title shown in the header.
    pub title: String,
    /// Child nodes forming the dialog body.
    #[serde(default)]
    pub children: Vec<Node>,
    /// Intent string emitted when the dialog close button is pressed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_close: Option<String>,
    /// Maximum width of the dialog in logical px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_width: Option<f32>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A hover tooltip wrapping a target node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TooltipNode {
    /// The tooltip label shown on hover.
    pub label: String,
    /// The element to wrap. Boxed to prevent infinite type recursion.
    pub target: Box<Node>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A childless decorative node (divider, spacer, spinner).
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

/// A linear gradient background.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GradientSpec {
    /// Gradient angle in degrees (0 = top→bottom, 90 = left→right).
    pub angle: f32,
    /// Color stops: at least two [`ColorSpec`]s. Evenly distributed if only two.
    pub stops: Vec<ColorSpec>,
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
    /// Top padding (overrides `p`/`py`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pt: Option<f32>,
    /// Bottom padding (overrides `p`/`py`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pb: Option<f32>,
    /// Left padding (overrides `p`/`px`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pl: Option<f32>,
    /// Right padding (overrides `p`/`px`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<f32>,
    /// Margin on all sides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub m: Option<f32>,
    /// Horizontal margin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mx: Option<f32>,
    /// Vertical margin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub my: Option<f32>,
    /// Top margin (overrides `m`/`my`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mt: Option<f32>,
    /// Bottom margin (overrides `m`/`my`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mb: Option<f32>,
    /// Left margin (overrides `m`/`mx`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ml: Option<f32>,
    /// Right margin (overrides `m`/`mx`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mr: Option<f32>,
    /// Gap between children.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<f32>,
    /// Fixed width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<f32>,
    /// Fixed height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<f32>,
    /// Minimum width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_w: Option<f32>,
    /// Maximum width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_w: Option<f32>,
    /// Minimum height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_h: Option<f32>,
    /// Maximum height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_h: Option<f32>,
    /// Background fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg: Option<ColorSpec>,
    /// Linear gradient background (applied after `bg`; gradient wins).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<GradientSpec>,
    /// Shadow / elevation token: `sm` | `md` | `lg` | `xl`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<String>,
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
    /// Text alignment: `start` | `center` | `end` (text nodes only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_align: Option<String>,
    /// Alpha opacity, `0.0..=1.0` (any node).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    /// Remove from flow and position absolutely within the nearest positioned ancestor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub absolute: Option<bool>,
    /// Left offset in logical px (only meaningful when `absolute: true`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<f32>,
    /// Top offset in logical px (only meaningful when `absolute: true`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<f32>,
    /// Right offset in logical px (only meaningful when `absolute: true`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<f32>,
    /// Bottom offset in logical px (only meaningful when `absolute: true`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom: Option<f32>,
    /// Grid template columns (switches the container to grid). An array of track
    /// entries — see [`TrackSpec`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_cols: Option<Vec<TrackSpec>>,
    /// Grid template rows. An array of track entries — see [`TrackSpec`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_rows: Option<Vec<TrackSpec>>,
}

/// One grid template entry: a track *string* (`"200px"`, `"1fr"`, `"auto"`,
/// `"min-content"`, `"max-content"`), or a structured object for `minmax`,
/// `fit_content`, or `repeat`. This is the JSON form of the builder vocabulary
/// (`Track` / `GridTemplate`), so `repeat(auto-fit, minmax(180px, 1fr))` — the
/// responsive grid — is authorable as
/// `{"repeat": {"count": "auto-fit", "tracks": [{"minmax": ["180px", "1fr"]}]}}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TrackSpec {
    /// A single track as a keyword/length string.
    Keyword(String),
    /// A structured track entry (`minmax` / `fit_content` / `repeat`).
    Structured(Box<TrackObj>),
}

/// The structured form of a [`TrackSpec`]: exactly one of `minmax`, `fit_content`,
/// or `repeat`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackObj {
    /// `minmax(min, max)` — two track strings, e.g. `["180px", "1fr"]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minmax: Option<[String; 2]>,
    /// `fit-content(px)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit_content: Option<f32>,
    /// `repeat(count, tracks)` — including the responsive `auto-fit` / `auto-fill`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<RepeatSpec>,
}

/// A `repeat(count, tracks)` fragment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepeatSpec {
    /// A positive integer count, or `"auto-fit"` / `"auto-fill"`.
    pub count: RepeatCount,
    /// The track fragment to repeat (keyword / `minmax` / `fit_content`; not nested `repeat`).
    pub tracks: Vec<TrackSpec>,
}

/// How many times a `repeat(...)` is generated.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RepeatCount {
    /// Exactly `n` repetitions.
    Count(u16),
    /// `"auto-fit"` or `"auto-fill"`.
    Keyword(String),
}

/// A color reference: a theme role name, or an explicit OKLCH triple. A raw hex
/// string is not a known role, so it is rejected at color resolution — colors
/// come from the theme, never an arbitrary literal.
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

/// A surface-framed toolbar grouping action controls.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolbarNode {
    /// The controls, in order.
    #[serde(default)]
    pub children: Vec<Node>,
    /// Stack vertically instead of in a row.
    #[serde(default)]
    pub vertical: bool,
    /// Accessible name for the bar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// One item of a menubar dropdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MenuItemDto {
    /// The item label.
    pub label: String,
    /// Intent emitted when the item is chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_select: Option<String>,
}

/// One top-level menu of a menubar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MenubarMenuDto {
    /// The trigger title.
    pub title: String,
    /// The dropdown items.
    #[serde(default)]
    pub items: Vec<MenuItemDto>,
}

/// An application menu bar.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MenubarNode {
    /// The top-level menus, in order.
    #[serde(default)]
    pub menus: Vec<MenubarMenuDto>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// An edge-anchored drawer / sheet panel.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrawerNode {
    /// Optional heading shown at the top of the panel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The panel content, in order.
    #[serde(default)]
    pub children: Vec<Node>,
    /// Anchored edge: `left` (default) | `right` | `top` | `bottom`.
    #[serde(default = "default_left")]
    pub side: String,
    /// Panel thickness (width for left/right, height for top/bottom).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f32>,
    /// Intent emitted on Esc / scrim click / close button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_close: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// Default value for `status`/`delta_status` fields (serde `default` attribute).
pub fn default_status() -> String {
    "accent".to_string()
}

/// Default `"left"` for the drawer's `side` (serde `default` attribute).
pub fn default_left() -> String {
    "left".to_string()
}

/// Default `true` for boolean fields that default on (serde `default` attribute).
pub fn default_true() -> bool {
    true
}

/// Default `1.0` for the meter's `max` (serde `default` attribute).
pub fn default_one() -> f32 {
    1.0
}
