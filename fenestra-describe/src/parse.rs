//! Parse a [`Description`] into an `Element<String>` — the same tree the
//! builders produce, ready for the identical render and verification pipeline.
//!
//! The parser never panics on hostile input (clamp over panic): a color role it
//! cannot resolve, or an alignment word it does not know, degrades to a sensible
//! default and records a path-pointed [`DescribeError`] instead of failing the
//! whole screen. Handlers become inert intent strings — the closure ignores the
//! live event value and returns the author's fixed intent.

use fenestra_core::{Color, Element, Theme, Weight, col, div, divider, row, spacer, stack, text};
use fenestra_kit::{button, checkbox, radio, slider, switch, text_area, text_input};

use crate::color::resolve_color;
use crate::error::DescribeError;
use crate::format::{Container, Description, Leaf, Node, SCHEMA_V1, Style, TextNode};

/// Parses `desc` into an element, or the accumulated problems. Strict: any
/// problem (a bad schema tag, an unresolvable color, an unknown alignment word)
/// makes this return `Err` so an agent fixes the description before rendering.
/// Use [`to_element_lenient`] to render the best-effort tree anyway.
///
/// # Errors
/// A non-empty [`Vec`] of path-pointed [`DescribeError`]s.
pub fn to_element(
    desc: &Description,
    theme: &Theme,
) -> Result<Element<String>, Vec<DescribeError>> {
    let (el, errors) = to_element_lenient(desc, theme);
    if errors.is_empty() {
        Ok(el)
    } else {
        Err(errors)
    }
}

/// Like [`to_element`], but always returns a best-effort element alongside any
/// problems, so a renderer can show the degraded UI with its warnings attached
/// rather than refusing. This is the clamp-over-panic contract made visible.
pub fn to_element_lenient(
    desc: &Description,
    theme: &Theme,
) -> (Element<String>, Vec<DescribeError>) {
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
    let el = node_to_element(&desc.root, theme, "root", &mut errors);
    (el, errors)
}

/// Maps one node to an element, recursing into children. Always produces an
/// element; soft problems append to `errors`.
fn node_to_element(
    node: &Node,
    theme: &Theme,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<String> {
    match node {
        Node::Row(c) => container(row(), c, theme, path, errors),
        Node::Col(c) => container(col(), c, theme, path, errors),
        Node::Div(c) => container(div(), c, theme, path, errors),
        Node::Stack(c) => container(stack(), c, theme, path, errors),
        Node::Text(t) => text_node(t, theme, path, errors),
        Node::Divider(l) => leaf(divider(), l, theme, path, errors),
        Node::Spacer(l) => leaf(spacer(), l, theme, path, errors),
        Node::Button(b) => {
            let mut w = button(b.label.clone());
            if let Some(intent) = &b.on_click {
                w = w.on_click(intent.clone());
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
            let mut w = checkbox(c.checked);
            if let Some(label) = &c.label {
                w = w.label(label.clone());
            }
            if let Some(intent) = &c.on_change {
                w = w.on_toggle(intent.clone());
            }
            if let Some(id) = &c.id {
                w = w.id(id);
            }
            w.into()
        }
        Node::Switch(s) => {
            let mut w = switch(s.on);
            if let Some(label) = &s.label {
                w = w.label(label.clone());
            }
            if let Some(intent) = &s.on_change {
                w = w.on_toggle(intent.clone());
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
                w = w.on_select(intent.clone());
            }
            if let Some(id) = &r.id {
                w = w.id(id);
            }
            w.into()
        }
        Node::Slider(s) => {
            let mut w = slider(s.value);
            if let Some(intent) = &s.on_change {
                let intent = intent.clone();
                w = w.on_change(move |_| intent.clone());
            }
            if let Some(id) = &s.id {
                w = w.id(id);
            }
            w.into()
        }
        Node::TextInput(i) => {
            let mut w = text_input(i.value.clone());
            if let Some(ph) = &i.placeholder {
                w = w.placeholder(ph.clone());
            }
            if let Some(intent) = &i.on_input {
                let intent = intent.clone();
                w = w.on_input(move |_| intent.clone());
            }
            if let Some(id) = &i.id {
                w = w.id(id);
            }
            w.into()
        }
        Node::TextArea(i) => {
            let mut w = text_area(i.value.clone());
            if let Some(ph) = &i.placeholder {
                w = w.placeholder(ph.clone());
            }
            if let Some(intent) = &i.on_input {
                let intent = intent.clone();
                w = w.on_input(move |_| intent.clone());
            }
            if let Some(id) = &i.id {
                w = w.id(id);
            }
            w.into()
        }
    }
}

/// Builds a container element: style, id, then recursively-mapped children.
fn container(
    base: Element<String>,
    c: &Container,
    theme: &Theme,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<String> {
    let mut el = apply_style(base, &c.style, theme, path, errors);
    if let Some(id) = &c.id {
        el = el.id(id);
    }
    let children: Vec<Element<String>> = c
        .children
        .iter()
        .enumerate()
        .map(|(i, child)| node_to_element(child, theme, &format!("{path}/children/{i}"), errors))
        .collect();
    el.children(children)
}

/// Builds a text element with its type styling.
fn text_node(
    t: &TextNode,
    theme: &Theme,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<String> {
    let mut el = apply_style(text(t.content.clone()), &t.style, theme, path, errors);
    if let Some(id) = &t.id {
        el = el.id(id);
    }
    el
}

/// Builds a childless decorative element (divider, spacer).
fn leaf(
    base: Element<String>,
    l: &Leaf,
    theme: &Theme,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<String> {
    let mut el = apply_style(base, &l.style, theme, path, errors);
    if let Some(id) = &l.id {
        el = el.id(id);
    }
    el
}

/// Applies a style block to an element. Unresolvable colors and unknown
/// alignment words degrade to a default and record a path-pointed error.
fn apply_style(
    mut el: Element<String>,
    style: &Style,
    theme: &Theme,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<String> {
    if let Some(v) = style.p {
        el = el.p(v);
    }
    if let Some(v) = style.px {
        el = el.px(v);
    }
    if let Some(v) = style.py {
        el = el.py(v);
    }
    if let Some(v) = style.gap {
        el = el.gap(v);
    }
    if let Some(v) = style.w {
        el = el.w(v);
    }
    if let Some(v) = style.h {
        el = el.h(v);
    }
    if let Some(v) = style.rounded {
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
            Ok(c) => el = el.border(border.width, c),
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
            Ok(c) => el = text_color(el, c),
            Err(e) => errors.push(relocate(e, format!("{path}/style/color"))),
        }
    }
    if let Some(v) = style.size_px {
        el = el.size_px(v);
    }
    if let Some(w) = style.weight {
        el = el.weight(weight_from(w));
    }
    el
}

/// Sets text color on an element.
fn text_color(el: Element<String>, color: Color) -> Element<String> {
    el.color(color)
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
