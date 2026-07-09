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
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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

/// The JSON Schema for a `fenestra/1` [`Description`] — a machine-checkable input
/// grammar a client can validate or autocomplete against *before* a round-trip,
/// the formal complement to the prose [`crate::vocabulary::describe_vocabulary`].
/// Derived from the format types, so it can never drift from what the parser
/// accepts (the externally-tagged nodes, `deny_unknown_fields`, and the untagged
/// color/track unions all express precisely in the schema).
#[must_use]
pub fn description_schema() -> serde_json::Value {
    schemars::schema_for!(Description).to_value()
}

/// One node in the tree. Externally tagged: each node object carries exactly
/// one variant key. Unknown keys are rejected as unknown variants.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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
    /// Two children split by a draggable divider. `bind` a root `state`
    /// number key for the split fraction.
    SplitPane(SplitPaneNode),
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
    /// A labelled form-field wrapper: a label (with an optional required
    /// mark) above `control`, and help or error text (error wins) below it.
    Field(FieldNode),
    /// An editable select: typing filters `options`; `bind` a root `state`
    /// text key for the value (both typing and picking an option write it).
    Combobox(ComboboxNode),
    /// A set of toggleable option chips. `selected` lists the pre-checked
    /// option indices.
    MultiSelect(MultiSelectNode),
    /// A bordered field holding removable tag chips plus an inline entry
    /// field for typing new ones.
    TagInput(TagInputNode),
    /// A month calendar. Single-date by default; `range:true` switches to
    /// start/end range selection.
    DatePicker(DatePickerNode),
    /// An OKLCH color picker: a lightness×chroma pad, hue/alpha strips, a
    /// swatch, and a hex/`oklch()` text entry. `bind` a root `state` text key
    /// for the committed hex value.
    ColorPicker(ColorPickerNode),
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
    /// A nested disclosure tree with app-owned expansion and selection; one
    /// tab stop, keyboard-navigable (arrows, Home/End, type-ahead).
    Tree(TreeViewNode),
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
    /// A base64-encoded PNG image, decoded to RGBA8 at parse time. `label`
    /// is the required accessible alt text.
    Image(ImageNode),
    /// A stack of transient status toasts pinned to the top-right. An empty
    /// `items` list renders nothing.
    Toast(ToastStackNode),
    // ── Data ──────────────────────────────────────────────────────────────
    /// A sortable, optionally multi-select data grid.
    DataTable(DataTableNode),
    /// A fixed-row-height virtualized list of literal child nodes (never a
    /// code closure — a bounded, authored array of rows).
    VirtualList(VirtualListNode),
    // ── Overlays ──────────────────────────────────────────────────────────
    /// A centered modal dialog with title, children, and optional `on_close` intent.
    Modal(ModalNode),
    /// A hover tooltip wrapping a `target` node.
    Tooltip(TooltipNode),
    /// An edge-anchored drawer / sheet panel with a backdrop and `children`.
    Drawer(DrawerNode),
    /// A floating panel anchored below `trigger`, toggled by clicking it
    /// (self-contained: outside click / Escape close it automatically).
    Popover(PopoverNode),
    /// A menu that toggles open when `trigger` is clicked; `items` are
    /// `(label, on_select intent)` pairs. Self-contained like `popover`.
    DropdownMenu(DropdownMenuNode),
    /// A modal Cmd-K launcher: typing filters `commands`, Enter runs the top
    /// match. Present in the tree = shown (like `modal`); omit it from the
    /// next description to close it.
    CommandPalette(CommandPaletteNode),
    // ── Decoration ────────────────────────────────────────────────────────
    /// A themed hairline rule.
    Divider(Leaf),
    /// Flexible empty space.
    Spacer(Leaf),
}

/// A flex/grid container: children plus a style block.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AccordionItemDto {
    /// The header title.
    pub title: String,
    /// The collapsible body content (any node).
    pub body: Box<Node>,
}

/// A stack of expandable disclosure sections.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
    /// `grid-template-areas`: rows of whitespace-separated area names, `.` for an
    /// empty cell, e.g. `["header header", "nav main", "footer footer"]`. Place
    /// children with `grid_area`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_template_areas: Option<Vec<String>>,
    /// Named-area placement (CSS `grid-area`), e.g. `"main"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_area: Option<String>,
    /// Named-line column placement `[start, end]` (CSS `grid-column: a / b`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_col_lines: Option<[String; 2]>,
    /// Named-line row placement `[start, end]` (CSS `grid-row: a / b`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_row_lines: Option<[String; 2]>,
    /// Column line names, positional: the i-th names the (i+1)-th column line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_col_names: Option<Vec<String>>,
    /// Row line names, positional: the i-th names the (i+1)-th row line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_row_names: Option<Vec<String>>,
    /// Surface material role: `card` | `raised` | `popover` | `menu` | `modal` |
    /// `glass` | `tooltip` | `thumb`. `glass` applies the whole frosted Liquid
    /// Glass treatment — vibrancy tint, backdrop blur, specular rim, body sheen,
    /// backdrop-adaptive tint, and elevation — in one token. A material role owns
    /// the fill/border/radius/shadow it sets; for a custom look use the individual
    /// paint fields (`bg`, `rounded`, the glass optics) instead of a role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,
    /// Corner smoothing `0.0..=1.0`: the continuous-curvature "squircle" amount
    /// (Apple-style corners). `0` is an exact circular-arc rounded rect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_smoothing: Option<f32>,
    /// Backdrop blur radius in logical px: frosts the content behind a translucent
    /// pane (realized headlessly; the live single-pass window keeps the flat tint).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backdrop_blur: Option<f32>,
    /// Liquid Glass specular edge rim: the string `"glass"` for the stock recipe,
    /// or a structured `{ "light_deg", "intensity", "shade" }`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specular_edge: Option<EdgeSpec>,
    /// Directional body sheen: `"glass"`, or `{ "light_deg", "top", "bottom" }`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheen: Option<SheenSpec>,
    /// Backdrop-adaptive vibrancy: `"glass"`, or `{ "pivot", "gain" }`. Shifts the
    /// glass tint's lightness by the backdrop's mean luminance (headless).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adaptive_tint: Option<AdaptiveSpec>,
    /// A translucent vibrancy material as the background — the custom-glass escape
    /// hatch (a `surface: "glass"` gives the stock recipe). Sets a [`Material`] tint
    /// of its `tint` color as the fill and its `blur` as the backdrop blur.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<MaterialSpec>,
    /// Per-corner radii `[tl, tr, br, bl]` in logical px (overrides `rounded`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corners: Option<[f32; 4]>,
    /// Fully round the corners into a pill / capsule (radius = half the short side).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rounded_full: Option<bool>,
    /// Paint-time translation `[x, y]` in logical px (no layout effect).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translate: Option<[f32; 2]>,
    /// Paint-time rotation in degrees, about the element's center.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotate: Option<f32>,
    /// Paint-time skew `[x_deg, y_deg]` in degrees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skew: Option<[f32; 2]>,
    /// A foreground filter on this element's own content (blur / brightness /
    /// saturate). Realized in the headless two-pass render. Cannot be combined with
    /// a paint transform (`translate` / `rotate` / `skew`) on the same node — the
    /// filter samples the pre-transform layout rect, so the pair is rejected; apply
    /// them on separate nested nodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element_filter: Option<FilterSpec>,
    /// Path draw-progress `0.0..=1.0` (only meaningful on path / icon elements).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim: Option<f32>,
}

/// A Liquid Glass specular edge rim in JSON: the `"glass"` preset, or explicit
/// levers `{ "light_deg", "intensity", "shade" }`. Mirrors
/// [`fenestra_core::SpecularEdge`].
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum EdgeSpec {
    /// A named preset — currently only `"glass"`.
    Preset(String),
    /// Explicit levers, all required.
    Custom {
        /// Light azimuth in CSS gradient degrees (`0` = top).
        light_deg: f32,
        /// Bright-side rim alpha at the lit edge.
        intensity: f32,
        /// Dark-side rim alpha at the shaded edge.
        shade: f32,
    },
}

/// A directional body sheen in JSON: the `"glass"` preset, or explicit levers
/// `{ "light_deg", "top", "bottom" }`. Mirrors [`fenestra_core::Sheen`].
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum SheenSpec {
    /// A named preset — currently only `"glass"`.
    Preset(String),
    /// Explicit levers, all required.
    Custom {
        /// Gradient axis in CSS degrees (`0` = up).
        light_deg: f32,
        /// White alpha at the lit (near) end.
        top: f32,
        /// Dark alpha at the far end.
        bottom: f32,
    },
}

/// Backdrop-adaptive vibrancy in JSON: the `"glass"` preset, or explicit levers
/// `{ "pivot", "gain" }`. Mirrors [`fenestra_core::AdaptiveTint`].
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum AdaptiveSpec {
    /// A named preset — currently only `"glass"`.
    Preset(String),
    /// Explicit levers, all required.
    Custom {
        /// Backdrop luminance the shift pivots around (`0`..`1`).
        pivot: f32,
        /// Lightness-shift gain.
        gain: f32,
    },
}

/// A translucent vibrancy material in JSON: a [`ColorSpec`] `tint` plus the
/// `fill_alpha` / `blur` / `saturation` levers. Mirrors [`fenestra_core::Material`]
/// — the custom-glass escape hatch behind `surface: "glass"`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialSpec {
    /// Base color the vibrancy tint is derived from.
    pub tint: ColorSpec,
    /// Fraction of the tint that shows over the (blurred) backdrop, `0.0..=1.0`.
    pub fill_alpha: f32,
    /// Backdrop blur radius in logical px (also drives the element's backdrop blur).
    pub blur: f32,
    /// OKLCH chroma multiplier on the tint (`>= 1.0` re-saturates).
    pub saturation: f32,
}

/// A foreground filter in JSON: `{ "blur": r }` | `{ "brightness": m }` |
/// `{ "saturate": m }`. Mirrors [`fenestra_core::ElementFilter`].
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FilterSpec {
    /// Gaussian blur of this element's own content (radius in logical px).
    Blur(f32),
    /// Brightness multiplier (`1.0` = unchanged).
    Brightness(f32),
    /// Saturation multiplier (`1.0` = unchanged, `0.0` = grayscale).
    Saturate(f32),
}

/// One grid template entry: a track *string* (`"200px"`, `"1fr"`, `"auto"`,
/// `"min-content"`, `"max-content"`), or a structured object for `minmax`,
/// `fit_content`, or `repeat`. This is the JSON form of the builder vocabulary
/// (`Track` / `GridTemplate`), so `repeat(auto-fit, minmax(180px, 1fr))` — the
/// responsive grid — is authorable as
/// `{"repeat": {"count": "auto-fit", "tracks": [{"minmax": ["180px", "1fr"]}]}}`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum TrackSpec {
    /// A single track as a keyword/length string.
    Keyword(String),
    /// A structured track entry (`minmax` / `fit_content` / `repeat`).
    Structured(Box<TrackObj>),
}

/// The structured form of a [`TrackSpec`]: exactly one of `minmax`, `fit_content`,
/// or `repeat`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RepeatSpec {
    /// A positive integer count, or `"auto-fit"` / `"auto-fill"`.
    pub count: RepeatCount,
    /// The track fragment to repeat (keyword / `minmax` / `fit_content`; not nested `repeat`).
    pub tracks: Vec<TrackSpec>,
}

/// How many times a `repeat(...)` is generated.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum ColorSpec {
    /// A theme role name, e.g. `"surface"`, `"accent"`, `"text"`, `"danger"`.
    Role(String),
    /// An explicit OKLCH escape hatch: `{ "oklch": [l, c, h] }`.
    Oklch(OklchColor),
}

/// The OKLCH escape hatch payload: `{ "oklch": [lightness, chroma, hue] }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OklchColor {
    /// `[lightness 0..=1, chroma, hue degrees]`.
    pub oklch: [f32; 3],
}

/// A border: a width in logical pixels and a color.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Border {
    /// Stroke width in logical pixels.
    pub width: f32,
    /// Stroke color.
    pub color: ColorSpec,
}

/// A surface-framed toolbar grouping action controls.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MenuItemDto {
    /// The item label.
    pub label: String,
    /// Intent emitted when the item is chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_select: Option<String>,
}

/// One top-level menu of a menubar.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MenubarMenuDto {
    /// The trigger title.
    pub title: String,
    /// The dropdown items.
    #[serde(default)]
    pub items: Vec<MenuItemDto>,
}

/// An application menu bar.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
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

// ── R3 vocabulary expansion: image + kit-widget completeness ────────────────
//
// The nodes below complete the fenestra/1 grammar against the kit's full
// widget set. Three of the kit's widgets carry *continuous, per-event*
// payloads (a computed pixel width, a drag-and-drop from/to index pair) that
// do not fit any [`crate::state::Action`] variant — every handler here is a
// single inert intent string or a framework-owned scalar (bool/number/text)
// state write, never a value computed from the interaction itself. Where a
// kit widget cannot be represented for that reason, its node's doc comment
// says so explicitly rather than silently omitting it.

/// A form field: a label (with an optional required mark) above `control`,
/// and help or error text (error wins) below it.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FieldNode {
    /// The field label.
    pub label: String,
    /// The wrapped control (input, select, switch, …).
    pub control: Box<Node>,
    /// Muted helper text below the control (hidden when `error` is set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// A validation message shown in the danger tone; wins over `help`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Appends a danger-toned `*` to the label.
    #[serde(default)]
    pub required: bool,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// Two children split by a draggable divider.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SplitPaneNode {
    /// The first (left/top) pane content.
    pub first: Box<Node>,
    /// The second (right/bottom) pane content.
    pub second: Box<Node>,
    /// The split fraction (the widget itself clamps to `0.05..=0.95`).
    #[serde(default = "default_half")]
    pub fraction: f32,
    /// Bind the fraction to a `state` number key (the framework sets it on drag).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    /// Stack the panes vertically (divider drags up/down) instead of side by side.
    #[serde(default)]
    pub vertical: bool,
    /// Intent emitted on drag (ignored when `bind` is set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_resize: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// An editable select: typing filters `options` (case-insensitive contains)
/// while `open` is true; picking an option updates the bound value. Drops
/// the kit widget's `on_navigate`/`highlighted` keyboard-cursor wiring (a
/// decorative veil over the active option) — Enter still accepts the top
/// filtered match without it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComboboxNode {
    /// The full option list (filtered client-side by the typed value).
    pub options: Vec<String>,
    /// The current text value.
    #[serde(default)]
    pub value: String,
    /// Bind the value to a `state` text key: both typing and picking an
    /// option write it (the framework sets it; takes priority over
    /// `on_input`/`on_pick`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    /// Whether the filtered listbox is shown.
    #[serde(default)]
    pub open: bool,
    /// Placeholder shown while `value` is empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Intent emitted on every edit of the text (ignored when `bind` is set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_input: Option<String>,
    /// Intent emitted when an option is picked (ignored when `bind` is set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_pick: Option<String>,
    /// Intent emitted when the listbox wants to close (outside click, Escape).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_close: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A set of toggleable option chips.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MultiSelectNode {
    /// The option labels.
    pub options: Vec<String>,
    /// Indices of the pre-checked options.
    #[serde(default)]
    pub selected: Vec<usize>,
    /// Whether the group is disabled.
    #[serde(default)]
    pub disabled: bool,
    /// Intent emitted when a chip is toggled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_toggle: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A bordered field holding removable tag chips plus an inline entry field.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TagInputNode {
    /// The current tags, in order.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Placeholder shown in the inline field while it is empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Intent emitted when a chip's remove button is pressed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_remove: Option<String>,
    /// Intent emitted on every edit of the inline field. The kit widget has
    /// no commit-on-Enter hook and keeps no draft of its own, so this fires
    /// per keystroke with the field's full current text, not once on submit
    /// — the same constraint the kit widget's own docs call out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_add: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A calendar date as `[year, month 1..=12, day 1..=31]`.
pub type DateSpec = [i32; 3];

/// A month calendar. Single-date by default; `range:true` switches to
/// start/end range selection. Drops the kit widget's WAI-ARIA keyboard grid
/// navigation (`on_focus` + app-owned `focused_day`, driving arrow/Home/End/
/// PageUp/PageDown): Enter/Space still select a computed default focus (the
/// selected day, else today, else the 1st, within the visible month), but
/// arrow-key cursor movement has no bound handler to report to and is
/// silently inert rather than broken.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DatePickerNode {
    /// The visible year.
    pub year: i32,
    /// The visible month, `1..=12`.
    pub month: u32,
    /// Range mode instead of single-date.
    #[serde(default)]
    pub range: bool,
    /// The selected date (single mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<DateSpec>,
    /// The range start (range mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_start: Option<DateSpec>,
    /// The range end (range mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_end: Option<DateSpec>,
    /// The date marked "today" (the widget is clock-free; pass it explicitly).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub today: Option<DateSpec>,
    /// Days before this date render disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<DateSpec>,
    /// Days after this date render disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<DateSpec>,
    /// Intent emitted when a day is picked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_pick: Option<String>,
    /// Intent emitted when a header prev/next month/year button is pressed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_month: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// An OKLCH color picker: a lightness×chroma pad, hue/alpha strips, a
/// swatch, and a hex/`oklch()` text entry. `value` is a hex (`#rrggbb`/
/// `#rrggbbaa`) or CSS `oklch(l c h[ / a])` string, parsed with the kit's own
/// `parse_color_text`; an unparseable `value` (or the bound state text
/// overriding it) degrades to sRGB middle gray (`#808080`) and records a
/// path-pointed error — never a panic. `bind` a
/// root `state` text key: every pad/hue/alpha gesture, and every text edit
/// that currently parses, commits the formatted hex back to it; an edit
/// that doesn't yet parse (mid-keystroke) leaves the bound value alone,
/// mirroring the kit widget's own "don't destroy the last good color on an
/// invalid keystroke" contract. The kit widget also supports a *separate*
/// in-progress text buffer (`ColorPicker::text`, for showing invalid
/// partial input while the confirmed `value` stays unchanged) — that has no
/// JSON projection here: describe has one state slot per `bind`, not two,
/// so an invalid keystroke is simply not committed rather than shown.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ColorPickerNode {
    /// Current color as hex or `oklch()` text.
    #[serde(default = "default_color_picker_value")]
    pub value: String,
    /// Bind to a `state` text key (see the struct docs for the commit rule).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    /// Accessible label (default `"Color"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Disables every control (pad, strips, entry field) and dims the widget.
    #[serde(default)]
    pub disabled: bool,
    /// The 2D pad's side length in logical px (200 by default); the widget
    /// itself clamps this to `80.0..=480.0`, so no separate clamp is needed here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pad_size: Option<f32>,
    /// Intent emitted on any gesture or valid text edit (ignored when `bind`
    /// is set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_change: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// Default `value` for a `color_picker` when omitted: a neutral mid-gray —
/// the same fallback an unparseable `value` degrades to (serde `default` attribute).
pub fn default_color_picker_value() -> String {
    "#808080".to_string()
}

/// One item of a [`TreeViewNode`]: a stable `id`, visible `label`, and nested
/// `children` (empty = leaf). Recursive like [`Node`] itself, so the same
/// JSON-nesting ceiling applies: `serde_json` caps deserialization recursion
/// at 128 levels by default, well before the parser's own total-node clamp
/// (`items`/`children` fan-out, not depth) ever comes into play.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TreeItemDto {
    /// Stable id; expansion and selection key off it.
    pub id: String,
    /// The visible label.
    pub label: String,
    /// Child items (empty = leaf).
    #[serde(default)]
    pub children: Vec<TreeItemDto>,
}

/// A nested disclosure tree with app-owned expansion and selection; one tab
/// stop, keyboard-navigable (arrows, Home/End, type-ahead) for free from the
/// kit widget.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TreeViewNode {
    /// The root items.
    pub items: Vec<TreeItemDto>,
    /// Ids currently expanded.
    #[serde(default)]
    pub expanded: Vec<String>,
    /// The selected id, highlighted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<String>,
    /// Intent emitted when a branch is toggled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_toggle: Option<String>,
    /// Intent emitted when a node is selected (leaf click, or keyboard nav).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_select: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A base64-encoded PNG image, decoded to RGBA8 at parse time.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImageNode {
    /// The image bytes, base64-encoded (RFC 4648 standard alphabet, `=`
    /// padding, no embedded whitespace). A decode failure — bad base64, not
    /// a PNG, or a clamp violation — degrades to an invisible spacer and
    /// records a path-pointed error; it never panics.
    pub png: String,
    /// Required accessible label (alt text). An image without one is
    /// rejected with a path-pointed error rather than silently defaulted —
    /// this is the a11y failure the format exists to catch.
    pub label: String,
    /// Layout and appearance. `style.w`/`style.h` resize the image (it sizes
    /// to the decoded pixel dimensions by default); `style.rounded_full`
    /// crops a square source into a round avatar.
    #[serde(default)]
    pub style: Style,
    /// Intent string emitted on click (makes the image interactive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_click: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace — the same annotate-only field every other
    /// node carries; kept here purely for shape consistency across the
    /// grammar, not because images have a distinct fallback path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// One toast: a message and status color.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToastItemDto {
    /// The message text.
    pub message: String,
    /// Status color: `accent` (default) | `danger` | `warning` | `success`.
    #[serde(default = "default_status")]
    pub status: String,
}

/// A stack of transient status toasts pinned to the top-right. An empty
/// `items` list renders nothing.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToastStackNode {
    /// The toasts, in order.
    #[serde(default)]
    pub items: Vec<ToastItemDto>,
    /// Stack width in logical px (340 by default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f32>,
    /// Intent emitted when a toast's close button (or a swipe) dismisses it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_dismiss: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A `data_table`'s sort indicator (purely visual — the widget never sorts
/// its own `rows`; the host sorts and re-authors them).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SortSpec {
    /// The sorted column's index.
    pub column: usize,
    /// Ascending (`true`) or descending.
    pub ascending: bool,
}

/// A sortable, optionally multi-select data grid. Elm-pure like the kit
/// widget: `rows` render exactly as given (sort/filter your own data before
/// authoring), and every handler is a single inert intent — the host reads
/// which control fired from whatever it authors next, the same contract as
/// `tabs`/`select`. Drops the kit widget's column **resize** (`on_resize`/
/// `on_resize_end`/`resize_active`) and **reorder** (`on_reorder`, a
/// drag-and-drop from/to index pair): both need a computed value (a pixel
/// width, a pair of indices) per event that no single intent string or
/// scalar state write can carry. `column_widths`/`pinned_left`/
/// `pinned_right` are static layout, not interactions, so they stay
/// authorable.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DataTableNode {
    /// The column headers.
    pub columns: Vec<String>,
    /// The row cells, outer = row, inner = column (parallel to `columns`).
    #[serde(default)]
    pub rows: Vec<Vec<String>>,
    /// Draws the sort indicator on one column (▲/▼); purely visual.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<SortSpec>,
    /// Highlights one row by index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<usize>,
    /// Adds a leading tri-state checkbox column; `selection[i]` is row `i`'s
    /// checked state (the header shows checked/mixed/unchecked from this).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<Vec<bool>>,
    /// Forces the scrolling sticky-header body.
    #[serde(default)]
    pub sticky_header: bool,
    /// Explicit per-column widths in logical px (parallel to `columns`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column_widths: Option<Vec<f32>>,
    /// Freezes the first `n` columns to the left edge on horizontal scroll
    /// (needs `column_widths`).
    #[serde(default)]
    pub pinned_left: usize,
    /// Freezes the last `n` columns to the right edge on horizontal scroll
    /// (needs `column_widths`).
    #[serde(default)]
    pub pinned_right: usize,
    /// Current per-column filter text (parallel to `columns`), shown in a
    /// filter row between the header and body. The widget never filters
    /// `rows` itself — wire `on_filter`, filter your data, and re-author it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Vec<String>>,
    /// Intent emitted when a header is clicked to sort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_sort: Option<String>,
    /// Intent emitted when a row is clicked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_select: Option<String>,
    /// Intent emitted when a row's checkbox is toggled (needs `selection`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_select_row: Option<String>,
    /// Intent emitted when the header's tri-state checkbox is toggled
    /// (needs `selection`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_select_all: Option<String>,
    /// Intent emitted when a filter cell is edited (needs `filter`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_filter: Option<String>,
    /// Stable key (also namespaces the virtualized body's scroll state and
    /// the per-column filter editors).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A fixed-row-height virtualized list: only rows scrolled into view
/// materialize. `items` are literal child nodes built through the same
/// [`Node`] grammar as everything else — never a code closure — so a "100k
/// row" list is only ever as large as the JSON actually sent; see
/// [`MAX_LIST_ITEMS`](crate::parse::MAX_LIST_ITEMS) for the authored clamp.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VirtualListNode {
    /// The row content, in order.
    pub items: Vec<Node>,
    /// Fixed row height in logical px.
    pub row_height: f32,
    /// Stable key (recommended: the scroll offset is kept per id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// One dropdown/popover-menu item: a label and the intent it emits when
/// chosen. Reuses the same shape as [`MenubarMenuDto`]'s items. Deliberately
/// the *simple* menu shape (`(label, message)` pairs) — the kit's richer
/// `menu_item`/`menu_items` API (leading icons, trailing shortcut hints,
/// disabled rows, nested submenus, separators) has no JSON projection here;
/// author a plain flat list of choices instead.
pub type DropdownMenuItemDto = MenuItemDto;

/// A menu that toggles open when `trigger` is clicked; outside click, an
/// item click, or Escape close it automatically (no state needed — the
/// engine owns the open/closed flag).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DropdownMenuNode {
    /// The clickable anchor content.
    pub trigger: Box<Node>,
    /// The dropdown items (see [`DropdownMenuItemDto`]).
    pub items: Vec<DropdownMenuItemDto>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A floating panel anchored below `trigger`, toggled by clicking it — the
/// general-purpose escape hatch when `content` isn't a flat action list
/// (unlike `dropdown_menu`, `content` is any node).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PopoverNode {
    /// The clickable anchor content.
    pub trigger: Box<Node>,
    /// The panel content.
    pub content: Box<Node>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// A modal Cmd-K launcher. Present in the tree = shown — the same
/// tree-presence-is-visibility contract as `modal`/`drawer` (there is no
/// `open` field; omit the node from the next description to close it).
/// Drops the kit widget's `on_navigate`/`highlighted` keyboard-cursor wiring,
/// same as `combobox` — Enter still runs the top filtered match.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommandPaletteNode {
    /// The current query text.
    #[serde(default)]
    pub query: String,
    /// Bind the query to a `state` text key (the framework sets it as the
    /// user types).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    /// The command list (see [`MenuItemDto`]).
    pub commands: Vec<MenuItemDto>,
    /// Intent emitted on Escape / outside click.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_close: Option<String>,
    /// Stable key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reserved fallback trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// Default `0.5` for the split pane's `fraction` (serde `default` attribute).
pub fn default_half() -> f32 {
    0.5
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
