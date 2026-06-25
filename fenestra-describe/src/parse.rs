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

use fenestra_core::{
    AdaptiveTint, DrawerSide, Element, GridTemplate, Repeat, ShadowToken, Sheen, SpecularEdge,
    Surface, TextAlign, Theme, Track, TrackMax, TrackMin, Weight, col, div, divider,
    linear_gradient, row, spacer, stack, text,
};
use fenestra_kit::{
    ButtonVariant, Status as KitStatus, accordion, accordion_item, avatar, badge, breadcrumbs,
    button, callout, card, checkbox, crumb, drawer, kbd, kbd_raised, menubar, meter, modal,
    pagination, progress, progress_indeterminate, radio, segmented, select, skeleton,
    skeleton_circle, skeleton_text, slider, spin_button, spinner, stat_card, status as kit_status,
    stepper, switch, tabs, text_area, text_input, toolbar, tooltip,
};

use crate::color::resolve_color;
use crate::error::DescribeError;
use crate::format::{
    AccordionNode, AdaptiveSpec, AvatarNode, BadgeNode, BreadcrumbsNode, CalloutNode, Container,
    Description, DrawerNode, EdgeSpec, IconNode, InputNode, KbdNode, Leaf, MenubarNode, MeterNode,
    ModalNode, Node, PaginationNode, ProgressNode, RadioNode, RepeatCount, SCHEMA_V1,
    SegmentedNode, SelectNode, SheenSpec, SkeletonNode, SpinButtonNode, StatCardNode, StatusNode,
    StepperNode, Style, TabsNode, TextNode, ToolbarNode, TooltipNode, TrackSpec,
};
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
        // ── Layout containers ─────────────────────────────────────────────────
        Node::Row(c) => container(row(), c, theme, state, path, errors),
        Node::Col(c) => container(col(), c, theme, state, path, errors),
        Node::Div(c) => container(div(), c, theme, state, path, errors),
        Node::Stack(c) => container(stack(), c, theme, state, path, errors),
        Node::Card(c) => container(card(), c, theme, state, path, errors),
        // ── Text ──────────────────────────────────────────────────────────────
        Node::Text(t) => text_node(t, theme, path, errors),
        // ── Form controls ──────────────────────────────────────────────────────
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
            // `bind` takes priority: click toggles state[bind].
            if let Some(key) = &b.bind {
                let current = bound_bool(state, key, false);
                let key = key.clone();
                w = w.on_click(Action::SetBool(key, !current));
            } else if let Some(intent) = &b.on_click {
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
        Node::Radio(r) => radio_node(r, state),
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
            if i.invalid == Some(true) {
                w = w.invalid(true);
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
            if i.invalid == Some(true) {
                w = w.invalid(true);
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
        Node::Select(s) => select_node(s, state, path, errors),
        Node::SpinButton(s) => spin_button_node(s),
        // ── Navigation ────────────────────────────────────────────────────────
        Node::Tabs(t) => tabs_node(t, state),
        Node::Segmented(s) => segmented_node(s, state),
        Node::Breadcrumbs(b) => breadcrumbs_node(b),
        Node::Pagination(p) => pagination_node(p, state),
        Node::Stepper(s) => stepper_node(s, state),
        Node::Toolbar(t) => toolbar_node(t, theme, state, path, errors),
        Node::Menubar(m) => menubar_node(m),
        // ── Display / feedback ─────────────────────────────────────────────────
        Node::Badge(b) => badge_node(b, path, errors),
        Node::Callout(c) => callout_node(c, path, errors),
        Node::StatCard(s) => stat_card_node(s, path, errors),
        Node::Avatar(a) => avatar_node(a),
        Node::Status(s) => status_node(s, path, errors),
        Node::Kbd(k) => kbd_node(k),
        Node::Progress(p) => progress_node(p),
        Node::Meter(m) => meter_node(m, state),
        Node::Accordion(a) => accordion_node(a, theme, state, path, errors),
        Node::Spinner(l) => leaf(spinner(), l, theme, path, errors),
        Node::Skeleton(k) => skeleton_node(k),
        Node::Icon(i) => icon_node(i, path, errors),
        // ── Overlays ──────────────────────────────────────────────────────────
        Node::Modal(m) => modal_node(m, theme, state, path, errors),
        Node::Tooltip(t) => tooltip_node(t, theme, state, path, errors),
        Node::Drawer(d) => drawer_node(d, theme, state, path, errors),
        // ── Decoration ────────────────────────────────────────────────────────
        Node::Divider(l) => leaf(divider(), l, theme, path, errors),
        Node::Spacer(l) => leaf(spacer(), l, theme, path, errors),
    }
}

// ── Form control helpers ──────────────────────────────────────────────────────

/// The displayed value of an input: the bound state value, else the literal.
fn input_value(i: &InputNode, state: &StateMap) -> String {
    i.bind
        .as_ref()
        .map_or_else(|| i.value.clone(), |k| bound_text(state, k, &i.value))
}

fn radio_node(r: &RadioNode, state: &StateMap) -> Element<Action> {
    // When group + value are set, derive selected from state[group] == value.
    let selected = if let (Some(group), Some(value)) = (&r.group, &r.value) {
        state
            .get(group)
            .and_then(|v| v.as_str())
            .map_or(r.selected, |s| s == value.as_str())
    } else {
        r.selected
    };
    let mut w = radio(selected);
    if let Some(label) = &r.label {
        w = w.label(label.clone());
    }
    // When group binding: clicking emits SetText(group, value).
    if let (Some(group), Some(value)) = (&r.group, &r.value) {
        w = w.on_select(Action::SetText(group.clone(), value.clone()));
    } else if let Some(intent) = &r.on_change {
        w = w.on_select(Action::Intent(intent.clone()));
    }
    if let Some(id) = &r.id {
        w = w.id(id);
    }
    w.into()
}

fn select_node(
    s: &SelectNode,
    state: &StateMap,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    if s.options.is_empty() {
        errors.push(DescribeError::new(
            format!("{path}/options"),
            "select requires at least one option".to_string(),
        ));
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "option index fits in usize on any platform fenestra supports"
    )]
    let active = if let Some(key) = &s.bind {
        bound_number(state, key, s.selected as f32).max(0.0) as usize
    } else {
        s.selected
    };
    let mut w = select(active, s.options.clone());
    match &s.bind {
        Some(key) => {
            let key = key.clone();
            w = w.on_change(move |i| Action::SetNumber(key.clone(), i as f32));
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

// ── Navigation helpers ────────────────────────────────────────────────────────

/// The selection handler shared by the index-based navigation widgets: a bound
/// key projects a framework-owned number change, an unbound `on_change` fires an
/// inert author intent, and a bare widget emits an empty intent.
fn index_handler(
    bind: &Option<String>,
    on_change: &Option<String>,
) -> Box<dyn Fn(usize) -> Action> {
    if let Some(key) = bind {
        let key = key.clone();
        Box::new(move |i| Action::SetNumber(key.clone(), i as f32))
    } else if let Some(intent) = on_change {
        let intent = intent.clone();
        Box::new(move |_| Action::Intent(intent.clone()))
    } else {
        Box::new(|_| Action::Intent(String::new()))
    }
}

/// Reads a bound 0-based index from state (clamped non-negative), else `default`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "selection indices fit in usize on any platform fenestra supports"
)]
fn bound_index(state: &StateMap, bind: &Option<String>, default: usize) -> usize {
    match bind {
        Some(key) => bound_number(state, key, default as f32).max(0.0) as usize,
        None => default,
    }
}

fn tabs_node(t: &TabsNode, state: &StateMap) -> Element<Action> {
    let active = bound_index(state, &t.bind, t.active);
    tabs(
        active,
        t.labels.clone(),
        index_handler(&t.bind, &t.on_change),
    )
}

fn segmented_node(s: &SegmentedNode, state: &StateMap) -> Element<Action> {
    let active = bound_index(state, &s.bind, s.active);
    let w = segmented(
        active,
        s.labels.clone(),
        index_handler(&s.bind, &s.on_change),
    )
    .disabled(s.disabled);
    let el: Element<Action> = w.into();
    if let Some(id) = &s.id { el.id(id) } else { el }
}

fn breadcrumbs_node(b: &BreadcrumbsNode) -> Element<Action> {
    let handler = index_handler(&b.bind, &b.on_change);
    let n = b.items.len();
    let mut w = breadcrumbs(b.items.iter().enumerate().map(|(i, label)| {
        let c = crumb(label.clone());
        // The last crumb is the current page (non-link); earlier ones navigate.
        if i + 1 < n {
            c.on_select(handler(i))
        } else {
            c
        }
    }));
    if let Some(max) = b.max_items {
        w = w.max_items(max);
    }
    let el: Element<Action> = w.into();
    if let Some(id) = &b.id { el.id(id) } else { el }
}

fn pagination_node(p: &PaginationNode, state: &StateMap) -> Element<Action> {
    let page = bound_index(state, &p.bind, p.page);
    let mut w = pagination(page, p.count);
    if let Some(s) = p.siblings {
        w = w.siblings(s);
    }
    let el: Element<Action> = w.on_select(index_handler(&p.bind, &p.on_change)).into();
    if let Some(id) = &p.id { el.id(id) } else { el }
}

fn stepper_node(s: &StepperNode, state: &StateMap) -> Element<Action> {
    let current = bound_index(state, &s.bind, s.current);
    let mut w = stepper(current);
    for (i, title) in s.steps.iter().enumerate() {
        match s.descriptions.get(i) {
            Some(desc) if !desc.is_empty() => w = w.step_with(title.clone(), desc.clone()),
            _ => w = w.step(title.clone()),
        }
    }
    let el: Element<Action> = w.on_select(index_handler(&s.bind, &s.on_change)).into();
    if let Some(id) = &s.id { el.id(id) } else { el }
}

fn toolbar_node(
    t: &ToolbarNode,
    theme: &Theme,
    state: &StateMap,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let mut w = toolbar();
    if t.vertical {
        w = w.vertical();
    }
    if let Some(label) = &t.label {
        w = w.label(label.clone());
    }
    for (i, child) in t.children.iter().enumerate() {
        let c = node_to_element(child, theme, state, &format!("{path}/children/{i}"), errors);
        w = w.item(c);
    }
    let el: Element<Action> = w.into();
    if let Some(id) = &t.id { el.id(id) } else { el }
}

fn menubar_node(m: &MenubarNode) -> Element<Action> {
    let mut w = menubar();
    for menu in &m.menus {
        let items: Vec<(String, Action)> = menu
            .items
            .iter()
            .map(|it| {
                let action = it.on_select.as_ref().map_or_else(
                    || Action::Intent(String::new()),
                    |s| Action::Intent(s.clone()),
                );
                (it.label.clone(), action)
            })
            .collect();
        w = w.menu(menu.title.clone(), items);
    }
    w.into()
}

fn drawer_node(
    d: &DrawerNode,
    theme: &Theme,
    state: &StateMap,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let side = match d.side.as_str() {
        "right" => DrawerSide::Right,
        "top" => DrawerSide::Top,
        "bottom" => DrawerSide::Bottom,
        "left" => DrawerSide::Left,
        other => {
            errors.push(DescribeError::new(
                format!("{path}/side"),
                format!("unknown drawer side {other:?}; expected left|right|top|bottom"),
            ));
            DrawerSide::Left
        }
    };
    let mut w = drawer(side);
    if let Some(title) = &d.title {
        w = w.title(title.clone());
    }
    if let Some(size) = d.size {
        w = w.size(size);
    }
    if let Some(intent) = &d.on_close {
        w = w.on_close(Action::Intent(intent.clone()));
    }
    for (i, child) in d.children.iter().enumerate() {
        let c = node_to_element(child, theme, state, &format!("{path}/children/{i}"), errors);
        w = w.child(c);
    }
    let el: Element<Action> = w.into();
    if let Some(id) = &d.id { el.id(id) } else { el }
}

fn spin_button_node(s: &SpinButtonNode) -> Element<Action> {
    let mut w = spin_button(s.value.clone());
    if let Some(label) = &s.label {
        w = w.label(label.clone());
    }
    if let Some(intent) = &s.on_decrement {
        w = w.on_decrement(Action::Intent(intent.clone()));
    }
    if let Some(intent) = &s.on_increment {
        w = w.on_increment(Action::Intent(intent.clone()));
    }
    let w = w
        .can_decrement(s.can_decrement)
        .can_increment(s.can_increment);
    let el: Element<Action> = w.into();
    if let Some(id) = &s.id { el.id(id) } else { el }
}

// ── Display / feedback helpers ────────────────────────────────────────────────

fn meter_node(m: &MeterNode, state: &StateMap) -> Element<Action> {
    let value = match &m.bind {
        Some(key) => bound_number(state, key, m.value),
        None => m.value,
    };
    let mut w = meter(value, m.min, m.max);
    if let Some(low) = m.low {
        w = w.low(low);
    }
    if let Some(high) = m.high {
        w = w.high(high);
    }
    if let Some(opt) = m.optimum {
        w = w.optimum(opt);
    }
    if let Some(label) = &m.label {
        w = w.label(label.clone());
    }
    let el: Element<Action> = w.into();
    if let Some(id) = &m.id { el.id(id) } else { el }
}

fn accordion_node(
    a: &AccordionNode,
    theme: &Theme,
    state: &StateMap,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let open = match &a.bind {
        Some(_) => Some(bound_index(state, &a.bind, a.open.unwrap_or(0))),
        None => a.open,
    };
    let handler = index_handler(&a.bind, &a.on_change);
    let mut built = Vec::with_capacity(a.items.len());
    for (i, item) in a.items.iter().enumerate() {
        let body = node_to_element(
            item.body.as_ref(),
            theme,
            state,
            &format!("{path}/items/{i}/body"),
            errors,
        );
        let mut it = accordion_item(item.title.clone(), body);
        if Some(i) == open {
            it = it.open(true);
        }
        built.push(it.on_toggle(handler(i)));
    }
    let el: Element<Action> = accordion(built).into();
    if let Some(id) = &a.id { el.id(id) } else { el }
}

fn badge_node(b: &BadgeNode, path: &str, errors: &mut Vec<DescribeError>) -> Element<Action> {
    let status = kit_status_from_str(&b.status, path, "status", errors);
    let mut el: Element<Action> = badge(b.label.clone(), status);
    if let Some(id) = &b.id {
        el = el.id(id);
    }
    el
}

fn callout_node(c: &CalloutNode, path: &str, errors: &mut Vec<DescribeError>) -> Element<Action> {
    let status = kit_status_from_str(&c.status, path, "status", errors);
    let mut el: Element<Action> = callout(status, c.message.clone());
    if let Some(id) = &c.id {
        el = el.id(id);
    }
    el
}

fn stat_card_node(
    s: &StatCardNode,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let mut w = stat_card(s.label.clone(), s.value.clone());
    if let Some(delta) = &s.delta {
        let delta_status = kit_status_from_str(&s.delta_status, path, "delta_status", errors);
        w = w.delta(delta.clone(), delta_status);
    }
    let el: Element<Action> = w.into();
    if let Some(id) = &s.id { el.id(id) } else { el }
}

fn avatar_node(a: &AvatarNode) -> Element<Action> {
    let el: Element<Action> = avatar(a.initials.clone());
    if let Some(id) = &a.id { el.id(id) } else { el }
}

fn status_node(s: &StatusNode, path: &str, errors: &mut Vec<DescribeError>) -> Element<Action> {
    let kstatus = kit_status_from_str(&s.status, path, "status", errors);
    let mut el: Element<Action> = kit_status(s.label.clone(), kstatus).live(s.live).into();
    if let Some(id) = &s.id {
        el = el.id(id);
    }
    el
}

fn kbd_node(k: &KbdNode) -> Element<Action> {
    let el = if k.raised {
        kbd_raised(k.keys.clone())
    } else {
        kbd(k.keys.clone())
    };
    if let Some(id) = &k.id { el.id(id) } else { el }
}

fn progress_node(p: &ProgressNode) -> Element<Action> {
    let el = if p.indeterminate {
        progress_indeterminate()
    } else {
        progress(p.value.clamp(0.0, 1.0))
    };
    if let Some(id) = &p.id { el.id(id) } else { el }
}

fn skeleton_node(k: &SkeletonNode) -> Element<Action> {
    let el = match k.kind.as_deref().unwrap_or("rect") {
        "text" => skeleton_text(k.lines.unwrap_or(3)),
        "circle" => skeleton_circle(k.w.unwrap_or(32.0).max(1.0)),
        _ => skeleton(k.w.unwrap_or(120.0).max(1.0), k.h.unwrap_or(16.0).max(1.0)),
    };
    if let Some(id) = &k.id { el.id(id) } else { el }
}

fn icon_node(i: &IconNode, path: &str, errors: &mut Vec<DescribeError>) -> Element<Action> {
    match named_icon(&i.name) {
        Some(el) => {
            if let Some(id) = &i.id {
                el.id(id)
            } else {
                el
            }
        }
        None => {
            errors.push(DescribeError::new(
                format!("{path}/name"),
                format!(
                    "unknown icon {:?}; known names: {}",
                    i.name,
                    ICON_NAMES.join(", ")
                ),
            ));
            spacer()
        }
    }
}

// ── Overlay helpers ───────────────────────────────────────────────────────────

fn modal_node(
    m: &ModalNode,
    theme: &Theme,
    state: &StateMap,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let children: Vec<Element<Action>> = m
        .children
        .iter()
        .enumerate()
        .map(|(i, child)| {
            node_to_element(child, theme, state, &format!("{path}/children/{i}"), errors)
        })
        .collect();
    // Wrap all children into a single col for the modal body.
    let body = col().children(children);
    let mut w = modal(m.title.clone()).child(body);
    if let Some(on_close) = &m.on_close {
        w = w.on_close(Action::Intent(on_close.clone()));
    }
    if let Some(mw) = m.max_width
        && mw.is_finite()
        && mw > 0.0
    {
        w = w.max_width(mw);
    } else if let Some(mw) = m.max_width {
        errors.push(DescribeError::new(
            format!("{path}/max_width"),
            format!("max_width must be a finite positive number; got {mw}"),
        ));
    }
    let mut el: Element<Action> = w.into();
    if let Some(id) = &m.id {
        el = el.id(id);
    }
    el
}

fn tooltip_node(
    t: &TooltipNode,
    theme: &Theme,
    state: &StateMap,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let target_el = node_to_element(&t.target, theme, state, &format!("{path}/target"), errors);
    let el: Element<Action> = tooltip(target_el, t.label.clone());
    if let Some(id) = &t.id { el.id(id) } else { el }
}

// ── Icon registry ─────────────────────────────────────────────────────────────

/// The kebab-case names of every supported Lucide icon, for error messages.
const ICON_NAMES: &[&str] = &[
    "alert-triangle",
    "arrow-left",
    "arrow-right",
    "bell",
    "calendar",
    "check",
    "chevron-down",
    "chevron-left",
    "chevron-right",
    "chevron-up",
    "clock",
    "copy",
    "download",
    "external-link",
    "eye",
    "file",
    "folder",
    "home",
    "info",
    "link",
    "lock",
    "log-out",
    "mail",
    "menu",
    "minus",
    "moon",
    "pencil",
    "plus",
    "refresh-cw",
    "save",
    "search",
    "settings",
    "star",
    "sun",
    "trash",
    "upload",
    "user",
    "x",
];

/// Maps a kebab-case icon name to a Lucide element, or `None` for unknown names.
fn named_icon(name: &str) -> Option<Element<Action>> {
    use fenestra_kit::icons::lucide;
    Some(match name {
        "alert-triangle" => lucide::alert_triangle(),
        "arrow-left" => lucide::arrow_left(),
        "arrow-right" => lucide::arrow_right(),
        "bell" => lucide::bell(),
        "calendar" => lucide::calendar(),
        "check" => lucide::check(),
        "chevron-down" => lucide::chevron_down(),
        "chevron-left" => lucide::chevron_left(),
        "chevron-right" => lucide::chevron_right(),
        "chevron-up" => lucide::chevron_up(),
        "clock" => lucide::clock(),
        "copy" => lucide::copy(),
        "download" => lucide::download(),
        "external-link" => lucide::external_link(),
        "eye" => lucide::eye(),
        "file" => lucide::file(),
        "folder" => lucide::folder(),
        "home" => lucide::home(),
        "info" => lucide::info(),
        "link" => lucide::link(),
        "lock" => lucide::lock(),
        "log-out" => lucide::log_out(),
        "mail" => lucide::mail(),
        "menu" => lucide::menu(),
        "minus" => lucide::minus(),
        "moon" => lucide::moon(),
        "pencil" => lucide::pencil(),
        "plus" => lucide::plus(),
        "refresh-cw" => lucide::refresh_cw(),
        "save" => lucide::save(),
        "search" => lucide::search(),
        "settings" => lucide::settings(),
        "star" => lucide::star(),
        "sun" => lucide::sun(),
        "trash" => lucide::trash(),
        "upload" => lucide::upload(),
        "user" => lucide::user(),
        "x" => lucide::x(),
        _ => return None,
    })
}

// ── Status helper ─────────────────────────────────────────────────────────────

/// Maps a status string to a [`KitStatus`]. Unknown values degrade to
/// `KitStatus::Accent` and record a path-pointed error.
fn kit_status_from_str(
    s: &str,
    path: &str,
    field: &str,
    errors: &mut Vec<DescribeError>,
) -> KitStatus {
    match s {
        "accent" => KitStatus::Accent,
        "danger" => KitStatus::Danger,
        "warning" => KitStatus::Warning,
        "success" => KitStatus::Success,
        other => {
            errors.push(DescribeError::new(
                format!("{path}/{field}"),
                format!("unknown status {other:?}; expected accent|danger|warning|success"),
            ));
            KitStatus::Accent
        }
    }
}

// ── Shadow helper ─────────────────────────────────────────────────────────────

/// Maps a shadow token name to a [`ShadowToken`].
fn shadow_token(name: &str) -> Result<ShadowToken, String> {
    match name {
        "xs" => Ok(ShadowToken::Xs),
        "sm" => Ok(ShadowToken::Sm),
        "md" => Ok(ShadowToken::Md),
        "lg" => Ok(ShadowToken::Lg),
        "xl" => Ok(ShadowToken::Xl),
        other => Err(format!(
            "unknown shadow token {other:?}; expected xs|sm|md|lg|xl"
        )),
    }
}

// ── Surface / glass helpers ─────────────────────────────────────────────────────

/// Maps a surface role name to a [`Surface`].
fn surface_role(name: &str) -> Result<Surface, String> {
    match name {
        "card" => Ok(Surface::Card),
        "raised" => Ok(Surface::Raised),
        "popover" => Ok(Surface::Popover),
        "menu" => Ok(Surface::Menu),
        "modal" => Ok(Surface::Modal),
        "glass" => Ok(Surface::Glass),
        "tooltip" => Ok(Surface::Tooltip),
        "thumb" => Ok(Surface::Thumb),
        other => Err(format!(
            "unknown surface role {other:?}; expected card|raised|popover|menu|modal|glass|tooltip|thumb"
        )),
    }
}

/// Resolves an [`EdgeSpec`] (the `"glass"` preset or explicit levers) to a
/// [`SpecularEdge`].
fn resolve_edge(spec: &EdgeSpec) -> Result<SpecularEdge, String> {
    match spec {
        EdgeSpec::Preset(name) => match name.as_str() {
            "glass" => Ok(SpecularEdge::glass()),
            other => Err(format!(
                "unknown specular_edge preset {other:?}; expected \"glass\" or a {{light_deg,intensity,shade}} object"
            )),
        },
        EdgeSpec::Custom {
            light_deg,
            intensity,
            shade,
        } => Ok(SpecularEdge {
            light_deg: *light_deg,
            intensity: *intensity,
            shade: *shade,
        }),
    }
}

/// Resolves a [`SheenSpec`] (the `"glass"` preset or explicit levers) to a
/// [`Sheen`].
fn resolve_sheen(spec: &SheenSpec) -> Result<Sheen, String> {
    match spec {
        SheenSpec::Preset(name) => match name.as_str() {
            "glass" => Ok(Sheen::glass()),
            other => Err(format!(
                "unknown sheen preset {other:?}; expected \"glass\" or a {{light_deg,top,bottom}} object"
            )),
        },
        SheenSpec::Custom {
            light_deg,
            top,
            bottom,
        } => Ok(Sheen {
            light_deg: *light_deg,
            top: *top,
            bottom: *bottom,
        }),
    }
}

/// Resolves an [`AdaptiveSpec`] (the `"glass"` preset or explicit levers) to an
/// [`AdaptiveTint`].
fn resolve_adaptive(spec: &AdaptiveSpec) -> Result<AdaptiveTint, String> {
    match spec {
        AdaptiveSpec::Preset(name) => match name.as_str() {
            "glass" => Ok(AdaptiveTint::glass()),
            other => Err(format!(
                "unknown adaptive_tint preset {other:?}; expected \"glass\" or a {{pivot,gain}} object"
            )),
        },
        AdaptiveSpec::Custom { pivot, gain } => Ok(AdaptiveTint {
            pivot: *pivot,
            gain: *gain,
        }),
    }
}

// ── Container / text / leaf builders ─────────────────────────────────────────

/// Builds a container element: style, on_click, id, then recursively-mapped children.
fn container(
    base: Element<Action>,
    c: &Container,
    theme: &Theme,
    state: &StateMap,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let mut el = apply_style(base, &c.style, theme, path, errors);
    if let Some(intent) = &c.on_click {
        el = el.on_click(Action::Intent(intent.clone()));
    }
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

/// Builds a text element with its type styling and optional on_click.
fn text_node(
    t: &TextNode,
    theme: &Theme,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let mut el = apply_style(text(t.content.clone()), &t.style, theme, path, errors);
    if let Some(intent) = &t.on_click {
        el = el.on_click(Action::Intent(intent.clone()));
    }
    if let Some(id) = &t.id {
        el = el.id(id);
    }
    el
}

/// Builds a childless decorative element (divider, spacer, spinner).
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

// ── Style application ─────────────────────────────────────────────────────────

/// Applies a style block to an element. Unresolvable colors, unknown alignment
/// words, and non-finite numeric props degrade to defaults and record errors.
fn apply_style(
    mut el: Element<Action>,
    style: &Style,
    theme: &Theme,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    // ── Surface material role: a complete role (fill/border/radius/shadow + the
    //    glass optics) resolved at theme time; it owns the paint it sets. ───────
    if let Some(name) = &style.surface {
        match surface_role(name) {
            Ok(role) => el = el.surface(role),
            Err(msg) => {
                errors.push(DescribeError::new(format!("{path}/style/surface"), msg));
            }
        }
    }
    // ── Padding (all → axes → per-side) ──────────────────────────────────────
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
    if let Some(v) = style.pt
        && finite_num(v, path, "pt", errors)
    {
        el = el.pt(v);
    }
    if let Some(v) = style.pb
        && finite_num(v, path, "pb", errors)
    {
        el = el.pb(v);
    }
    if let Some(v) = style.pl
        && finite_num(v, path, "pl", errors)
    {
        el = el.pl(v);
    }
    if let Some(v) = style.pr
        && finite_num(v, path, "pr", errors)
    {
        el = el.pr(v);
    }
    // ── Margin (all → axes → per-side) ───────────────────────────────────────
    if let Some(v) = style.m
        && finite_num(v, path, "m", errors)
    {
        el = el.m(v);
    }
    if let Some(v) = style.mx
        && finite_num(v, path, "mx", errors)
    {
        el = el.mx(v);
    }
    if let Some(v) = style.my
        && finite_num(v, path, "my", errors)
    {
        el = el.my(v);
    }
    if let Some(v) = style.mt
        && finite_num(v, path, "mt", errors)
    {
        el = el.mt(v);
    }
    if let Some(v) = style.mb
        && finite_num(v, path, "mb", errors)
    {
        el = el.mb(v);
    }
    if let Some(v) = style.ml
        && finite_num(v, path, "ml", errors)
    {
        el = el.ml(v);
    }
    if let Some(v) = style.mr
        && finite_num(v, path, "mr", errors)
    {
        el = el.mr(v);
    }
    // ── Gap / dimensions ──────────────────────────────────────────────────────
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
    if let Some(v) = style.min_w
        && finite_num(v, path, "min_w", errors)
    {
        el = el.min_w(v);
    }
    if let Some(v) = style.max_w
        && finite_num(v, path, "max_w", errors)
    {
        el = el.max_w(v);
    }
    if let Some(v) = style.min_h
        && finite_num(v, path, "min_h", errors)
    {
        el = el.min_h(v);
    }
    if let Some(v) = style.max_h
        && finite_num(v, path, "max_h", errors)
    {
        el = el.max_h(v);
    }
    // ── Grid templates ────────────────────────────────────────────────────────
    if let Some(specs) = &style.grid_cols {
        el = el.grid_cols(track_list(specs, path, "grid_cols", errors));
    }
    if let Some(specs) = &style.grid_rows {
        el = el.grid_rows(track_list(specs, path, "grid_rows", errors));
    }
    // ── Named grid lines & areas ──────────────────────────────────────────────
    if let Some(rows) = &style.grid_template_areas {
        el = el.grid_template_areas(rows.iter());
    }
    if let Some(name) = &style.grid_area {
        el = el.grid_area(name.clone());
    }
    if let Some([start, end]) = &style.grid_col_lines {
        el = el.grid_col_lines(start.clone(), end.clone());
    }
    if let Some([start, end]) = &style.grid_row_lines {
        el = el.grid_row_lines(start.clone(), end.clone());
    }
    if let Some(names) = &style.grid_col_names {
        el = el.grid_col_names(names.iter().cloned());
    }
    if let Some(names) = &style.grid_row_names {
        el = el.grid_row_names(names.iter().cloned());
    }
    // ── Corner radius ─────────────────────────────────────────────────────────
    if let Some(v) = style.rounded
        && finite_num(v, path, "rounded", errors)
    {
        el = el.rounded(v);
    }
    // ── Glass optics (squircle, backdrop blur, specular rim, sheen, adaptive) ──
    if let Some(v) = style.corner_smoothing
        && finite_num(v, path, "corner_smoothing", errors)
    {
        el = el.corner_smoothing(v);
    }
    if let Some(v) = style.backdrop_blur
        && finite_num(v, path, "backdrop_blur", errors)
    {
        el = el.backdrop_blur(v);
    }
    if let Some(spec) = &style.specular_edge {
        match resolve_edge(spec) {
            Ok(edge) => el = el.specular_edge(edge),
            Err(msg) => {
                errors.push(DescribeError::new(
                    format!("{path}/style/specular_edge"),
                    msg,
                ));
            }
        }
    }
    if let Some(spec) = &style.sheen {
        match resolve_sheen(spec) {
            Ok(sheen) => el = el.sheen(sheen),
            Err(msg) => errors.push(DescribeError::new(format!("{path}/style/sheen"), msg)),
        }
    }
    if let Some(spec) = &style.adaptive_tint {
        match resolve_adaptive(spec) {
            Ok(adaptive) => el = el.adaptive_tint(adaptive),
            Err(msg) => {
                errors.push(DescribeError::new(
                    format!("{path}/style/adaptive_tint"),
                    msg,
                ));
            }
        }
    }
    // ── Background (solid, then gradient overrides if both set) ───────────────
    if let Some(spec) = &style.bg {
        match resolve_color(spec, theme) {
            Ok(c) => el = el.bg(c),
            Err(e) => errors.push(relocate(e, format!("{path}/style/bg"))),
        }
    }
    if let Some(grad) = &style.gradient {
        if grad.stops.len() < 2 {
            errors.push(DescribeError::new(
                format!("{path}/style/gradient"),
                format!(
                    "gradient requires at least 2 stops; got {}",
                    grad.stops.len()
                ),
            ));
        } else {
            let mut colors = Vec::with_capacity(grad.stops.len());
            let mut ok = true;
            for (i, stop) in grad.stops.iter().enumerate() {
                match resolve_color(stop, theme) {
                    Ok(c) => colors.push(c),
                    Err(e) => {
                        errors.push(relocate(e, format!("{path}/style/gradient/stops/{i}")));
                        ok = false;
                    }
                }
            }
            if ok {
                el = el.bg(linear_gradient(grad.angle, colors));
            }
        }
    }
    // ── Border ────────────────────────────────────────────────────────────────
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
    // ── Shadow ────────────────────────────────────────────────────────────────
    if let Some(shadow_name) = &style.shadow {
        match shadow_token(shadow_name) {
            Ok(token) => el = el.shadow(token),
            Err(msg) => errors.push(DescribeError::new(format!("{path}/style/shadow"), msg)),
        }
    }
    // ── Alignment ────────────────────────────────────────────────────────────
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
    // ── Typography ────────────────────────────────────────────────────────────
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
    if let Some(align) = &style.text_align {
        el = match align.as_str() {
            "start" => el.text_align(TextAlign::Start),
            "center" => el.text_align(TextAlign::Center),
            "end" => el.text_align(TextAlign::End),
            other => {
                errors.push(DescribeError::new(
                    format!("{path}/style/text_align"),
                    format!("unknown text_align {other:?}; expected start|center|end"),
                ));
                el
            }
        };
    }
    // ── Opacity ───────────────────────────────────────────────────────────────
    if let Some(v) = style.opacity {
        if v.is_finite() && (0.0..=1.0).contains(&v) {
            el = el.opacity(v);
        } else {
            errors.push(DescribeError::new(
                format!("{path}/style/opacity"),
                format!("opacity must be a finite number in 0..=1; got {v}"),
            ));
        }
    }
    // ── Absolute positioning ──────────────────────────────────────────────────
    if style.absolute.unwrap_or(false) {
        el = el.absolute();
    }
    if let Some(v) = style.left
        && finite_num(v, path, "left", errors)
    {
        el = el.left(v);
    }
    if let Some(v) = style.top
        && finite_num(v, path, "top", errors)
    {
        el = el.top(v);
    }
    if let Some(v) = style.right
        && finite_num(v, path, "right", errors)
    {
        el = el.right(v);
    }
    if let Some(v) = style.bottom
        && finite_num(v, path, "bottom", errors)
    {
        el = el.bottom(v);
    }
    el
}

/// Largest font size, in logical pixels, a description may request. A larger or
/// non-finite size is an authoring error.
const MAX_FONT_PX: f32 = 4096.0;

/// Accepts a finite style length; a non-finite (`NaN`/`±∞`) value is an authoring
/// error, so it records a path-pointed error and returns `false` (the caller
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

/// Maps an array of [`TrackSpec`]s to grid template entries, path-pointing any
/// problem (a bad track degrades to `1fr` rather than failing the whole tree).
fn track_list(
    specs: &[TrackSpec],
    path: &str,
    field: &str,
    errors: &mut Vec<DescribeError>,
) -> Vec<GridTemplate> {
    specs
        .iter()
        .enumerate()
        .map(|(i, s)| track_template(s, &format!("{path}/style/{field}/{i}"), errors))
        .collect()
}

/// One template entry: a `repeat(...)`, else a single track.
fn track_template(spec: &TrackSpec, path: &str, errors: &mut Vec<DescribeError>) -> GridTemplate {
    if let TrackSpec::Structured(obj) = spec
        && let Some(rep) = &obj.repeat
    {
        let count = repeat_count(&rep.count, path, errors);
        let tracks = rep
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| leaf_track(t, &format!("{path}/repeat/tracks/{i}"), errors))
            .collect();
        return GridTemplate::Repeat(count, tracks);
    }
    GridTemplate::Single(leaf_track(spec, path, errors))
}

/// A single (non-repeat) track; a `repeat` here is an error, degrading to `1fr`.
fn leaf_track(spec: &TrackSpec, path: &str, errors: &mut Vec<DescribeError>) -> Track {
    match spec {
        TrackSpec::Keyword(s) => track_keyword(s, path, errors),
        TrackSpec::Structured(obj) => {
            if obj.repeat.is_some() {
                errors.push(DescribeError::new(path, "nested repeat is not allowed"));
                return Track::Fr(1.0);
            }
            if let Some([mn, mx]) = &obj.minmax {
                return Track::MinMax(track_min(mn, path, errors), track_max(mx, path, errors));
            }
            if let Some(px) = obj.fit_content {
                if finite_num(px, path, "fit_content", errors) {
                    return Track::FitContent(px);
                }
                return Track::Fr(1.0);
            }
            errors.push(DescribeError::new(
                path,
                "empty track object: set one of minmax, fit_content, repeat",
            ));
            Track::Fr(1.0)
        }
    }
}

/// Parses a track keyword/length string (`"<n>fr"`, `"<n>px"`, `"auto"`,
/// `"min-content"`, `"max-content"`) into a [`Track`].
fn track_keyword(s: &str, path: &str, errors: &mut Vec<DescribeError>) -> Track {
    let t = s.trim();
    if let Some(n) = t
        .strip_suffix("fr")
        .and_then(|n| n.trim().parse::<f32>().ok())
        && n.is_finite()
    {
        return Track::Fr(n);
    }
    if let Some(n) = t
        .strip_suffix("px")
        .and_then(|n| n.trim().parse::<f32>().ok())
        && n.is_finite()
    {
        return Track::Px(n);
    }
    match t {
        "auto" => Track::Auto,
        "min-content" => Track::MinContent,
        "max-content" => Track::MaxContent,
        _ => {
            errors.push(DescribeError::new(
                path,
                format!(
                    "unknown track {s:?}; use \"<n>fr\", \"<n>px\", \"auto\", \"min-content\", or \"max-content\""
                ),
            ));
            Track::Fr(1.0)
        }
    }
}

/// Parses the `min` side of a `minmax` — `fr` is not allowed for a floor.
fn track_min(s: &str, path: &str, errors: &mut Vec<DescribeError>) -> TrackMin {
    match track_keyword(s, path, errors) {
        Track::Px(v) => TrackMin::Px(v),
        Track::MinContent => TrackMin::MinContent,
        Track::MaxContent => TrackMin::MaxContent,
        Track::Fr(_) => {
            errors.push(DescribeError::new(
                path,
                "minmax min cannot be a fraction (fr); use px, auto, min-content, or max-content",
            ));
            TrackMin::Auto
        }
        _ => TrackMin::Auto,
    }
}

/// Parses the `max` side of a `minmax`.
fn track_max(s: &str, path: &str, errors: &mut Vec<DescribeError>) -> TrackMax {
    match track_keyword(s, path, errors) {
        Track::Px(v) => TrackMax::Px(v),
        Track::Fr(v) => TrackMax::Fr(v),
        Track::MinContent => TrackMax::MinContent,
        Track::MaxContent => TrackMax::MaxContent,
        _ => TrackMax::Auto,
    }
}

/// Parses a repeat count: a positive integer, or `"auto-fit"` / `"auto-fill"`.
fn repeat_count(c: &RepeatCount, path: &str, errors: &mut Vec<DescribeError>) -> Repeat {
    match c {
        RepeatCount::Count(n) => Repeat::Count((*n).max(1)),
        RepeatCount::Keyword(k) => match k.as_str() {
            "auto-fit" => Repeat::AutoFit,
            "auto-fill" => Repeat::AutoFill,
            _ => {
                errors.push(DescribeError::new(
                    path,
                    format!(
                        "unknown repeat count {k:?}; use a positive integer, \"auto-fit\", or \"auto-fill\""
                    ),
                ));
                Repeat::AutoFit
            }
        },
    }
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
