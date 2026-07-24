//! Surface → `Element` rendering: the A2UI basic catalog mapped onto
//! fenestra-kit widgets, with data bindings resolved against the surface
//! data model. Everything that cannot map faithfully renders a labeled
//! placeholder and records a note — silence means fidelity.

use fenestra_core::{Element, TextSize, Theme, Weight, col, div, divider, row, text};
use fenestra_kit::{
    ButtonVariant, button, card, checkbox, field, icon_button, modal, multi_select, select, slider,
    tabs, text_area, text_input,
};
use serde_json::Value;

use crate::catalog::{Action, ChildList, ChoiceOption, Component, Dyn, FunctionCall, Kind};
use crate::functions;
use crate::surface::Surface;

/// The deepest component chain the renderer follows. True cycles are
/// caught exactly by the render-path stack (see [`render_by_id`]); this
/// cap bounds legitimate-but-absurd nesting so the produced *element*
/// tree stays well inside `fenestra_core::MAX_TREE_DEPTH` (each catalog
/// component lowers to roughly 1–3 element levels).
const MAX_DEPTH: usize = 16;

/// The most children one template expansion materializes.
const MAX_TEMPLATE_CHILDREN: usize = 1000;

/// Messages the rendered surface emits; feed them to [`Surface::handle`].
#[derive(Clone, Debug)]
pub enum A2uiMsg {
    /// Write a string at an absolute data-model path (two-way binding).
    SetString {
        /// Absolute JSON Pointer.
        path: String,
        /// The new value.
        value: String,
    },
    /// Write a boolean at an absolute data-model path.
    SetBool {
        /// Absolute JSON Pointer.
        path: String,
        /// The new value.
        value: bool,
    },
    /// Write a number at an absolute data-model path.
    SetNumber {
        /// Absolute JSON Pointer.
        path: String,
        /// The new value.
        value: f64,
    },
    /// Write a string list at an absolute data-model path.
    SetList {
        /// Absolute JSON Pointer.
        path: String,
        /// The new values.
        values: Vec<String>,
    },
    /// Store a local edit for a literal-valued input (no binding path).
    LocalEdit {
        /// The input component id.
        id: String,
        /// The edited value.
        value: Value,
    },
    /// A server-bound action fired (button click).
    Event {
        /// The action name.
        name: String,
        /// Resolved context payload.
        context: Value,
        /// The id of the component that fired the action — what the
        /// client→server action message's `sourceComponentId` requires
        /// (see [`Surface::action_message`]).
        source_id: String,
    },
    /// A local `openUrl` function action.
    OpenUrl(
        /// The URL to open.
        String,
    ),
    /// Open a Modal component.
    OpenModal(
        /// The Modal component id.
        String,
    ),
    /// Close a Modal component.
    CloseModal(
        /// The Modal component id.
        String,
    ),
    /// Switch a Tabs component to a tab.
    SelectTab {
        /// The Tabs component id.
        id: String,
        /// The new active index.
        index: usize,
    },
}

/// What [`Surface::handle`] hands back to the host: the effects the host
/// (agent transport, OS integration) must carry out.
#[derive(Clone, Debug)]
pub enum A2uiSignal {
    /// Dispatch this action event to the agent (the client→server
    /// `action` message; see [`Surface::action_message`], which takes
    /// `source_id` as its `sourceComponentId`).
    Event {
        /// The action name.
        name: String,
        /// Resolved context payload.
        context: Value,
        /// The full data model, when the surface asked to send it.
        data_model: Option<Value>,
        /// The id of the component that fired the action.
        source_id: String,
    },
    /// Open a URL with the platform opener.
    OpenUrl(
        /// The URL.
        String,
    ),
}

/// A rendered surface: the element tree plus render-time fidelity notes.
pub struct Rendered {
    /// The tree, ready for any fenestra runner or headless render.
    pub element: Element<A2uiMsg>,
    /// Render-time notes (unknown components, unresolved calls,
    /// truncations). Empty means every component mapped cleanly.
    pub notes: Vec<String>,
}

struct Ctx<'a> {
    surface: &'a Surface,
    theme: &'a Theme,
    notes: std::cell::RefCell<Vec<String>>,
    /// The id chain currently being rendered: exact cycle detection
    /// (`a → b → a` trips on re-entry, not after burning stack).
    path_stack: std::cell::RefCell<Vec<String>>,
}

impl Ctx<'_> {
    fn note(&self, id: &str, msg: impl std::fmt::Display) {
        self.notes.borrow_mut().push(format!("{id}: {msg}"));
    }
}

impl Surface {
    /// Renders the surface's component tree. Missing `root` renders an
    /// empty placeholder with a note (progressive streams may simply not
    /// have delivered it yet).
    #[must_use]
    pub fn render(&self, theme: &Theme) -> Rendered {
        let ctx = Ctx {
            surface: self,
            theme,
            notes: std::cell::RefCell::new(Vec::new()),
            path_stack: std::cell::RefCell::new(Vec::new()),
        };
        let element = if self.components.contains_key("root") {
            render_by_id(&ctx, "root", None, 0)
        } else {
            ctx.note("root", "no root component yet (stream incomplete?)");
            col()
        };
        Rendered {
            element,
            notes: ctx.notes.into_inner(),
        }
    }

    /// Applies one rendered-surface message: binding writes and UI state
    /// mutate the surface; agent-facing effects come back as a signal.
    pub fn handle(&mut self, msg: A2uiMsg) -> Option<A2uiSignal> {
        match msg {
            A2uiMsg::SetString { path, value } => {
                self.write(&path, Some(Value::String(value)));
                None
            }
            A2uiMsg::SetBool { path, value } => {
                self.write(&path, Some(Value::Bool(value)));
                None
            }
            A2uiMsg::SetNumber { path, value } => {
                self.write(
                    &path,
                    serde_json::Number::from_f64(value).map(Value::Number),
                );
                None
            }
            A2uiMsg::SetList { path, values } => {
                self.write(
                    &path,
                    Some(Value::Array(
                        values.into_iter().map(Value::String).collect(),
                    )),
                );
                None
            }
            A2uiMsg::LocalEdit { id, value } => {
                self.ui.local_edits.insert(id, value);
                None
            }
            A2uiMsg::Event {
                name,
                context,
                source_id,
            } => Some(A2uiSignal::Event {
                name,
                context,
                data_model: self.send_data_model.then(|| self.data.clone()),
                source_id,
            }),
            A2uiMsg::OpenUrl(url) => Some(A2uiSignal::OpenUrl(url)),
            A2uiMsg::OpenModal(id) => {
                self.ui.open_modals.insert(id);
                None
            }
            A2uiMsg::CloseModal(id) => {
                self.ui.open_modals.remove(&id);
                None
            }
            A2uiMsg::SelectTab { id, index } => {
                self.ui.active_tabs.insert(id, index);
                None
            }
        }
    }

    /// Builds the client→server `action` message for an
    /// [`A2uiSignal::Event`], per the v0.9 `client_to_server` schema.
    /// `timestamp` is caller-supplied (ISO-8601) to keep this crate
    /// clock-free and deterministic.
    #[must_use]
    pub fn action_message(
        &self,
        name: &str,
        source_component_id: &str,
        context: &Value,
        timestamp: &str,
    ) -> Value {
        serde_json::json!({
            "name": name,
            "surfaceId": self.id,
            "sourceComponentId": source_component_id,
            "timestamp": timestamp,
            "context": context,
        })
    }
}

// ── Dynamic-value resolution ──────────────────────────────────────────────

/// The one canonical path joiner: absolute paths stand alone; relative
/// paths resolve under the collection scope, or from the root without one.
/// Reads ([`lookup`]), template item scopes ([`children_of`]), and binding
/// *write* paths all go through here, so a value always reads back from
/// exactly where its two-way binding writes.
fn absolute(path: &str, scope: Option<&str>) -> String {
    if path.starts_with('/') {
        path.to_owned()
    } else {
        match scope {
            Some(s) => format!("{s}/{path}"),
            None => format!("/{path}"),
        }
    }
}

fn lookup<'a>(surface: &'a Surface, path: &str, scope: Option<&str>) -> Option<&'a Value> {
    surface.data().pointer(&absolute(path, scope))
}

fn resolve_value(ctx: &Ctx, id: &str, d: &Dyn<String>, scope: Option<&str>) -> String {
    match d {
        Dyn::Lit(s) => s.clone(),
        Dyn::Binding { path } => match lookup(ctx.surface, path, scope) {
            Some(v) => functions::display(v),
            None => {
                ctx.note(id, format!("binding {path:?} resolves to nothing"));
                String::new()
            }
        },
        Dyn::Call(call) => resolve_call(ctx, id, call, scope),
    }
}

/// A bound boolean input value. Absent stays silently `false` — form
/// values legitimately start unset — but a present non-boolean is always
/// an authoring error and records a note.
fn bound_bool(ctx: &Ctx, id: &str, path: &str, scope: Option<&str>) -> bool {
    match lookup(ctx.surface, path, scope) {
        None => false,
        Some(v) => v.as_bool().unwrap_or_else(|| {
            ctx.note(id, format!("binding {path:?} is not a boolean; false"));
            false
        }),
    }
}

/// A bound numeric input value; same note policy as [`bound_bool`].
fn bound_f64(ctx: &Ctx, id: &str, path: &str, scope: Option<&str>, fallback: f64) -> f64 {
    match lookup(ctx.surface, path, scope) {
        None => fallback,
        Some(v) => v.as_f64().unwrap_or_else(|| {
            ctx.note(id, format!("binding {path:?} is not a number; {fallback}"));
            fallback
        }),
    }
}

fn resolve_bool(ctx: &Ctx, id: &str, d: &Dyn<bool>, scope: Option<&str>) -> bool {
    match d {
        Dyn::Lit(b) => *b,
        Dyn::Binding { path } => bound_bool(ctx, id, path, scope),
        Dyn::Call(call) => {
            ctx.note(
                id,
                format!("function {:?} in a boolean slot; false", call.call),
            );
            false
        }
    }
}

fn resolve_f64(ctx: &Ctx, id: &str, d: &Dyn<f64>, scope: Option<&str>) -> f64 {
    match d {
        Dyn::Lit(n) => *n,
        Dyn::Binding { path } => bound_f64(ctx, id, path, scope, 0.0),
        Dyn::Call(call) => {
            ctx.note(id, format!("function {:?} in a numeric slot; 0", call.call));
            0.0
        }
    }
}

fn arg_value(
    ctx: &Ctx,
    id: &str,
    args: &serde_json::Map<String, Value>,
    key: &str,
    scope: Option<&str>,
) -> Value {
    match args.get(key) {
        Some(Value::Object(o)) if o.contains_key("path") => {
            let path = o["path"].as_str().unwrap_or_default();
            lookup(ctx.surface, path, scope)
                .cloned()
                .unwrap_or(Value::Null)
        }
        Some(v) => v.clone(),
        None => {
            let _ = id;
            Value::Null
        }
    }
}

fn resolve_call(ctx: &Ctx, id: &str, call: &FunctionCall, scope: Option<&str>) -> String {
    match call.call.as_str() {
        "formatString" => {
            let template = match call.args.get("value") {
                Some(Value::String(s)) => s.clone(),
                _ => String::new(),
            };
            interpolate(ctx, id, &template, scope)
        }
        "formatNumber" => {
            let v = arg_value(ctx, id, &call.args, "value", scope);
            functions::format_number(v.as_f64().unwrap_or(0.0))
        }
        "formatCurrency" => {
            let v = arg_value(ctx, id, &call.args, "value", scope);
            let currency = call
                .args
                .get("currency")
                .and_then(Value::as_str)
                .unwrap_or("USD");
            functions::format_currency(v.as_f64().unwrap_or(0.0), currency)
        }
        "formatDate" => {
            let v = arg_value(ctx, id, &call.args, "value", scope);
            let pattern = call
                .args
                .get("format")
                .and_then(Value::as_str)
                .unwrap_or("yyyy-MM-dd");
            let raw = functions::display(&v);
            functions::format_date(&raw, pattern).unwrap_or_else(|| {
                ctx.note(id, format!("formatDate could not parse {raw:?}"));
                raw
            })
        }
        "pluralize" => {
            let v = arg_value(ctx, id, &call.args, "value", scope);
            let one = call.args.get("one").and_then(Value::as_str).unwrap_or("");
            let other = call.args.get("other").and_then(Value::as_str).unwrap_or("");
            functions::pluralize(v.as_f64().unwrap_or(0.0), one, other)
        }
        other => {
            ctx.note(id, format!("function {other:?} is not implemented"));
            format!("[{other}]")
        }
    }
}

/// `${…}` interpolation for `formatString`: absolute/relative data paths
/// resolve; nested function-call syntax (`${fn(…)}`) is beyond this pass
/// and resolves to nothing, with a note. Braces balance, so a nested
/// expression is skipped whole rather than split at its first `}`.
fn interpolate(ctx: &Ctx, id: &str, template: &str, scope: Option<&str>) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        // Find the matching close brace, counting nested `${`/`}` pairs.
        let mut depth = 1_usize;
        let mut end = None;
        let bytes = after.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(i);
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        let Some(end) = end else {
            out.push_str(&rest[start..]);
            return out;
        };
        let expr = &after[..end];
        if expr.contains('(') {
            ctx.note(
                id,
                format!("nested call in formatString template ({expr:?}) is not implemented"),
            );
        } else {
            match lookup(ctx.surface, expr, scope) {
                Some(v) => out.push_str(&functions::display(v)),
                None => ctx.note(id, format!("template path {expr:?} resolves to nothing")),
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

// ── Component rendering ───────────────────────────────────────────────────

/// Trims a placeholder label to a displayable length (char-safe).
fn truncate_label(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{cut}…")
}

fn placeholder<Msg: 'static>(label: String, theme: &Theme) -> Element<Msg> {
    let border = theme.border_subtle;
    let muted = theme.text_muted;
    div()
        .p(8.0)
        .rounded(4.0)
        .bg(theme.surface)
        .child(text(label).size(TextSize::Sm).color(muted))
        .border(1.0, border)
}

fn render_by_id(ctx: &Ctx, id: &str, scope: Option<&str>, depth: usize) -> Element<A2uiMsg> {
    if ctx.path_stack.borrow().iter().any(|p| p == id) {
        ctx.note(
            id,
            "reference cycle detected (depth cap); rendering a placeholder",
        );
        return placeholder(format!("[cycle: {id}]"), ctx.theme);
    }
    if depth > MAX_DEPTH {
        ctx.note(
            id,
            "component chain exceeds the depth cap; rendering a placeholder",
        );
        return placeholder(format!("[deep: {id}]"), ctx.theme);
    }
    let Some(component) = ctx.surface.components.get(id) else {
        ctx.note(id, "referenced component is not defined");
        return placeholder(format!("[missing: {id}]"), ctx.theme);
    };
    ctx.path_stack.borrow_mut().push(id.to_owned());
    let el = render_component(ctx, component, scope, depth);
    ctx.path_stack.borrow_mut().pop();
    match component.weight {
        #[expect(clippy::cast_possible_truncation, reason = "flex weights are small")]
        Some(w) if w > 0.0 => el.grow_by(w as f32),
        _ => el,
    }
}

fn children_of(
    ctx: &Ctx,
    id: &str,
    list: &ChildList,
    scope: Option<&str>,
    depth: usize,
) -> Vec<Element<A2uiMsg>> {
    match list {
        ChildList::Static(ids) => ids
            .iter()
            .map(|cid| render_by_id(ctx, cid, scope, depth + 1))
            .collect(),
        ChildList::Template { component_id, path } => {
            let Some(Value::Array(items)) = lookup(ctx.surface, path, scope) else {
                ctx.note(id, format!("template path {path:?} is not a list"));
                return Vec::new();
            };
            if items.len() > MAX_TEMPLATE_CHILDREN {
                ctx.note(
                    id,
                    format!(
                        "{} template items exceed the cap ({MAX_TEMPLATE_CHILDREN}); extra items dropped",
                        items.len()
                    ),
                );
            }
            // The canonical join: an absolute template path stays absolute
            // even inside a collection scope (a naive `{scope}/{path}` join
            // used to corrupt it into a `//` pointer).
            let base = absolute(path, scope);
            (0..items.len().min(MAX_TEMPLATE_CHILDREN))
                .map(|i| {
                    let item_scope = format!("{base}/{i}");
                    render_by_id(ctx, component_id, Some(&item_scope), depth + 1)
                })
                .collect()
        }
    }
}

fn apply_flex(
    mut el: Element<A2uiMsg>,
    justify: Option<&str>,
    align: Option<&str>,
    id: &str,
    ctx: &Ctx,
) -> Element<A2uiMsg> {
    el = match justify {
        Some("center") => el.justify_center(),
        Some("end") => el.justify_end(),
        Some("spaceBetween") => el.justify_between(),
        Some("spaceAround" | "spaceEvenly") => {
            ctx.note(id, "spaceAround/spaceEvenly approximate as spaceBetween");
            el.justify_between()
        }
        Some("stretch") | Some("start") | None => el,
        Some(other) => {
            ctx.note(id, format!("unknown justify {other:?}"));
            el
        }
    };
    match align {
        Some("center") => el.items_center(),
        Some("end") => el.items_end(),
        Some("start") => el.items_start(),
        Some("stretch") | None => el,
        Some(other) => {
            ctx.note(id, format!("unknown align {other:?}"));
            el
        }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one arm per catalog component; splitting would scatter the mapping"
)]
fn render_component(
    ctx: &Ctx,
    component: &Component,
    scope: Option<&str>,
    depth: usize,
) -> Element<A2uiMsg> {
    let id = component.id.as_str();
    let theme = ctx.theme;
    match &component.kind {
        Kind::Text {
            text: content,
            variant,
        } => {
            let resolved = resolve_value(ctx, id, content, scope);
            match variant.as_deref() {
                Some("h1") => text(resolved).size_px(28.0).weight(Weight::Semibold),
                Some("h2") => text(resolved).size_px(22.0).weight(Weight::Semibold),
                Some("h3") => text(resolved).size_px(18.0).weight(Weight::Semibold),
                Some("h4") => text(resolved).size_px(16.0).weight(Weight::Medium),
                Some("h5") => text(resolved).size_px(14.0).weight(Weight::Medium),
                Some("caption") => text(resolved).size(TextSize::Xs).color(theme.text_muted),
                // Body text supports simple Markdown per the catalog docs.
                _ => fenestra_markdown::markdown(resolved).into(),
            }
        }
        Kind::Image {
            url,
            description,
            variant,
            ..
        } => {
            // Deterministic headless renders never fetch the network: a
            // labeled placeholder stands in, sized by the variant hint.
            let desc = description
                .as_ref()
                .map(|d| resolve_value(ctx, id, d, scope))
                .filter(|d| !d.is_empty())
                .unwrap_or_else(|| resolve_value(ctx, id, url, scope));
            let (w, h) = match variant.as_deref() {
                Some("icon") => (24.0, 24.0),
                Some("avatar") => (40.0, 40.0),
                Some("smallFeature") => (80.0, 80.0),
                Some("largeFeature") => (240.0, 180.0),
                Some("header") => (320.0, 120.0),
                _ => (160.0, 120.0),
            };
            let short = truncate_label(&desc, 36);
            let el = div()
                .w(w)
                .h(h)
                .rounded(if variant.as_deref() == Some("avatar") {
                    w / 2.0
                } else {
                    4.0
                })
                .bg(theme.surface)
                .border(1.0, theme.border_subtle)
                .items_center()
                .justify_center()
                .overflow_hidden()
                .child(
                    text(format!("[img: {short}]"))
                        .size(TextSize::Xs)
                        .color(theme.text_muted),
                );
            el.label(desc)
        }
        Kind::Icon { name } => {
            let name = resolve_value(ctx, id, name, scope);
            match fenestra_kit::icons::lucide::by_name(&name) {
                Some(icon) => icon.label(name),
                None => {
                    ctx.note(
                        id,
                        format!("icon {name:?} is not in the vendored Lucide set"),
                    );
                    placeholder(format!("[icon: {name}]"), theme)
                }
            }
        }
        Kind::Video { url } => {
            let url = resolve_value(ctx, id, url, scope);
            placeholder(format!("[video: {}]", truncate_label(&url, 48)), theme)
        }
        Kind::AudioPlayer { url, description } => {
            let label = description
                .as_ref()
                .map(|d| resolve_value(ctx, id, d, scope))
                .filter(|d| !d.is_empty())
                .unwrap_or_else(|| resolve_value(ctx, id, url, scope));
            placeholder(format!("[audio: {}]", truncate_label(&label, 48)), theme)
        }
        Kind::Row {
            children,
            justify,
            align,
        } => {
            let kids = children_of(ctx, id, children, scope, depth);
            apply_flex(
                row().gap(8.0).children(kids),
                justify.as_deref(),
                align.as_deref(),
                id,
                ctx,
            )
        }
        Kind::Column {
            children,
            justify,
            align,
        } => {
            let kids = children_of(ctx, id, children, scope, depth);
            apply_flex(
                col().gap(8.0).children(kids),
                justify.as_deref(),
                align.as_deref(),
                id,
                ctx,
            )
        }
        Kind::List {
            children,
            direction,
            align,
        } => {
            let kids = children_of(ctx, id, children, scope, depth);
            let horizontal = direction.as_deref() == Some("horizontal");
            let el = if horizontal {
                row().gap(8.0).children(kids).scroll_x()
            } else {
                col().gap(8.0).children(kids).scroll_y()
            };
            apply_flex(el.id(id), None, align.as_deref(), id, ctx)
        }
        Kind::Card { child } => card()
            .child(render_by_id(ctx, child, scope, depth + 1))
            .p(16.0),
        Kind::Tabs { tabs: items } => {
            let labels: Vec<String> = items
                .iter()
                .map(|t| resolve_value(ctx, id, &t.title, scope))
                .collect();
            let active = ctx
                .surface
                .ui
                .active_tabs
                .get(id)
                .copied()
                .unwrap_or(0)
                .min(items.len().saturating_sub(1));
            let strip = {
                let tabs_id = id.to_owned();
                tabs(active, labels, move |index| A2uiMsg::SelectTab {
                    id: tabs_id.clone(),
                    index,
                })
            };
            let mut container = col().gap(8.0).child(strip);
            if let Some(tab) = items.get(active) {
                container = container.child(render_by_id(ctx, &tab.child, scope, depth + 1));
            }
            container
        }
        Kind::Modal { trigger, content } => {
            let open = ctx.surface.ui.open_modals.contains(id);
            let trigger_el = render_by_id(ctx, trigger, scope, depth + 1);
            let wrapped = div()
                .child(trigger_el)
                .on_click(A2uiMsg::OpenModal(id.to_owned()));
            if open {
                col().children((
                    wrapped,
                    modal("")
                        .child(render_by_id(ctx, content, scope, depth + 1))
                        .on_close(A2uiMsg::CloseModal(id.to_owned())),
                ))
            } else {
                wrapped
            }
        }
        Kind::Divider { axis } => {
            if axis.as_deref() == Some("vertical") {
                div().w(1.0).h_full().bg(theme.border_subtle)
            } else {
                divider()
            }
        }
        Kind::Button {
            child,
            variant,
            action,
            ..
        } => {
            // Extract a text label when the child is a Text component; any
            // other child renders inside an icon button.
            let child_component = ctx.surface.components.get(child);
            let label = match child_component.map(|c| &c.kind) {
                Some(Kind::Text { text: content, .. }) => {
                    Some(resolve_value(ctx, id, content, scope))
                }
                _ => None,
            };
            let kit_variant = match variant.as_deref() {
                Some("primary") => ButtonVariant::Primary,
                Some("borderless") => ButtonVariant::Ghost,
                _ => ButtonVariant::Secondary,
            };
            let msg = action.as_ref().map(|a| action_msg(ctx, id, a, scope));
            match label {
                Some(label) => {
                    let mut b = button(label).variant(kit_variant);
                    match msg {
                        Some(m) => b = b.on_click(m),
                        None => b = b.disabled(true),
                    }
                    b.into()
                }
                None => {
                    let inner = render_by_id(ctx, child, scope, depth + 1);
                    let mut b = icon_button(inner);
                    if let Some(m) = msg {
                        b = b.on_click(m);
                    }
                    b.into()
                }
            }
        }
        Kind::TextField {
            label,
            value,
            variant,
            ..
        } => {
            let label = resolve_value(ctx, id, label, scope);
            let (current, path) = input_state(ctx, id, value.as_ref(), scope);
            if variant.as_deref() == Some("obscured") {
                ctx.note(id, "obscured input renders unmasked (masking is a kit gap)");
            }
            let control: Element<A2uiMsg> = if variant.as_deref() == Some("longText") {
                let mut area = text_area(current);
                area = match path {
                    Some(path) => area.on_input(move |v| A2uiMsg::SetString {
                        path: path.clone(),
                        value: v,
                    }),
                    None => {
                        let id = id.to_owned();
                        area.on_input(move |v| A2uiMsg::LocalEdit {
                            id: id.clone(),
                            value: Value::String(v),
                        })
                    }
                };
                area.into()
            } else {
                let mut input = text_input(current);
                input = match path {
                    Some(path) => input.on_input(move |v| A2uiMsg::SetString {
                        path: path.clone(),
                        value: v,
                    }),
                    None => {
                        let id = id.to_owned();
                        input.on_input(move |v| A2uiMsg::LocalEdit {
                            id: id.clone(),
                            value: Value::String(v),
                        })
                    }
                };
                input.into()
            };
            field(label).child(control).into()
        }
        Kind::CheckBox { label, value, .. } => {
            let label = resolve_value(ctx, id, label, scope);
            let (checked, path) = match value {
                Dyn::Binding { path } => (
                    bound_bool(ctx, id, path, scope),
                    Some(absolute(path, scope)),
                ),
                other => {
                    // Literal-valued inputs stay interactive: the toggle
                    // stores a local edit, and the render reads it back.
                    let base = resolve_bool(ctx, id, other, scope);
                    let checked = ctx
                        .surface
                        .ui
                        .local_edits
                        .get(id)
                        .and_then(Value::as_bool)
                        .unwrap_or(base);
                    (checked, None)
                }
            };
            let mut cb = checkbox(checked).label(label);
            cb = match path {
                Some(path) => cb.on_toggle(A2uiMsg::SetBool {
                    path,
                    value: !checked,
                }),
                None => cb.on_toggle(A2uiMsg::LocalEdit {
                    id: id.to_owned(),
                    value: Value::Bool(!checked),
                }),
            };
            cb.into()
        }
        Kind::ChoicePicker {
            label,
            variant,
            options,
            value,
            ..
        } => render_choice_picker(
            ctx,
            id,
            label.as_ref(),
            variant.as_deref(),
            options,
            value,
            scope,
        ),
        Kind::Slider {
            label,
            min,
            max,
            value,
        } => {
            let min = min.unwrap_or(0.0);
            let (current, path) = match value {
                Dyn::Binding { path } => (
                    bound_f64(ctx, id, path, scope, min),
                    Some(absolute(path, scope)),
                ),
                other => {
                    // Literal-valued sliders stay interactive through
                    // local edits, like every other input control.
                    let base = resolve_f64(ctx, id, other, scope);
                    let current = ctx
                        .surface
                        .ui
                        .local_edits
                        .get(id)
                        .and_then(Value::as_f64)
                        .unwrap_or(base);
                    (current, None)
                }
            };
            #[expect(clippy::cast_possible_truncation, reason = "UI ranges fit in f32")]
            let mut s = slider(current as f32).range(min as f32, *max as f32);
            s = match path {
                Some(path) => s.on_change(move |v| A2uiMsg::SetNumber {
                    path: path.clone(),
                    value: f64::from(v),
                }),
                None => {
                    let id = id.to_owned();
                    s.on_change(move |v| A2uiMsg::LocalEdit {
                        id: id.clone(),
                        value: serde_json::json!(f64::from(v)),
                    })
                }
            };
            match label {
                Some(l) => field(resolve_value(ctx, id, l, scope)).child(s).into(),
                None => s.into(),
            }
        }
        Kind::DateTimeInput { value, label, .. } => {
            ctx.note(
                id,
                "DateTimeInput renders as an ISO text field (calendar UI TBD)",
            );
            let (current, path) = input_state(ctx, id, Some(value), scope);
            let mut input = text_input(current).placeholder("YYYY-MM-DD");
            if let Some(path) = path {
                input = input.on_input(move |v| A2uiMsg::SetString {
                    path: path.clone(),
                    value: v,
                });
            }
            match label {
                Some(l) => field(resolve_value(ctx, id, l, scope)).child(input).into(),
                None => input.into(),
            }
        }
        Kind::Unknown(raw) => {
            let name = raw
                .get("component")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            ctx.note(
                id,
                format!(
                    "component {name:?} did not map onto the basic catalog (unknown name or \
                     malformed fields); rendering a placeholder"
                ),
            );
            placeholder(format!("[{name}]"), theme)
        }
    }
}

/// Current value + absolute write path for a string-valued input,
/// consulting local edits for literal-valued ones.
fn input_state(
    ctx: &Ctx,
    id: &str,
    value: Option<&Dyn<String>>,
    scope: Option<&str>,
) -> (String, Option<String>) {
    match value {
        Some(Dyn::Binding { path }) => (
            lookup(ctx.surface, path, scope)
                .map(functions::display)
                .unwrap_or_default(),
            Some(absolute(path, scope)),
        ),
        Some(other) => {
            let base = resolve_value(ctx, id, other, scope);
            let current = ctx
                .surface
                .ui
                .local_edits
                .get(id)
                .map(functions::display)
                .unwrap_or(base);
            (current, None)
        }
        None => (
            ctx.surface
                .ui
                .local_edits
                .get(id)
                .map(functions::display)
                .unwrap_or_default(),
            None,
        ),
    }
}

fn action_msg(ctx: &Ctx, id: &str, action: &Action, scope: Option<&str>) -> A2uiMsg {
    match action {
        Action::Event { event } => {
            let context = event
                .context
                .as_ref()
                .map(|c| resolve_context(ctx, c, scope))
                .unwrap_or(Value::Null);
            A2uiMsg::Event {
                name: event.name.clone(),
                context,
                source_id: id.to_owned(),
            }
        }
        Action::FunctionCall { function_call } if function_call.call == "openUrl" => {
            let url = match function_call.args.get("url") {
                Some(Value::Object(o)) if o.contains_key("path") => {
                    let path = o["path"].as_str().unwrap_or_default();
                    lookup(ctx.surface, path, scope)
                        .map(functions::display)
                        .unwrap_or_default()
                }
                Some(v) => functions::display(v),
                None => String::new(),
            };
            A2uiMsg::OpenUrl(url)
        }
        Action::FunctionCall { function_call } => {
            ctx.note(
                id,
                format!(
                    "action function {:?} is not implemented",
                    function_call.call
                ),
            );
            A2uiMsg::Event {
                name: format!("unimplemented:{}", function_call.call),
                context: Value::Null,
                source_id: id.to_owned(),
            }
        }
    }
}

/// Resolves `{path}` bindings anywhere inside an action context object.
fn resolve_context(ctx: &Ctx, value: &Value, scope: Option<&str>) -> Value {
    match value {
        Value::Object(map) => {
            if map.len() == 1
                && let Some(Value::String(path)) = map.get("path")
            {
                return lookup(ctx.surface, path, scope)
                    .cloned()
                    .unwrap_or(Value::Null);
            }
            Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), resolve_context(ctx, v, scope)))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|v| resolve_context(ctx, v, scope))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn render_choice_picker(
    ctx: &Ctx,
    id: &str,
    label: Option<&Dyn<String>>,
    variant: Option<&str>,
    options: &[ChoiceOption],
    value: &Value,
    scope: Option<&str>,
) -> Element<A2uiMsg> {
    let labels: Vec<String> = options
        .iter()
        .map(|o| resolve_value(ctx, id, &o.label, scope))
        .collect();
    let values: Vec<String> = options.iter().map(|o| o.value.clone()).collect();
    /// A selection as a string list: an array of values, or one string (a
    /// valid mutually-exclusive selection) — accepted identically whether
    /// it arrives literal or through a binding.
    fn selection_of(v: &Value) -> Option<Vec<String>> {
        match v {
            Value::Array(items) => Some(items.iter().map(functions::display).collect()),
            Value::String(s) => Some(vec![s.clone()]),
            _ => None,
        }
    }
    let (mut selected_values, path): (Vec<String>, Option<String>) = match value {
        Value::Object(o) if o.contains_key("path") => {
            let p = o["path"].as_str().unwrap_or_default();
            let selected = match lookup(ctx.surface, p, scope) {
                Some(v) => selection_of(v).unwrap_or_else(|| {
                    ctx.note(
                        id,
                        format!("selection binding {p:?} is neither a list nor a string"),
                    );
                    Vec::new()
                }),
                None => Vec::new(),
            };
            (selected, Some(absolute(p, scope)))
        }
        other => (selection_of(other).unwrap_or_default(), None),
    };
    if path.is_none() {
        // Literal-valued pickers stay interactive through local edits.
        if let Some(edited) = ctx
            .surface
            .ui
            .local_edits
            .get(id)
            .and_then(selection_of)
        {
            selected_values = edited;
        }
    }
    let selected_idx: Vec<usize> = values
        .iter()
        .enumerate()
        .filter(|(_, v)| selected_values.contains(v))
        .map(|(i, _)| i)
        .collect();
    // Selection changes write through the binding, or store a local edit
    // for literal-valued pickers — either way the picker stays live.
    let make_msg = {
        let path = path.clone();
        let id = id.to_owned();
        move |values: Vec<String>| match &path {
            Some(p) => A2uiMsg::SetList {
                path: p.clone(),
                values,
            },
            None => A2uiMsg::LocalEdit {
                id: id.clone(),
                value: Value::Array(values.into_iter().map(Value::String).collect()),
            },
        }
    };
    let multiple = variant == Some("multipleSelection");
    let control: Element<A2uiMsg> = if multiple {
        let mut ms = multi_select(selected_idx.clone(), labels);
        {
            let values = values.clone();
            let current = selected_idx;
            ms = ms.on_toggle(move |i| {
                let mut next: Vec<usize> = current.clone();
                if let Some(pos) = next.iter().position(|&x| x == i) {
                    next.remove(pos);
                } else {
                    next.push(i);
                    next.sort_unstable();
                }
                make_msg(
                    next.iter()
                        .filter_map(|&x| values.get(x).cloned())
                        .collect(),
                )
            });
        }
        ms.into()
    } else {
        let mut sel = select(selected_idx.first().copied().unwrap_or(0), labels);
        {
            let values = values.clone();
            sel = sel.on_change(move |i| make_msg(values.get(i).cloned().into_iter().collect()));
        }
        sel.into()
    };
    match label {
        Some(l) => field(resolve_value(ctx, id, l, scope))
            .child(control)
            .into(),
        None => control,
    }
}
