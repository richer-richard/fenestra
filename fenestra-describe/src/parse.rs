//! Parse a [`Description`] into an `Element<Action>` — the same tree the builders
//! produce, ready for the identical render and verification pipeline.
//!
//! Handlers carry an [`Action`]: an unbound widget emits an inert author
//! [`Action::Intent`] (the original string), while a widget with `bind` emits a
//! framework-owned state change (toggle a bool, set text, set a number) that the
//! engine applies — no logic crosses the boundary. A bound widget also reads its
//! displayed value from the runtime `state`, so typing and toggling reflect.
//!
//! The parser never panics on hostile input (clamp over panic): a color it cannot
//! resolve, or an alignment word it does not know, degrades to a default and
//! records a path-pointed [`DescribeError`] instead of failing the whole screen.

use fenestra_core::{Element, Theme, Weight, col, div, divider, row, spacer, stack, text};
use fenestra_kit::{ButtonVariant, button, checkbox, radio, slider, switch, text_area, text_input};

use crate::color::resolve_color;
use crate::error::DescribeError;
use crate::format::{Container, Description, InputNode, Leaf, Node, SCHEMA_V1, Style, TextNode};
use crate::state::{Action, StateMap, bound_bool, bound_number, bound_text};

/// Parses `desc` into an element, or the accumulated problems, against the
/// description's own initial `state`. Strict: any problem makes this return `Err`.
///
/// # Errors
/// A non-empty [`Vec`] of path-pointed [`DescribeError`]s.
pub fn to_element(
    desc: &Description,
    theme: &Theme,
) -> Result<Element<Action>, Vec<DescribeError>> {
    to_element_with(desc, theme, &desc.state)
}

/// Like [`to_element`], but renders bound widgets from an explicit runtime
/// `state` (the engine passes the live state after each interaction).
///
/// # Errors
/// A non-empty [`Vec`] of path-pointed [`DescribeError`]s.
pub fn to_element_with(
    desc: &Description,
    theme: &Theme,
    state: &StateMap,
) -> Result<Element<Action>, Vec<DescribeError>> {
    let (el, errors) = to_element_lenient_with(desc, theme, state);
    if errors.is_empty() {
        Ok(el)
    } else {
        Err(errors)
    }
}

/// Best-effort parse against the description's own initial `state`, returning the
/// element alongside any problems (the clamp-over-panic contract made visible).
pub fn to_element_lenient(
    desc: &Description,
    theme: &Theme,
) -> (Element<Action>, Vec<DescribeError>) {
    to_element_lenient_with(desc, theme, &desc.state)
}

/// Best-effort parse against an explicit runtime `state`.
pub fn to_element_lenient_with(
    desc: &Description,
    theme: &Theme,
    state: &StateMap,
) -> (Element<Action>, Vec<DescribeError>) {
    let mut errors = Vec::new();
    if desc.schema != SCHEMA_V1 {
        errors.push(DescribeError::new(
            "schema",
            format!(
                "unsupported schema {:?}; expected {SCHEMA_V1:?}",
                desc.schema
            ),
        ));
    }
    let el = node_to_element(&desc.root, theme, state, "root", &mut errors);
    (el, errors)
}

/// Maps one node to an element, recursing into children. Always produces an
/// element; soft problems append to `errors`.
fn node_to_element(
    node: &Node,
    theme: &Theme,
    state: &StateMap,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    match node {
        Node::Row(c) => container(row(), c, theme, state, path, errors),
        Node::Col(c) => container(col(), c, theme, state, path, errors),
        Node::Div(c) => container(div(), c, theme, state, path, errors),
        Node::Stack(c) => container(stack(), c, theme, state, path, errors),
        Node::Text(t) => text_node(t, theme, path, errors),
        Node::Divider(l) => leaf(divider(), l, theme, path, errors),
        Node::Spacer(l) => leaf(spacer(), l, theme, path, errors),
        Node::Button(b) => {
            let mut w = button(b.label.clone());
            if let Some(name) = &b.variant {
                match button_variant(name) {
                    Ok(variant) => w = w.variant(variant),
                    Err(message) => {
                        errors.push(DescribeError::new(format!("{path}/variant"), message));
                    }
                }
            }
            if let Some(intent) = &b.on_click {
                w = w.on_click(Action::Intent(intent.clone()));
            }
            if b.disabled {
                w = w.disabled(true);
            }
            if let Some(id) = &b.id {
                w = w.id(id);
            }
            w.into()
        }
        Node::Checkbox(c) => {
            let checked = c
                .bind
                .as_ref()
                .map_or(c.checked, |k| bound_bool(state, k, c.checked));
            let mut w = checkbox(checked);
            if let Some(label) = &c.label {
                w = w.label(label.clone());
            }
            match &c.bind {
                Some(key) => w = w.on_toggle(Action::SetBool(key.clone(), !checked)),
                None => {
                    if let Some(intent) = &c.on_change {
                        w = w.on_toggle(Action::Intent(intent.clone()));
                    }
                }
            }
            if let Some(id) = &c.id {
                w = w.id(id);
            }
            w.into()
        }
        Node::Switch(s) => {
            let on = s.bind.as_ref().map_or(s.on, |k| bound_bool(state, k, s.on));
            let mut w = switch(on);
            if let Some(label) = &s.label {
                w = w.label(label.clone());
            }
            match &s.bind {
                Some(key) => w = w.on_toggle(Action::SetBool(key.clone(), !on)),
                None => {
                    if let Some(intent) = &s.on_change {
                        w = w.on_toggle(Action::Intent(intent.clone()));
                    }
                }
            }
            if let Some(id) = &s.id {
                w = w.id(id);
            }
            w.into()
        }
        Node::Radio(r) => {
            let mut w = radio(r.selected);
            if let Some(label) = &r.label {
                w = w.label(label.clone());
            }
            if let Some(intent) = &r.on_change {
                w = w.on_select(Action::Intent(intent.clone()));
            }
            if let Some(id) = &r.id {
                w = w.id(id);
            }
            w.into()
        }
        Node::Slider(s) => {
            let value = s
                .bind
                .as_ref()
                .map_or(s.value, |k| bound_number(state, k, s.value));
            let mut w = slider(value);
            if let Some(step) = s.step {
                w = w.step(step);
            }
            match &s.bind {
                Some(key) => {
                    let key = key.clone();
                    w = w.on_change(move |v| Action::SetNumber(key.clone(), v));
                }
                None => {
                    if let Some(intent) = &s.on_change {
                        let intent = intent.clone();
                        w = w.on_change(move |_| Action::Intent(intent.clone()));
                    }
                }
            }
            if let Some(id) = &s.id {
                w = w.id(id);
            }
            w.into()
        }
        Node::TextInput(i) => {
            let value = input_value(i, state);
            let mut w = text_input(value);
            if let Some(ph) = &i.placeholder {
                w = w.placeholder(ph.clone());
            }
            match &i.bind {
                Some(key) => {
                    let key = key.clone();
                    w = w.on_input(move |s| Action::SetText(key.clone(), s));
                }
                None => {
                    if let Some(intent) = &i.on_input {
                        let intent = intent.clone();
                        w = w.on_input(move |_| Action::Intent(intent.clone()));
                    }
                }
            }
            if let Some(id) = &i.id {
                w = w.id(id);
            }
            w.into()
        }
        Node::TextArea(i) => {
            let value = input_value(i, state);
            let mut w = text_area(value);
            if let Some(ph) = &i.placeholder {
                w = w.placeholder(ph.clone());
            }
            match &i.bind {
                Some(key) => {
                    let key = key.clone();
                    w = w.on_input(move |s| Action::SetText(key.clone(), s));
                }
                None => {
                    if let Some(intent) = &i.on_input {
                        let intent = intent.clone();
                        w = w.on_input(move |_| Action::Intent(intent.clone()));
                    }
                }
            }
            if let Some(id) = &i.id {
                w = w.id(id);
            }
            w.into()
        }
    }
}

/// The displayed value of an input: the bound state value, else the literal.
fn input_value(i: &InputNode, state: &StateMap) -> String {
    i.bind
        .as_ref()
        .map_or_else(|| i.value.clone(), |k| bound_text(state, k, &i.value))
}

/// Builds a container element: style, id, then recursively-mapped children.
fn container(
    base: Element<Action>,
    c: &Container,
    theme: &Theme,
    state: &StateMap,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let mut el = apply_style(base, &c.style, theme, path, errors);
    if let Some(id) = &c.id {
        el = el.id(id);
    }
    let children: Vec<Element<Action>> = c
        .children
        .iter()
        .enumerate()
        .map(|(i, child)| {
            node_to_element(child, theme, state, &format!("{path}/children/{i}"), errors)
        })
        .collect();
    el.children(children)
}

/// Builds a text element with its type styling.
fn text_node(
    t: &TextNode,
    theme: &Theme,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let mut el = apply_style(text(t.content.clone()), &t.style, theme, path, errors);
    if let Some(id) = &t.id {
        el = el.id(id);
    }
    el
}

/// Builds a childless decorative element (divider, spacer).
fn leaf(
    base: Element<Action>,
    l: &Leaf,
    theme: &Theme,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let mut el = apply_style(base, &l.style, theme, path, errors);
    if let Some(id) = &l.id {
        el = el.id(id);
    }
    el
}

/// Applies a style block to an element. Unresolvable colors and unknown
/// alignment words degrade to a default and record a path-pointed error.
fn apply_style(
    mut el: Element<Action>,
    style: &Style,
    theme: &Theme,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    if let Some(v) = style.p
        && finite_num(v, path, "p", errors)
    {
        el = el.p(v);
    }
    if let Some(v) = style.px
        && finite_num(v, path, "px", errors)
    {
        el = el.px(v);
    }
    if let Some(v) = style.py
        && finite_num(v, path, "py", errors)
    {
        el = el.py(v);
    }
    if let Some(v) = style.gap
        && finite_num(v, path, "gap", errors)
    {
        el = el.gap(v);
    }
    if let Some(v) = style.w
        && finite_num(v, path, "w", errors)
    {
        el = el.w(v);
    }
    if let Some(v) = style.h
        && finite_num(v, path, "h", errors)
    {
        el = el.h(v);
    }
    if let Some(v) = style.rounded
        && finite_num(v, path, "rounded", errors)
    {
        el = el.rounded(v);
    }
    if let Some(spec) = &style.bg {
        match resolve_color(spec, theme) {
            Ok(c) => el = el.bg(c),
            Err(e) => errors.push(relocate(e, format!("{path}/style/bg"))),
        }
    }
    if let Some(border) = &style.border {
        match resolve_color(&border.color, theme) {
            Ok(c) => {
                if finite_num(border.width, path, "border/width", errors) {
                    el = el.border(border.width, c);
                }
            }
            Err(e) => errors.push(relocate(e, format!("{path}/style/border/color"))),
        }
    }
    if let Some(align) = &style.align {
        el = match align.as_str() {
            "start" => el.items_start(),
            "center" => el.items_center(),
            "end" => el.items_end(),
            "baseline" => el.items_baseline(),
            other => {
                errors.push(DescribeError::new(
                    format!("{path}/style/align"),
                    format!("unknown align {other:?}; expected start|center|end|baseline"),
                ));
                el
            }
        };
    }
    if let Some(justify) = &style.justify {
        el = match justify.as_str() {
            "start" => el.justify_start(),
            "center" => el.justify_center(),
            "end" => el.justify_end(),
            "between" => el.justify_between(),
            other => {
                errors.push(DescribeError::new(
                    format!("{path}/style/justify"),
                    format!("unknown justify {other:?}; expected start|center|end|between"),
                ));
                el
            }
        };
    }
    if let Some(spec) = &style.color {
        match resolve_color(spec, theme) {
            Ok(c) => el = el.color(c),
            Err(e) => errors.push(relocate(e, format!("{path}/style/color"))),
        }
    }
    if let Some(v) = style.size_px {
        if v.is_finite() && v > 0.0 && v <= MAX_FONT_PX {
            el = el.size_px(v);
        } else {
            errors.push(DescribeError::new(
                format!("{path}/style/size_px"),
                format!("size_px must be a finite number in 0..={MAX_FONT_PX}; got {v}"),
            ));
        }
    }
    if let Some(w) = style.weight {
        el = el.weight(weight_from(w));
    }
    el
}

/// Largest font size, in logical pixels, a description may request. A larger or
/// non-finite size is an authoring error and would force the text layout into a
/// pathological slow path, so the boundary rejects it rather than render it.
const MAX_FONT_PX: f32 = 4096.0;

/// Accepts a finite style length; a non-finite (`NaN`/`±∞`) value is an authoring
/// error, so it records a path-pointed error and returns `false` (the caller then
/// skips applying it, degrading rather than rendering nonsense).
fn finite_num(v: f32, path: &str, field: &str, errors: &mut Vec<DescribeError>) -> bool {
    if v.is_finite() {
        true
    } else {
        errors.push(DescribeError::new(
            format!("{path}/style/{field}"),
            format!("{field} must be a finite number; got {v}"),
        ));
        false
    }
}

/// Re-points an error to a precise path.
fn relocate(mut e: DescribeError, path: String) -> DescribeError {
    e.path = path;
    e
}

/// Snaps a numeric OpenType weight to the nearest supported [`Weight`] step
/// (400 Regular, 500 Medium, 600 Semibold) — the only weights the kit ships.
fn weight_from(w: u16) -> Weight {
    if w <= 450 {
        Weight::Regular
    } else if w <= 550 {
        Weight::Medium
    } else {
        Weight::Semibold
    }
}

/// Maps a button-variant name to the kit variant.
fn button_variant(name: &str) -> Result<ButtonVariant, String> {
    Ok(match name {
        "primary" => ButtonVariant::Primary,
        "secondary" => ButtonVariant::Secondary,
        "ghost" => ButtonVariant::Ghost,
        "danger" => ButtonVariant::Danger,
        other => {
            return Err(format!(
                "unknown button variant {other:?}; expected primary|secondary|ghost|danger"
            ));
        }
    })
}

/// Validates a description's JSON without rendering. Structural problems
/// (unknown fields, bad variant tags, type mismatches) come back path-pointed
/// via `serde_path_to_error`; semantic problems (an unknown color role or
/// alignment word) are caught by a dry parse against the default theme.
///
/// # Errors
/// A non-empty [`Vec`] of path-pointed [`DescribeError`]s.
pub fn validate(json: &str) -> Result<(), Vec<DescribeError>> {
    let de = &mut serde_json::Deserializer::from_str(json);
    let desc: Description = match serde_path_to_error::deserialize(de) {
        Ok(desc) => desc,
        Err(e) => {
            return Err(vec![DescribeError::new(
                e.path().to_string(),
                e.inner().to_string(),
            )]);
        }
    };
    let (_, errors) = to_element_lenient(&desc, &Theme::light());
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
