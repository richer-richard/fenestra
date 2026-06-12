//! Semantic queries over a built frame: find widgets the way users and
//! assistive technology perceive them, instead of by coordinates.
//!
//! The query vocabulary follows the priority order the web's Testing
//! Library proved out: prefer [`by::role`] (with [`Query::name`]), then
//! [`by::label`], then [`by::value`]; reach for [`by::id`] (the test-id
//! escape hatch) last. [`Frame::get`] is strict like a Playwright
//! locator — zero or several matches panic, and the panic message dumps
//! the accessibility tree so the failure is self-explaining.
//!
//! ```
//! use fenestra_core::{Semantics, by, col};
//! use fenestra_core::{Fonts, FrameState, Theme, build_frame};
//! let view: fenestra_core::Element<()> = col().children([
//!     fenestra_core::text("Hello"),
//! ]);
//! let mut fonts = Fonts::embedded();
//! let mut state = FrameState::new();
//! let frame = build_frame(&view, &Theme::light(), &mut fonts, &mut state, (200.0, 100.0), 1.0);
//! assert!(frame.query(&by::label("Hello")).is_some());
//! assert!(frame.query(&by::role(Semantics::Button)).is_none());
//! ```

use crate::element::Semantics;
use crate::frame::{AccessNode, Frame};

/// How a text criterion matches an accessible string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextMatch {
    /// The whole string, exactly.
    Exact(String),
    /// Any substring.
    Contains(String),
}

impl TextMatch {
    fn matches(&self, hay: Option<&str>) -> bool {
        match (self, hay) {
            (Self::Exact(needle), Some(hay)) => hay == needle,
            (Self::Contains(needle), Some(hay)) => hay.contains(needle),
            (_, None) => false,
        }
    }
}

impl std::fmt::Display for TextMatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exact(s) => write!(f, "{s:?}"),
            Self::Contains(s) => write!(f, "*{s:?}*"),
        }
    }
}

/// A semantic query against the frame's accessibility tree. Build one
/// with the [`by`] constructors; refine role queries with
/// [`Query::name`]. All set criteria must match (AND).
#[derive(Debug, Clone)]
pub struct Query {
    role: Option<Semantics>,
    label: Option<TextMatch>,
    value: Option<TextMatch>,
    key: Option<String>,
}

/// Query constructors, in the order you should prefer them.
pub mod by {
    use super::{Query, TextMatch};
    use crate::element::Semantics;

    /// Matches by role: the payload is ignored, so
    /// `by::role(Semantics::Checkbox { checked: false })` finds every
    /// checkbox. Refine with [`Query::name`] for the accessible name.
    pub fn role(role: Semantics) -> Query {
        Query {
            role: Some(role),
            label: None,
            value: None,
            key: None,
        }
    }

    /// Matches the accessible name exactly. Text leaves expose their
    /// content as their label, so this also finds static text.
    pub fn label(label: impl Into<String>) -> Query {
        Query {
            role: None,
            label: Some(TextMatch::Exact(label.into())),
            value: None,
            key: None,
        }
    }

    /// Matches any substring of the accessible name.
    pub fn label_contains(label: impl Into<String>) -> Query {
        Query {
            role: None,
            label: Some(TextMatch::Contains(label.into())),
            value: None,
            key: None,
        }
    }

    /// Matches an input's current value exactly.
    pub fn value(value: impl Into<String>) -> Query {
        Query {
            role: None,
            label: None,
            value: Some(TextMatch::Exact(value.into())),
            key: None,
        }
    }

    /// Matches any substring of an input's current value.
    pub fn value_contains(value: impl Into<String>) -> Query {
        Query {
            role: None,
            label: None,
            value: Some(TextMatch::Contains(value.into())),
            key: None,
        }
    }

    /// Matches the stable key set with `.id("...")` — the test-id escape
    /// hatch. Users can't see keys; prefer [`role`] or [`label`].
    pub fn id(key: impl Into<String>) -> Query {
        Query {
            role: None,
            label: None,
            value: None,
            key: Some(key.into()),
        }
    }
}

impl Query {
    /// Refines the query with an exact accessible name (Playwright's
    /// `getByRole(role, { name })`).
    pub fn name(mut self, label: impl Into<String>) -> Self {
        self.label = Some(TextMatch::Exact(label.into()));
        self
    }

    /// Refines the query with an accessible-name substring.
    pub fn name_contains(mut self, label: impl Into<String>) -> Self {
        self.label = Some(TextMatch::Contains(label.into()));
        self
    }

    fn matches(&self, node: &AccessNode) -> bool {
        if let Some(role) = &self.role {
            let Some(semantics) = &node.semantics else {
                return false;
            };
            if std::mem::discriminant(role) != std::mem::discriminant(semantics) {
                return false;
            }
        }
        if let Some(label) = &self.label
            && !label.matches(node.label.as_deref())
        {
            return false;
        }
        if let Some(value) = &self.value
            && !value.matches(node.value.as_deref())
        {
            return false;
        }
        if let Some(key) = &self.key
            && node.key.as_deref() != Some(key.as_str())
        {
            return false;
        }
        true
    }
}

impl std::fmt::Display for Query {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if let Some(role) = &self.role {
            parts.push(format!("role={}", role_name(role)));
        }
        if let Some(label) = &self.label {
            parts.push(format!("name={label}"));
        }
        if let Some(value) = &self.value {
            parts.push(format!("value={value}"));
        }
        if let Some(key) = &self.key {
            parts.push(format!("id={key:?}"));
        }
        write!(f, "{}", parts.join(" "))
    }
}

/// The lowercase role word used in [`Frame::access_yaml`] and query
/// errors (ARIA vocabulary where one exists).
pub(crate) fn role_name(semantics: &Semantics) -> &'static str {
    match semantics {
        Semantics::Button => "button",
        Semantics::Checkbox { .. } => "checkbox",
        Semantics::Switch { .. } => "switch",
        Semantics::Radio { .. } => "radio",
        Semantics::Slider { .. } => "slider",
        Semantics::TextInput { .. } => "textbox",
        Semantics::ComboBox => "combobox",
        Semantics::Dialog => "dialog",
        Semantics::Tab { .. } => "tab",
        Semantics::Alert => "alert",
        Semantics::Label => "text",
        Semantics::Image => "image",
    }
}

/// Why a strict lookup failed — the non-panicking form used by
/// machine-driven harnesses (scenario scripts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryError {
    /// Nothing matched.
    NoMatch,
    /// Several nodes matched (the count) — refine the query.
    Ambiguous(usize),
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoMatch => write!(f, "no node matches"),
            Self::Ambiguous(n) => write!(f, "{n} nodes match (ambiguous)"),
        }
    }
}

fn collect<'t>(node: &'t AccessNode, q: &Query, out: &mut Vec<&'t AccessNode>) {
    if q.matches(node) {
        out.push(node);
    }
    for child in &node.children {
        collect(child, q, out);
    }
}

impl Frame {
    /// All nodes matching the query, in tree (paint) order.
    pub fn get_all(&self, q: &Query) -> Vec<AccessNode> {
        let root = self.access_tree();
        let mut out = Vec::new();
        collect(&root, q, &mut out);
        out.into_iter().cloned().collect()
    }

    /// The single matching node, without panicking — scenario runners
    /// and other machine drivers turn the error into their own report.
    ///
    /// # Errors
    /// [`QueryError::NoMatch`] when nothing matches,
    /// [`QueryError::Ambiguous`] when several nodes do.
    pub fn try_get(&self, q: &Query) -> Result<AccessNode, QueryError> {
        let mut all = self.get_all(q);
        match all.len() {
            0 => Err(QueryError::NoMatch),
            1 => Ok(all.pop().expect("len checked")),
            n => Err(QueryError::Ambiguous(n)),
        }
    }

    /// The single matching node, or `None` when nothing matches. Use to
    /// assert absence; panics if the query is ambiguous (several
    /// matches), because a test that silently picks one is lying.
    ///
    /// # Panics
    /// If more than one node matches.
    pub fn query(&self, q: &Query) -> Option<AccessNode> {
        match self.try_get(q) {
            Ok(node) => Some(node),
            Err(QueryError::NoMatch) => None,
            Err(QueryError::Ambiguous(n)) => panic!(
                "query [{q}] is ambiguous: {n} matches\n\
                 accessibility tree:\n{}",
                self.access_yaml()
            ),
        }
    }

    /// The single matching node. Strict: zero or several matches panic,
    /// and the message includes the full accessibility tree, so the
    /// failure explains itself.
    ///
    /// # Panics
    /// If no node or more than one node matches.
    pub fn get(&self, q: &Query) -> AccessNode {
        match self.query(q) {
            Some(node) => node,
            None => panic!(
                "no node matches [{q}]\naccessibility tree:\n{}",
                self.access_yaml()
            ),
        }
    }

    /// The accessibility tree as indented YAML in Playwright's
    /// aria-snapshot grammar: `- role "name" [attr=value]`. Containers
    /// without semantics, label, value, or key are flattened away, so the
    /// output stays signal-dense and stable. Snapshot it with insta to
    /// lock a screen's accessible structure.
    pub fn access_yaml(&self) -> String {
        fn attrs(node: &AccessNode) -> String {
            let mut out = String::new();
            match node.semantics {
                Some(Semantics::Checkbox { checked: true }) => out.push_str(" [checked]"),
                Some(Semantics::Switch { on: true }) => out.push_str(" [on]"),
                Some(Semantics::Radio { selected: true })
                | Some(Semantics::Tab { selected: true }) => out.push_str(" [selected]"),
                Some(Semantics::Slider { value, min, max }) => {
                    out.push_str(&format!(" [value={value} min={min} max={max}]"));
                }
                Some(Semantics::TextInput { multiline: true }) => out.push_str(" [multiline]"),
                _ => {}
            }
            if let Some(value) = &node.value
                && !value.is_empty()
            {
                out.push_str(&format!(" [value={value:?}]"));
            }
            out
        }
        fn emit(node: &AccessNode, depth: usize, out: &mut String) {
            let interesting = node.semantics.is_some()
                || node.label.is_some()
                || node.value.is_some()
                || node.key.is_some();
            let child_depth = if interesting {
                let role = node
                    .semantics
                    .as_ref()
                    .map_or("generic", crate::query::role_name);
                out.push_str(&"  ".repeat(depth));
                out.push('-');
                out.push(' ');
                out.push_str(role);
                if let Some(label) = &node.label {
                    out.push_str(&format!(" {label:?}"));
                }
                out.push_str(&attrs(node));
                if let Some(key) = &node.key {
                    out.push_str(&format!(" #{key}"));
                }
                out.push('\n');
                depth + 1
            } else {
                depth
            };
            for child in &node.children {
                emit(child, child_depth, out);
            }
        }
        let mut out = String::new();
        emit(&self.access_tree(), 0, &mut out);
        if out.is_empty() {
            out.push_str("(empty accessibility tree)\n");
        }
        out
    }
}
