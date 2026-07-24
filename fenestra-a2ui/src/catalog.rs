//! The A2UI v0.9 *basic catalog* component types
//! (<https://a2ui.org/specification/v0_9/catalogs/basic/catalog.json>):
//! eighteen components in a flat adjacency list, referencing children by
//! id and data through JSON Pointer bindings.

use serde::Deserialize;
use serde_json::Value;

/// One component definition: identity, layout weight, and the typed body.
///
/// Deserialization never fails a whole message over one bad component: a
/// definition whose body does not parse (a known component missing a
/// required field, a mistyped value) degrades to [`Kind::Unknown`] carrying
/// the raw JSON — rendered as a labeled placeholder with a note — while its
/// id, weight, and accessibility attributes are still honored. This keeps
/// the blast radius of malformed input to the one component that carried it
/// (A2UI's progressive spirit).
#[derive(Debug, Clone, Deserialize)]
#[serde(from = "Value")]
pub struct Component {
    /// Unique id within the surface; `root` anchors the tree.
    pub id: String,
    /// Relative flex weight within a Row/Column parent (CSS flex-grow).
    pub weight: Option<f64>,
    /// Accessibility attributes (advisory).
    pub accessibility: Option<Value>,
    /// The component body, discriminated by the `component` field.
    pub kind: Kind,
}

/// The strict shape [`Component`] tries first; failures fall back to
/// [`Kind::Unknown`] instead of erroring the message.
#[derive(Deserialize)]
struct TypedComponent {
    id: String,
    #[serde(default)]
    weight: Option<f64>,
    #[serde(default)]
    accessibility: Option<Value>,
    #[serde(flatten)]
    kind: Kind,
}

impl From<Value> for Component {
    fn from(v: Value) -> Self {
        match serde_json::from_value::<TypedComponent>(v.clone()) {
            Ok(t) => Self {
                id: t.id,
                weight: t.weight,
                accessibility: t.accessibility,
                kind: t.kind,
            },
            Err(_) => Self {
                id: v
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                weight: v.get("weight").and_then(Value::as_f64),
                accessibility: v.get("accessibility").cloned(),
                kind: Kind::Unknown(v),
            },
        }
    }
}

/// The component body. Unknown component names land in [`Kind::Unknown`]
/// so a stream from a newer/custom catalog degrades to a placeholder
/// instead of failing the whole surface (A2UI's progressive spirit).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "component")]
pub enum Kind {
    /// A text run; `variant` hints the base style, content supports
    /// simple Markdown.
    Text {
        /// The content (dynamic).
        text: Dyn<String>,
        /// `h1`..`h5`, `caption`, or `body` (default).
        #[serde(default)]
        variant: Option<String>,
    },
    /// A remote image. Rendered as a labeled placeholder: fenestra's
    /// deterministic headless renders never fetch the network.
    Image {
        /// The image URL (dynamic).
        url: Dyn<String>,
        /// Accessible description.
        #[serde(default)]
        description: Option<Dyn<String>>,
        /// Object-fit hint.
        #[serde(default)]
        fit: Option<String>,
        /// Size-class hint (`icon`, `avatar`, `smallFeature`, …).
        #[serde(default)]
        variant: Option<String>,
    },
    /// A named icon (mapped onto the vendored Lucide set where possible).
    Icon {
        /// The icon name (dynamic: a literal, a data binding, or a
        /// function call).
        name: Dyn<String>,
    },
    /// A remote video; rendered as a labeled placeholder (no network).
    Video {
        /// The video URL (dynamic).
        url: Dyn<String>,
    },
    /// A remote audio clip; rendered as a labeled placeholder.
    AudioPlayer {
        /// The audio URL (dynamic).
        url: Dyn<String>,
        /// Accessible description.
        #[serde(default)]
        description: Option<Dyn<String>>,
    },
    /// Horizontal layout.
    Row {
        /// Children by id, or a data-driven template.
        #[serde(default)]
        children: ChildList,
        /// Main-axis arrangement.
        #[serde(default)]
        justify: Option<String>,
        /// Cross-axis alignment.
        #[serde(default)]
        align: Option<String>,
    },
    /// Vertical layout.
    Column {
        /// Children by id, or a data-driven template.
        #[serde(default)]
        children: ChildList,
        /// Main-axis arrangement.
        #[serde(default)]
        justify: Option<String>,
        /// Cross-axis alignment.
        #[serde(default)]
        align: Option<String>,
    },
    /// A scrollable list of children.
    List {
        /// Children by id, or a data-driven template.
        #[serde(default)]
        children: ChildList,
        /// `vertical` (default) or `horizontal`.
        #[serde(default)]
        direction: Option<String>,
        /// Cross-axis alignment.
        #[serde(default)]
        align: Option<String>,
    },
    /// A surface-framed card around one child.
    Card {
        /// The child component id.
        child: String,
    },
    /// A tab strip; each tab titles one child subtree.
    Tabs {
        /// The tabs, in order.
        #[serde(default)]
        tabs: Vec<TabItem>,
    },
    /// A modal dialog: clicking `trigger` opens `content` over the UI.
    Modal {
        /// The always-visible trigger component id.
        trigger: String,
        /// The dialog content component id.
        content: String,
    },
    /// A hairline rule.
    Divider {
        /// `horizontal` (default) or `vertical`.
        #[serde(default)]
        axis: Option<String>,
    },
    /// A button wrapping one child (usually a Text), firing an action.
    Button {
        /// The child component id.
        child: String,
        /// `default`, `primary`, or `borderless`.
        #[serde(default)]
        variant: Option<String>,
        /// What clicking does.
        #[serde(default)]
        action: Option<Action>,
        /// Validation gates (parsed; enforcement is a noted gap).
        #[serde(default)]
        checks: Option<Value>,
    },
    /// A labeled text input, two-way bound when `value` is a path.
    TextField {
        /// The field label (dynamic).
        label: Dyn<String>,
        /// The value (dynamic; a path makes it two-way).
        #[serde(default)]
        value: Option<Dyn<String>>,
        /// `shortText` (default), `longText`, `number`, `obscured`.
        #[serde(default)]
        variant: Option<String>,
        /// Client-side validation regexp (parsed; enforcement noted).
        #[serde(default, rename = "validationRegexp")]
        validation_regexp: Option<String>,
        /// Validation gates (parsed; enforcement is a noted gap).
        #[serde(default)]
        checks: Option<Value>,
    },
    /// A labeled checkbox, two-way bound when `value` is a path.
    CheckBox {
        /// The label (dynamic).
        label: Dyn<String>,
        /// The checked state (dynamic; a path makes it two-way).
        value: Dyn<bool>,
        /// Validation gates (parsed; enforcement is a noted gap).
        #[serde(default)]
        checks: Option<Value>,
    },
    /// A single- or multi-select over labeled options.
    ChoicePicker {
        /// The picker label (dynamic).
        #[serde(default)]
        label: Option<Dyn<String>>,
        /// `mutuallyExclusive` or `multipleSelection`.
        #[serde(default)]
        variant: Option<String>,
        /// The options.
        #[serde(default)]
        options: Vec<ChoiceOption>,
        /// Selected values (dynamic string list; a path binds two-way).
        value: Value,
        /// `checkbox` or `chips` presentation hint.
        #[serde(default, rename = "displayStyle")]
        display_style: Option<String>,
        /// Whether the picker offers filtering.
        #[serde(default)]
        filterable: Option<bool>,
    },
    /// A numeric slider, two-way bound when `value` is a path.
    Slider {
        /// The label (dynamic).
        #[serde(default)]
        label: Option<Dyn<String>>,
        /// Range minimum (default 0).
        #[serde(default)]
        min: Option<f64>,
        /// Range maximum.
        max: f64,
        /// The value (dynamic; a path makes it two-way).
        value: Dyn<f64>,
    },
    /// A date and/or time input, two-way bound when `value` is a path.
    DateTimeInput {
        /// The value (dynamic ISO-8601 string).
        value: Dyn<String>,
        /// Whether the date part is editable.
        #[serde(default, rename = "enableDate")]
        enable_date: Option<bool>,
        /// Whether the time part is editable.
        #[serde(default, rename = "enableTime")]
        enable_time: Option<bool>,
        /// Range minimum (ISO-8601).
        #[serde(default)]
        min: Option<Value>,
        /// Range maximum (ISO-8601).
        #[serde(default)]
        max: Option<Value>,
        /// The label (dynamic).
        #[serde(default)]
        label: Option<Dyn<String>>,
    },
    /// Any component name this catalog build doesn't know: rendered as a
    /// labeled placeholder, recorded as a note.
    #[serde(untagged)]
    Unknown(Value),
}

/// One tab of a [`Kind::Tabs`].
#[derive(Debug, Clone, Deserialize)]
pub struct TabItem {
    /// The tab title (dynamic).
    pub title: Dyn<String>,
    /// The tab's content component id.
    pub child: String,
}

/// One [`Kind::ChoicePicker`] option.
#[derive(Debug, Clone, Deserialize)]
pub struct ChoiceOption {
    /// The user-visible label (dynamic).
    pub label: Dyn<String>,
    /// The value stored when selected.
    pub value: String,
}

/// A dynamic value: a literal, a JSON Pointer data binding, or a function
/// call (`formatString` & friends).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Dyn<T> {
    /// A literal value.
    Lit(T),
    /// A binding: resolve `path` against the surface data model (relative
    /// paths resolve against the current template scope).
    Binding {
        /// JSON Pointer, absolute (`/user/name`) or scope-relative.
        path: String,
    },
    /// A client-side function producing the value.
    Call(FunctionCall),
}

/// A function call in a dynamic value. `formatString` interpolation is
/// implemented; other calls resolve to a placeholder with a note.
#[derive(Debug, Clone, Deserialize)]
pub struct FunctionCall {
    /// The function name (e.g. `formatString`).
    pub call: String,
    /// Named arguments.
    #[serde(default)]
    pub args: serde_json::Map<String, Value>,
    /// Declared return type, when present.
    #[serde(default, rename = "returnType")]
    pub return_type: Option<String>,
}

/// Children of a layout component: a static id list or a template
/// generating one child per item of a data-model list.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ChildList {
    /// A fixed list of child component ids.
    Static(Vec<String>),
    /// One rendered child per item of the list at `path`, using
    /// `componentId` as the template (relative bindings inside it resolve
    /// against each item).
    Template {
        /// The template component id.
        #[serde(rename = "componentId")]
        component_id: String,
        /// JSON Pointer to the data-model list.
        path: String,
    },
}

impl Default for ChildList {
    fn default() -> Self {
        Self::Static(Vec::new())
    }
}

/// A user-interaction handler: a server-bound event or a local function.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Action {
    /// Dispatch a named event (plus context) to the agent.
    Event {
        /// The event payload.
        event: EventSpec,
    },
    /// Run a client-side function (e.g. `openUrl`).
    FunctionCall {
        /// The call payload.
        #[serde(rename = "functionCall")]
        function_call: FunctionCall,
    },
}

/// The server-bound half of an [`Action`].
#[derive(Debug, Clone, Deserialize)]
pub struct EventSpec {
    /// The action name the agent dispatches on.
    pub name: String,
    /// Key-value context sent with the event; dynamic values resolve
    /// against the data model first.
    #[serde(default)]
    pub context: Option<Value>,
}
