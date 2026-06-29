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

/// One style property's documentation: its key, a one-line summary, and a minimal
/// example *value* (the JSON that follows the key inside a node's `"style"`).
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct StyleDoc {
    /// The style key, e.g. `"gap"` or `"surface"`.
    pub key: String,
    /// A one-line description.
    pub summary: String,
    /// A minimal example value: `{"div":{"style":{"<key>": <example>}}}` is valid.
    pub example: String,
}

/// One closed enum's allowed string values and the key it attaches to.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct EnumDoc {
    /// The key the enum attaches to — a style key (`"surface"`) or a node field
    /// (`"button.variant"`).
    pub name: String,
    /// The allowed string values.
    pub values: Vec<String>,
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
    /// Every style property a node's `"style"` may carry, with a minimal example.
    pub style: Vec<StyleDoc>,
    /// Closed enum token sets: the allowed string values for keyed fields.
    pub enums: Vec<EnumDoc>,
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
    (
        "toolbar",
        "Surface-framed bar grouping action controls (`children`). `vertical:true` stacks them.",
        r#"{"children":[{"button":{"label":"Bold"}},{"button":{"label":"Italic"}}]}"#,
    ),
    (
        "menubar",
        "Application menu bar: top-level `menus`, each with a `title` and dropdown `items` (label + optional on_select intent).",
        r#"{"menus":[{"title":"File","items":[{"label":"New"},{"label":"Open"}]}]}"#,
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
        "Named Lucide icon (24x24, stroked).",
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
    (
        "drawer",
        "Edge-anchored drawer / sheet with a backdrop. `side`: left (default) | right | top | bottom; `children` are the panel content; `on_close` intent on Esc/scrim.",
        r#"{"title":"Filters","side":"left","children":[{"text":{"content":"Body"}}]}"#,
    ),
    // ── Decoration ────────────────────────────────────────────────────────
    ("divider", "Themed hairline rule.", r#"{}"#),
    ("spacer", "Flexible empty space.", r#"{}"#),
];

/// `(key, summary, minimal example value)` for every style property the parser
/// applies — the registry the style grammar is generated from, kept honest by a
/// coherence test that authors each one.
const STYLE_REGISTRY: &[(&str, &str, &str)] = &[
    // ── Spacing & sizing ──────────────────────────────────────────────────
    (
        "p",
        "Padding all sides, px (also px / py and per-side pt / pb / pl / pr).",
        "16",
    ),
    ("px", "Horizontal padding.", "12"),
    ("py", "Vertical padding.", "8"),
    (
        "m",
        "Margin all sides (also mx / my and per-side mt / mb / ml / mr).",
        "8",
    ),
    ("gap", "Gap between flex / grid children.", "8"),
    (
        "w",
        "Fixed width (also h, min_w, max_w, min_h, max_h).",
        "200",
    ),
    ("h", "Fixed height.", "40"),
    // ── Paint ─────────────────────────────────────────────────────────────
    (
        "bg",
        "Background fill: a color role or an {\"oklch\":[l,c,h]}.",
        r#""surface_raised""#,
    ),
    (
        "gradient",
        "Linear gradient bg: {angle, stops:[ColorSpec, …]}.",
        r#"{"angle":90,"stops":["accent","surface"]}"#,
    ),
    ("opacity", "Subtree opacity 0.0..=1.0.", "0.85"),
    // ── Border / radius / shadow ──────────────────────────────────────────
    (
        "border",
        "Border {width, color}.",
        r#"{"width":1,"color":"border"}"#,
    ),
    ("rounded", "Uniform corner radius, px.", "12"),
    ("corners", "Per-corner radii [tl, tr, br, bl].", "[8,8,0,0]"),
    ("rounded_full", "Pill / capsule corners.", "true"),
    (
        "corner_smoothing",
        "Continuous-curvature squircle amount 0.0..=1.0.",
        "0.6",
    ),
    (
        "shadow",
        "Elevation token (see the `shadow` enum).",
        r#""md""#,
    ),
    // ── Glass / material ──────────────────────────────────────────────────
    (
        "surface",
        "A whole material role in one token (see the `surface` enum).",
        r#""glass""#,
    ),
    (
        "material",
        "Custom translucent vibrancy bg {tint, fill_alpha, blur, saturation}.",
        r#"{"tint":"surface_raised","fill_alpha":0.5,"blur":24,"saturation":1.6}"#,
    ),
    (
        "backdrop_blur",
        "Frost the content behind a translucent pane, px.",
        "24",
    ),
    (
        "specular_edge",
        "Liquid Glass rim: \"glass\" or {light_deg, intensity, shade}.",
        r#""glass""#,
    ),
    (
        "sheen",
        "Body sheen: \"glass\" or {light_deg, top, bottom}.",
        r#""glass""#,
    ),
    (
        "adaptive_tint",
        "Backdrop-adaptive vibrancy: \"glass\" or {pivot, gain}.",
        r#""glass""#,
    ),
    // ── Transforms / filter ───────────────────────────────────────────────
    ("translate", "Paint-time translate [x, y], px.", "[4,0]"),
    ("rotate", "Paint-time rotation, degrees.", "15"),
    ("skew", "Paint-time skew [x_deg, y_deg].", "[6,0]"),
    (
        "element_filter",
        "Foreground filter: {blur | brightness | saturate: n}.",
        r#"{"blur":4}"#,
    ),
    // ── Typography (text nodes) ───────────────────────────────────────────
    ("size_px", "Text size, px.", "16"),
    ("weight", "Text weight 100..=900.", "600"),
    (
        "text_align",
        "Text alignment (see the `text_align` enum).",
        r#""center""#,
    ),
    (
        "color",
        "Text color: a role or an {\"oklch\":[l,c,h]}.",
        r#""text_muted""#,
    ),
    // ── Layout / positioning ──────────────────────────────────────────────
    (
        "align",
        "Cross-axis alignment (see the `align` enum).",
        r#""center""#,
    ),
    (
        "justify",
        "Main-axis distribution (see the `justify` enum).",
        r#""between""#,
    ),
    (
        "absolute",
        "Remove from flow + position absolutely (with left / top / right / bottom).",
        "true",
    ),
    // ── Grid ──────────────────────────────────────────────────────────────
    (
        "grid_cols",
        "Grid template columns: array of track entries (also grid_rows).",
        r#"["1fr","1fr"]"#,
    ),
    (
        "grid_area",
        "Named-area placement (CSS grid-area).",
        r#""main""#,
    ),
];

/// `(key, allowed values)` for every closed enum the parser accepts. Style-key
/// enums are coherence-tested by authoring each value; node-field enums (`a.b`)
/// are exercised by the node examples.
const ENUM_REGISTRY: &[(&str, &[&str])] = &[
    (
        "surface",
        &[
            "card", "raised", "popover", "menu", "modal", "glass", "tooltip", "thumb",
        ],
    ),
    ("shadow", &["xs", "sm", "md", "lg", "xl"]),
    ("align", &["start", "center", "end", "baseline"]),
    ("justify", &["start", "center", "end", "between"]),
    ("text_align", &["start", "center", "end"]),
    ("glass_preset", &["glass"]),
    (
        "button.variant",
        &["primary", "secondary", "ghost", "danger"],
    ),
    ("status", &["accent", "danger", "warning", "success"]),
    ("drawer.side", &["left", "right", "top", "bottom"]),
    ("skeleton.kind", &["rect", "circle", "text"]),
];

/// Returns the grammar: the schema tag, every node type with a minimal example,
/// and the color roles a `ColorSpec` may name. Generated from one registry, so
/// it cannot drift from what the parser accepts.
pub fn describe_vocabulary() -> Vocabulary {
    Vocabulary {
        schema: SCHEMA_V1.to_string(),
        nodes: NODE_REGISTRY
            .iter()
            .map(|(tag, summary, example)| {
                // The icon node's known-name list is generated from the kit's
                // vendored registry, so the grammar can never advertise a name the
                // parser cannot resolve (or omit one it can).
                let summary = if *tag == "icon" {
                    format!(
                        "{summary} Known names: {}.",
                        fenestra_kit::icons::lucide::names()
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                } else {
                    (*summary).to_string()
                };
                NodeDoc {
                    tag: (*tag).to_string(),
                    summary,
                    example: (*example).to_string(),
                }
            })
            .collect(),
        color_roles: COLOR_ROLES.iter().map(|r| (*r).to_string()).collect(),
        style: STYLE_REGISTRY
            .iter()
            .map(|(key, summary, example)| StyleDoc {
                key: (*key).to_string(),
                summary: (*summary).to_string(),
                example: (*example).to_string(),
            })
            .collect(),
        enums: ENUM_REGISTRY
            .iter()
            .map(|(name, values)| EnumDoc {
                name: (*name).to_string(),
                values: values.iter().map(|v| (*v).to_string()).collect(),
            })
            .collect(),
    }
}
