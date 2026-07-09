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

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use fenestra_core::{
    AdaptiveTint, Color, DrawerSide, Element, ElementFilter, GridTemplate, Material, Repeat,
    ShadowToken, Sheen, SpecularEdge, Surface, TextAlign, Theme, Track, TrackMax, TrackMin, Weight,
    col, div, divider, frame_epoch, image_from_data, image_payload, linear_gradient, row, spacer,
    stack, text,
};
use fenestra_kit::{
    ButtonVariant, Status as KitStatus, TreeNode as KitTreeNode, accordion, accordion_item, avatar,
    badge, breadcrumbs, button, callout, card, checkbox, color_picker, combobox, command_palette,
    crumb, data_table, date_picker, date_range_picker, drawer, dropdown_menu, field,
    format_color_text, kbd, kbd_raised, menubar, meter, modal, multi_select, pagination,
    parse_color_text, popover, progress, progress_indeterminate, radio, segmented, select,
    skeleton, skeleton_circle, skeleton_text, slider, spin_button, spinner, split_pane, stat_card,
    status as kit_status, stepper, switch, tabs, tag_input, text_area, text_input, toast_stack,
    toolbar, tooltip, tree_view, virtual_list,
};
use image::{ImageFormat, ImageReader, Limits};

use crate::color::resolve_color;
use crate::error::DescribeError;
use crate::format::{
    AccordionNode, AdaptiveSpec, AvatarNode, BadgeNode, BreadcrumbsNode, CalloutNode,
    ColorPickerNode, ComboboxNode, CommandPaletteNode, Container, DataTableNode, DatePickerNode,
    DateSpec, Description, DrawerNode, DropdownMenuNode, EdgeSpec, FieldNode, FilterSpec, IconNode,
    ImageNode, InputNode, KbdNode, Leaf, MenubarNode, MeterNode, ModalNode, MultiSelectNode, Node,
    PaginationNode, PopoverNode, ProgressNode, RadioNode, RepeatCount, SCHEMA_V1, SegmentedNode,
    SelectNode, SheenSpec, SkeletonNode, SpinButtonNode, SplitPaneNode, StatCardNode, StatusNode,
    StepperNode, Style, TabsNode, TagInputNode, TextNode, ToastStackNode, ToolbarNode, TooltipNode,
    TrackSpec, TreeItemDto, TreeViewNode, VirtualListNode,
};
use crate::state::{Action, StateMap, bound_bool, bound_number, bound_text};

/// The largest blur radius (logical px) an authored document may request. A real
/// frosted-glass blur is far under this; the cap keeps a hostile value from
/// reaching the headless blur pipeline as an unbounded box-window. (The pipeline
/// caps the box radius at the image extent as a backstop.)
const MAX_BLUR_PX: f32 = 200.0;

/// The largest number of items an authored literal collection (table rows,
/// tree nodes, virtual-list rows, toast/option/command/tag lists, …) may
/// carry. These are not amplifying like `grid_cols`' `repeat` count or
/// pagination's `siblings` (both bounded deeper in `fenestra-core`/
/// `fenestra-kit`) — every item here is a literal element proportional to the
/// JSON actually sent — but a bound still keeps parse-time work (and, for
/// `virtual_list`, the per-row rebuild closure) sane against a hostile payload.
pub const MAX_LIST_ITEMS: usize = 1000;

/// The largest number of columns a `data_table` may declare. Columns become
/// grid tracks, so this stays well under `fenestra-core`'s own
/// `MAX_GRID_TRACKS` (1024) ceiling.
const MAX_TABLE_COLUMNS: usize = 128;

/// The largest base64-encoded `image` payload (characters), checked before
/// any decode work starts.
const MAX_IMAGE_B64_LEN: usize = 8 * 1024 * 1024;

/// The largest width or height (px) a decoded `image` may have. Enforced
/// through the `image` crate's own [`Limits`] *before* the pixel buffer is
/// allocated — the PNG decoder checks the IHDR-declared dimensions as soon as
/// it reads the header — so a "decompression bomb" (a tiny file whose header
/// declares an enormous canvas) is rejected pre-decode, not after.
const MAX_IMAGE_DIM: u32 = 8192;

/// The largest *aggregate* decoded-RGBA byte count a single [`to_element`] /
/// [`to_element_lenient`] call may spend across every `image` node in the
/// document (not just one). The per-image dimension cap bounds each image
/// individually (8192×8192×4 ≈ 256 MiB), but a document can nest arbitrarily
/// many images —
/// as children of one container, as `virtual_list` items, nested in
/// `field`/`split_pane`/`tooltip`/`popover`/`dropdown_menu` content, and so
/// on — and a solid-color PNG compresses to a few hundred KiB regardless of
/// its declared canvas, so the *per-image* base64/dimension clamps alone do
/// nothing to bound the sum. 384 MiB comfortably fits one full-size image
/// (~256 MiB) plus headroom for a handful of smaller ones, while still
/// refusing a document that strings together enough images to exhaust host
/// memory. A `budget: &mut usize` threaded through every function that can
/// reach an `image` node (see `node_to_element` and its recursive callers)
/// starts at this value and is spent — via the `image` crate's own
/// `Limits::max_alloc`, so a too-large decode is refused *before* allocating,
/// not after — as each image commits; once exhausted, further images degrade
/// to a spacer with a path-pointed error, exactly like any other per-image
/// failure.
pub const MAX_TOTAL_IMAGE_BYTES: usize = 384 * 1024 * 1024;

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
    let mut budget = MAX_TOTAL_IMAGE_BYTES;
    let el = node_to_element(&desc.root, theme, state, "root", &mut budget, &mut errors);
    (el, errors)
}

/// Maps one node to an element, recursing into children. Always produces an
/// element; soft problems append to `errors`. `budget` is the remaining
/// aggregate decoded-image byte allowance for this call (see
/// [`MAX_TOTAL_IMAGE_BYTES`]) — threaded through every recursive path that
/// can reach an `image` node, spent by `image_node`.
fn node_to_element(
    node: &Node,
    theme: &Theme,
    state: &StateMap,
    path: &str,
    budget: &mut usize,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    match node {
        // ── Layout containers ─────────────────────────────────────────────────
        Node::Row(c) => container(row(), c, theme, state, path, budget, errors),
        Node::Col(c) => container(col(), c, theme, state, path, budget, errors),
        Node::Div(c) => container(div(), c, theme, state, path, budget, errors),
        Node::Stack(c) => container(stack(), c, theme, state, path, budget, errors),
        Node::Card(c) => container(card(), c, theme, state, path, budget, errors),
        Node::SplitPane(s) => split_pane_node(s, theme, state, path, budget, errors),
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
        Node::Field(f) => field_node(f, theme, state, path, budget, errors),
        Node::Combobox(c) => combobox_node(c, state, path, errors),
        Node::MultiSelect(m) => multi_select_node(m, path, errors),
        Node::TagInput(t) => tag_input_node(t, path, errors),
        Node::DatePicker(d) => date_picker_node(d, path, errors),
        Node::ColorPicker(c) => color_picker_node(c, state, path, errors),
        // ── Navigation ────────────────────────────────────────────────────────
        Node::Tabs(t) => tabs_node(t, state),
        Node::Segmented(s) => segmented_node(s, state),
        Node::Breadcrumbs(b) => breadcrumbs_node(b),
        Node::Pagination(p) => pagination_node(p, state),
        Node::Stepper(s) => stepper_node(s, state),
        Node::Toolbar(t) => toolbar_node(t, theme, state, path, budget, errors),
        Node::Menubar(m) => menubar_node(m),
        Node::Tree(t) => tree_view_node(t, path, errors),
        // ── Display / feedback ─────────────────────────────────────────────────
        Node::Badge(b) => badge_node(b, path, errors),
        Node::Callout(c) => callout_node(c, path, errors),
        Node::StatCard(s) => stat_card_node(s, path, errors),
        Node::Avatar(a) => avatar_node(a),
        Node::Status(s) => status_node(s, path, errors),
        Node::Kbd(k) => kbd_node(k),
        Node::Progress(p) => progress_node(p),
        Node::Meter(m) => meter_node(m, state),
        Node::Accordion(a) => accordion_node(a, theme, state, path, budget, errors),
        Node::Spinner(l) => leaf(spinner(), l, theme, path, errors),
        Node::Skeleton(k) => skeleton_node(k),
        Node::Icon(i) => icon_node(i, path, errors),
        Node::Image(i) => image_node(i, theme, path, budget, errors),
        Node::Toast(t) => toast_stack_node(t, path, errors),
        // ── Data ──────────────────────────────────────────────────────────────
        Node::DataTable(d) => data_table_node(d, path, errors),
        Node::VirtualList(v) => virtual_list_node(v, theme, state, path, budget, errors),
        // ── Overlays ──────────────────────────────────────────────────────────
        Node::Modal(m) => modal_node(m, theme, state, path, budget, errors),
        Node::Tooltip(t) => tooltip_node(t, theme, state, path, budget, errors),
        Node::Drawer(d) => drawer_node(d, theme, state, path, budget, errors),
        Node::Popover(p) => popover_node(p, theme, state, path, budget, errors),
        Node::DropdownMenu(d) => dropdown_menu_node(d, theme, state, path, budget, errors),
        Node::CommandPalette(c) => command_palette_node(c, state, path, errors),
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
    budget: &mut usize,
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
        let c = node_to_element(
            child,
            theme,
            state,
            &format!("{path}/children/{i}"),
            budget,
            errors,
        );
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
    budget: &mut usize,
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
        let c = node_to_element(
            child,
            theme,
            state,
            &format!("{path}/children/{i}"),
            budget,
            errors,
        );
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
    budget: &mut usize,
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
            budget,
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
                    fenestra_kit::icons::lucide::names()
                        .collect::<Vec<_>>()
                        .join(", ")
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
    budget: &mut usize,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let children: Vec<Element<Action>> = m
        .children
        .iter()
        .enumerate()
        .map(|(i, child)| {
            node_to_element(
                child,
                theme,
                state,
                &format!("{path}/children/{i}"),
                budget,
                errors,
            )
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
    budget: &mut usize,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let target_el = node_to_element(
        &t.target,
        theme,
        state,
        &format!("{path}/target"),
        budget,
        errors,
    );
    let el: Element<Action> = tooltip(target_el, t.label.clone());
    if let Some(id) = &t.id { el.id(id) } else { el }
}

fn popover_node(
    p: &PopoverNode,
    theme: &Theme,
    state: &StateMap,
    path: &str,
    budget: &mut usize,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let trigger = node_to_element(
        &p.trigger,
        theme,
        state,
        &format!("{path}/trigger"),
        budget,
        errors,
    );
    let content = node_to_element(
        &p.content,
        theme,
        state,
        &format!("{path}/content"),
        budget,
        errors,
    );
    let el = trigger.child(popover(content));
    if let Some(id) = &p.id { el.id(id) } else { el }
}

fn dropdown_menu_node(
    d: &DropdownMenuNode,
    theme: &Theme,
    state: &StateMap,
    path: &str,
    budget: &mut usize,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let trigger = node_to_element(
        &d.trigger,
        theme,
        state,
        &format!("{path}/trigger"),
        budget,
        errors,
    );
    if d.items.len() > MAX_LIST_ITEMS {
        errors.push(DescribeError::new(
            format!("{path}/items"),
            format!(
                "dropdown_menu items must be <= {MAX_LIST_ITEMS}; got {}",
                d.items.len()
            ),
        ));
    }
    let items: Vec<(String, Action)> = d
        .items
        .iter()
        .take(MAX_LIST_ITEMS)
        .map(|it| (it.label.clone(), intent_or_empty(&it.on_select)))
        .collect();
    let el = trigger.child(dropdown_menu(items));
    if let Some(id) = &d.id { el.id(id) } else { el }
}

fn command_palette_node(
    c: &CommandPaletteNode,
    state: &StateMap,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    if c.commands.len() > MAX_LIST_ITEMS {
        errors.push(DescribeError::new(
            format!("{path}/commands"),
            format!(
                "command_palette commands must be <= {MAX_LIST_ITEMS}; got {}",
                c.commands.len()
            ),
        ));
    }
    let query = c
        .bind
        .as_ref()
        .map_or_else(|| c.query.clone(), |k| bound_text(state, k, &c.query));
    let commands: Vec<(String, Action)> = c
        .commands
        .iter()
        .take(MAX_LIST_ITEMS)
        .map(|it| (it.label.clone(), intent_or_empty(&it.on_select)))
        .collect();
    // Present in the tree = shown (the `modal`/`drawer` contract) — always open.
    let mut w = command_palette(query, true, commands);
    if let Some(key) = &c.bind {
        let key = key.clone();
        w = w.on_input(move |s| Action::SetText(key.clone(), s));
    }
    if let Some(intent) = &c.on_close {
        w = w.on_close(Action::Intent(intent.clone()));
    }
    if let Some(key) = &c.id {
        w = w.id(key);
    }
    let el: Element<Action> = w.into();
    if let Some(id) = &c.id { el.id(id) } else { el }
}

/// Maps an optional author intent string to an [`Action`]: the intent when
/// set, else an empty inert intent (the same fallback `menubar_node` uses for
/// an item with no `on_select`).
fn intent_or_empty(intent: &Option<String>) -> Action {
    intent.as_ref().map_or_else(
        || Action::Intent(String::new()),
        |s| Action::Intent(s.clone()),
    )
}

// ── Form / field helpers ──────────────────────────────────────────────────────

fn field_node(
    f: &FieldNode,
    theme: &Theme,
    state: &StateMap,
    path: &str,
    budget: &mut usize,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let control = node_to_element(
        &f.control,
        theme,
        state,
        &format!("{path}/control"),
        budget,
        errors,
    );
    let mut w = field(f.label.clone()).child(control).required(f.required);
    if let Some(err) = &f.error {
        w = w.error(err.clone());
    } else if let Some(help) = &f.help {
        w = w.help(help.clone());
    }
    let el: Element<Action> = w.into();
    if let Some(id) = &f.id { el.id(id) } else { el }
}

fn split_pane_node(
    s: &SplitPaneNode,
    theme: &Theme,
    state: &StateMap,
    path: &str,
    budget: &mut usize,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let fraction = s
        .bind
        .as_ref()
        .map_or(s.fraction, |k| bound_number(state, k, s.fraction));
    let first = node_to_element(
        &s.first,
        theme,
        state,
        &format!("{path}/first"),
        budget,
        errors,
    );
    let second = node_to_element(
        &s.second,
        theme,
        state,
        &format!("{path}/second"),
        budget,
        errors,
    );
    let mut w = split_pane(fraction, first, second);
    if s.vertical {
        w = w.vertical();
    }
    match &s.bind {
        Some(key) => {
            let key = key.clone();
            w = w.on_resize(move |f| Action::SetNumber(key.clone(), f));
        }
        None => {
            if let Some(intent) = &s.on_resize {
                let intent = intent.clone();
                w = w.on_resize(move |_| Action::Intent(intent.clone()));
            }
        }
    }
    let el: Element<Action> = w.into();
    if let Some(id) = &s.id { el.id(id) } else { el }
}

fn combobox_node(
    c: &ComboboxNode,
    state: &StateMap,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    if c.options.len() > MAX_LIST_ITEMS {
        errors.push(DescribeError::new(
            format!("{path}/options"),
            format!(
                "combobox options must be <= {MAX_LIST_ITEMS}; got {}",
                c.options.len()
            ),
        ));
    }
    let options: Vec<String> = c.options.iter().take(MAX_LIST_ITEMS).cloned().collect();
    let value = c
        .bind
        .as_ref()
        .map_or_else(|| c.value.clone(), |k| bound_text(state, k, &c.value));
    let mut w = combobox(value, c.open, options);
    if let Some(ph) = &c.placeholder {
        w = w.placeholder(ph.clone());
    }
    // `bind` takes priority: both typing and picking write the same state key
    // (the same "framework owns the transition" contract as `text_input`).
    match &c.bind {
        Some(key) => {
            let key_in = key.clone();
            w = w.on_input(move |s| Action::SetText(key_in.clone(), s));
            let key_pick = key.clone();
            w = w.on_pick(move |s| Action::SetText(key_pick.clone(), s));
        }
        None => {
            if let Some(intent) = &c.on_input {
                let intent = intent.clone();
                w = w.on_input(move |_| Action::Intent(intent.clone()));
            }
            if let Some(intent) = &c.on_pick {
                let intent = intent.clone();
                w = w.on_pick(move |_| Action::Intent(intent.clone()));
            }
        }
    }
    if let Some(intent) = &c.on_close {
        w = w.on_close(Action::Intent(intent.clone()));
    }
    if let Some(key) = &c.id {
        w = w.id(key);
    }
    let el: Element<Action> = w.into();
    if let Some(id) = &c.id { el.id(id) } else { el }
}

fn multi_select_node(
    m: &MultiSelectNode,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    if m.options.len() > MAX_LIST_ITEMS {
        errors.push(DescribeError::new(
            format!("{path}/options"),
            format!(
                "multi_select options must be <= {MAX_LIST_ITEMS}; got {}",
                m.options.len()
            ),
        ));
    }
    let options: Vec<String> = m.options.iter().take(MAX_LIST_ITEMS).cloned().collect();
    let mut w = multi_select(m.selected.iter().copied(), options).disabled(m.disabled);
    if let Some(key) = &m.id {
        w = w.id(key);
    }
    if let Some(intent) = &m.on_toggle {
        let intent = intent.clone();
        w = w.on_toggle(move |_| Action::Intent(intent.clone()));
    }
    let el: Element<Action> = w.into();
    if let Some(id) = &m.id { el.id(id) } else { el }
}

fn tag_input_node(
    t: &TagInputNode,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    if t.tags.len() > MAX_LIST_ITEMS {
        errors.push(DescribeError::new(
            format!("{path}/tags"),
            format!(
                "tag_input tags must be <= {MAX_LIST_ITEMS}; got {}",
                t.tags.len()
            ),
        ));
    }
    let tags: Vec<String> = t.tags.iter().take(MAX_LIST_ITEMS).cloned().collect();
    let mut w = tag_input(tags);
    if let Some(ph) = &t.placeholder {
        w = w.placeholder(ph.clone());
    }
    if let Some(key) = &t.id {
        w = w.id(key);
    }
    if let Some(intent) = &t.on_remove {
        let intent = intent.clone();
        w = w.on_remove(move |_| Action::Intent(intent.clone()));
    }
    if let Some(intent) = &t.on_add {
        let intent = intent.clone();
        w = w.on_add(move |_| Action::Intent(intent.clone()));
    }
    let el: Element<Action> = w.into();
    if let Some(id) = &t.id { el.id(id) } else { el }
}

/// Validates and converts an authored `[y, m, d]` into the kit's `Date`
/// tuple. An out-of-range month/day is a path-pointed error (like an unknown
/// enum token elsewhere in this file), then clamped into range rather than
/// passed on as a huge wrapped value.
fn validate_date(d: DateSpec, path: &str, errors: &mut Vec<DescribeError>) -> fenestra_kit::Date {
    let [y, m, day] = d;
    if !(1..=12).contains(&m) {
        errors.push(DescribeError::new(
            format!("{path}/1"),
            format!("month must be 1..=12; got {m}"),
        ));
    }
    if !(1..=31).contains(&day) {
        errors.push(DescribeError::new(
            format!("{path}/2"),
            format!("day must be 1..=31; got {day}"),
        ));
    }
    #[expect(
        clippy::cast_sign_loss,
        reason = "clamped to 1..=31 immediately above, so the value is always positive"
    )]
    (y, m.clamp(1, 12) as u32, day.clamp(1, 31) as u32)
}

fn date_picker_node(
    d: &DatePickerNode,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let month = if (1..=12).contains(&d.month) {
        d.month
    } else {
        errors.push(DescribeError::new(
            format!("{path}/month"),
            format!("month must be 1..=12; got {}", d.month),
        ));
        d.month.clamp(1, 12)
    };
    let mut w = if d.range {
        date_range_picker((d.year, month))
    } else {
        date_picker((d.year, month))
    };
    if d.range {
        let start = d
            .range_start
            .map(|s| validate_date(s, &format!("{path}/range_start"), errors));
        let end = d
            .range_end
            .map(|e| validate_date(e, &format!("{path}/range_end"), errors));
        w = w.range(start, end);
    } else {
        w = w.selected(
            d.selected
                .map(|s| validate_date(s, &format!("{path}/selected"), errors)),
        );
    }
    if let Some(t) = d.today {
        w = w.today(validate_date(t, &format!("{path}/today"), errors));
    }
    if let Some(mn) = d.min {
        w = w.min(validate_date(mn, &format!("{path}/min"), errors));
    }
    if let Some(mx) = d.max {
        w = w.max(validate_date(mx, &format!("{path}/max"), errors));
    }
    if let Some(key) = &d.id {
        w = w.id(key);
    }
    // `on_focus` (the WAI-ARIA keyboard grid cursor) is not wired — see the
    // `DatePickerNode` doc comment — so arrow-key navigation is silently
    // inert; Enter/Space still select the widget's own computed default
    // focus, and click-to-pick always works.
    if d.range {
        if let Some(intent) = &d.on_pick {
            let intent = intent.clone();
            w = w.on_pick_range(move |_| Action::Intent(intent.clone()));
        }
    } else if let Some(intent) = &d.on_pick {
        let intent = intent.clone();
        w = w.on_pick(move |_| Action::Intent(intent.clone()));
    }
    if let Some(intent) = &d.on_month {
        let intent = intent.clone();
        w = w.on_month(move |_| Action::Intent(intent.clone()));
    }
    let el: Element<Action> = w.into();
    if let Some(id) = &d.id { el.id(id) } else { el }
}

/// sRGB middle gray (`#808080`) a `color_picker` falls back to when its
/// `value` (or the bound state text overriding it) fails to parse — plain
/// sRGB rather than an OKLCH construction, since OKLCH lightness is
/// perceptual (`oklch(0.5, 0, 0)` renders as `#636363`, not `#808080`) and
/// the documented fallback is specifically the hex `#808080`.
fn fallback_picker_color() -> Color {
    Color::from_rgba8(0x80, 0x80, 0x80, 0xFF)
}

fn color_picker_node(
    c: &ColorPickerNode,
    state: &StateMap,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let text = c
        .bind
        .as_ref()
        .map_or_else(|| c.value.clone(), |k| bound_text(state, k, &c.value));
    let color = parse_color_text(&text).unwrap_or_else(|| {
        errors.push(DescribeError::new(
            format!("{path}/value"),
            format!("{text:?} is not a valid hex or oklch() color; falling back to #808080"),
        ));
        fallback_picker_color()
    });
    let mut w = color_picker(color);
    if let Some(label) = &c.label {
        w = w.label(label.clone());
    }
    w = w.disabled(c.disabled);
    if let Some(side) = c.pad_size {
        if side.is_finite() {
            w = w.pad_size(side);
        } else {
            errors.push(DescribeError::new(
                format!("{path}/pad_size"),
                format!("pad_size must be a finite number; got {side}"),
            ));
        }
    }
    // `bind` takes priority: both the pad/hue/alpha gestures and a text edit
    // that currently parses commit the formatted hex back to the same key —
    // an edit that doesn't yet parse leaves it alone (see the node's doc
    // comment for why there is only one state slot, not a separate draft).
    match &c.bind {
        Some(key) => {
            let key_change = key.clone();
            w = w.on_change(move |color| {
                Action::SetText(key_change.clone(), format_color_text(color))
            });
            let key_text = key.clone();
            w = w.on_text_change(move |_, parsed| match parsed {
                Some(color) => Action::SetText(key_text.clone(), format_color_text(color)),
                None => Action::Intent(String::new()),
            });
        }
        None => {
            if let Some(intent) = &c.on_change {
                let intent_change = intent.clone();
                w = w.on_change(move |_| Action::Intent(intent_change.clone()));
                let intent_text = intent.clone();
                w = w.on_text_change(move |_, _| Action::Intent(intent_text.clone()));
            }
        }
    }
    if let Some(key) = &c.id {
        w = w.id(key);
    }
    let el: Element<Action> = w.into();
    if let Some(id) = &c.id { el.id(id) } else { el }
}

// ── Tree helpers ──────────────────────────────────────────────────────────────

fn tree_view_node(
    t: &TreeViewNode,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let mut count = 0usize;
    let mut truncated = false;
    let roots: Vec<KitTreeNode> = t
        .items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            tree_item(
                item,
                &format!("{path}/items/{i}"),
                &mut count,
                &mut truncated,
            )
        })
        .collect();
    if truncated {
        errors.push(DescribeError::new(
            format!("{path}/items"),
            format!("tree has more than {MAX_LIST_ITEMS} total nodes; the excess is dropped"),
        ));
    }
    let mut w = tree_view(roots);
    if !t.expanded.is_empty() {
        w = w.expanded(t.expanded.clone());
    }
    w = w.selected(t.selected.clone());
    if let Some(intent) = &t.on_toggle {
        let intent = intent.clone();
        w = w.on_toggle(move |_| Action::Intent(intent.clone()));
    }
    if let Some(intent) = &t.on_select {
        let intent = intent.clone();
        w = w.on_select(move |_| Action::Intent(intent.clone()));
    }
    // `TreeView` has no builder-level `.id()`, so the stable key is applied
    // only at the `Element` level below.
    let el: Element<Action> = w.into();
    if let Some(id) = &t.id { el.id(id) } else { el }
}

/// Converts one authored tree item (and its children) into the kit's
/// `TreeNode`, truncating total fan-out at [`MAX_LIST_ITEMS`] — a node count
/// cap, not a depth cap. JSON *nesting depth* is already bounded well before
/// this runs: `serde_json` caps deserialization recursion at 128 levels by
/// default, so a maliciously deep `children` chain fails to parse at all
/// long before it could reach this function. Returns `None` (dropping the
/// node and its whole subtree) once the cap is hit.
fn tree_item(
    item: &TreeItemDto,
    path: &str,
    count: &mut usize,
    truncated: &mut bool,
) -> Option<KitTreeNode> {
    if *count >= MAX_LIST_ITEMS {
        *truncated = true;
        return None;
    }
    *count += 1;
    let children: Vec<KitTreeNode> = item
        .children
        .iter()
        .enumerate()
        .filter_map(|(i, child)| {
            tree_item(child, &format!("{path}/children/{i}"), count, truncated)
        })
        .collect();
    Some(KitTreeNode::new(item.id.clone(), item.label.clone()).children(children))
}

// ── Image helpers ─────────────────────────────────────────────────────────────

/// Decodes strict RFC 4648 standard-alphabet base64 (`A`–`Z`, `a`–`z`, `0`–`9`,
/// `+`, `/`, `=` padding). No dependency — fenestra-describe already writes
/// its own dependency-free lexers rather than pull one in for a single
/// decode (see fenestra-markdown's syntax lexer). Rejects, never panics on: a
/// non-alphabet byte, a length not a multiple of 4, `=` outside the final
/// group or not a trailing suffix of it, a padding count other than 0/1/2,
/// and non-zero padding bits (a non-canonical encoding). Embedded whitespace
/// is also rejected — a hand-wrapped multi-line base64 string must be joined
/// into one line first.
#[expect(
    clippy::cast_possible_truncation,
    reason = "each output byte is masked out of a 24-bit accumulator before the `as u8`, so the \
              low 8 bits always hold the whole value"
)]
fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    let s = input.as_bytes();
    if s.is_empty() {
        return Ok(Vec::new());
    }
    if !s.len().is_multiple_of(4) {
        return Err(format!("base64 length {} is not a multiple of 4", s.len()));
    }
    fn val(b: u8) -> Option<u32> {
        match b {
            b'A'..=b'Z' => Some(u32::from(b - b'A')),
            b'a'..=b'z' => Some(u32::from(b - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(b - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let groups = s.len() / 4;
    let mut out = Vec::with_capacity(groups * 3);
    for (i, group) in s.chunks_exact(4).enumerate() {
        let is_last = i + 1 == groups;
        let pad = group.iter().rev().take_while(|&&b| b == b'=').count();
        if pad > 0 && !is_last {
            return Err("'=' padding may only appear in the final group".to_string());
        }
        if pad > 2 {
            return Err("a base64 group may end in at most two '=' characters".to_string());
        }
        let data_len = 4 - pad;
        for (j, &b) in group.iter().enumerate() {
            if b == b'=' && j < data_len {
                return Err("'=' padding must be a trailing suffix of its group".to_string());
            }
        }
        let mut v = [0u32; 4];
        for (j, &b) in group[..data_len].iter().enumerate() {
            v[j] = val(b).ok_or_else(|| format!("invalid base64 character {:?}", b as char))?;
        }
        let bits = (v[0] << 18) | (v[1] << 12) | (v[2] << 6) | v[3];
        match pad {
            0 => {
                out.push((bits >> 16) as u8);
                out.push((bits >> 8) as u8);
                out.push(bits as u8);
            }
            1 => {
                if bits & 0xFF != 0 {
                    return Err("non-zero padding bits in the final base64 group".to_string());
                }
                out.push((bits >> 16) as u8);
                out.push((bits >> 8) as u8);
            }
            2 => {
                if bits & 0xFFFF != 0 {
                    return Err("non-zero padding bits in the final base64 group".to_string());
                }
                out.push((bits >> 16) as u8);
            }
            _ => unreachable!("pad was checked to be 0..=2 above"),
        }
    }
    Ok(out)
}

// ── decoded-image cache ───────────────────────────────────────────────────────
//
// `image_node` decodes its base64 PNG on every build, and `view()` rebuilds the
// whole element tree on every frame (each click, every `pump`, every ~200ms
// preview poll). Without a cache, an image in a driven or animated view re-runs
// the full base64 → PNG → RGBA8 pipeline every frame — turning e.g. an
// `interact` `{"tab": 4096}` step (up to `MAX_TAB_REPEAT` rebuilds) into
// thousands of ~256 MiB decodes. This thread-local cache decodes each distinct
// payload once and hands every later build a clone of the *same* atomically
// reference-counted pixel blob (a cheap `ImageData` clone — no re-decode, no
// copy). It holds at most `MAX_TOTAL_IMAGE_BYTES` of decoded bytes, evicting
// least-recently-used, so it can never itself grow into a memory leak.

thread_local! {
    static DECODE_CACHE: RefCell<DecodeCache> = RefCell::new(DecodeCache::default());
    /// Count of real PNG decodes (cache misses) — lets tests assert reuse.
    static DECODE_COUNT: Cell<u64> = const { Cell::new(0) };
}

#[derive(Default)]
struct DecodeCache {
    entries: HashMap<u64, CachedImage>,
    total_bytes: usize,
    tick: u64,
}

struct CachedImage {
    data: fenestra_core::ImageData,
    bytes: usize,
    last_used: u64,
}

/// A 64-bit content hash of a base64 payload — the cache key. A collision would
/// serve one image's pixels for another's bytes; at 64 bits that is
/// astronomically unlikely for the handful of images a document carries.
fn image_cache_key(png_b64: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    png_b64.hash(&mut hasher);
    hasher.finish()
}

/// The cached decoded image for `key`, if resident, marked most-recently-used.
fn image_cache_get(key: u64) -> Option<fenestra_core::ImageData> {
    DECODE_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        c.tick += 1;
        let tick = c.tick;
        let entry = c.entries.get_mut(&key)?;
        entry.last_used = tick;
        Some(entry.data.clone())
    })
}

/// Inserts a freshly decoded image, first evicting least-recently-used entries
/// so total resident decoded bytes stay within [`MAX_TOTAL_IMAGE_BYTES`]. An
/// image as large as the whole budget is not cached (it would evict everything
/// and still dominate) — it is simply rebuilt on demand.
fn image_cache_put(key: u64, data: fenestra_core::ImageData, bytes: usize) {
    if bytes > MAX_TOTAL_IMAGE_BYTES {
        return;
    }
    DECODE_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        c.tick += 1;
        let last_used = c.tick;
        while c.total_bytes + bytes > MAX_TOTAL_IMAGE_BYTES {
            let Some(lru) = c
                .entries
                .iter()
                .min_by_key(|(_, e)| e.last_used)
                .map(|(&k, _)| k)
            else {
                break;
            };
            if let Some(removed) = c.entries.remove(&lru) {
                c.total_bytes -= removed.bytes;
            }
        }
        if let Some(prev) = c.entries.insert(
            key,
            CachedImage {
                data,
                bytes,
                last_used,
            },
        ) {
            c.total_bytes -= prev.bytes;
        }
        c.total_bytes += bytes;
    });
}

/// Frees every decoded image this thread's `fenestra/1` image cache holds,
/// reclaiming their memory. Rendering re-decodes (and re-caches) on demand, so
/// this only trades memory for a one-time re-decode — useful between rendering
/// unrelated documents in a long-lived process.
pub fn clear_image_cache() {
    DECODE_CACHE.with(|c| *c.borrow_mut() = DecodeCache::default());
}

/// Builds the image element from decoded pixel `data` and applies the node's
/// style, click intent, and id — shared by the cache-hit and freshly-decoded
/// paths so both render identically.
fn image_element(
    data: fenestra_core::ImageData,
    i: &ImageNode,
    theme: &Theme,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    let mut el = image_from_data(data).label(i.label.clone());
    el = apply_style(el, &i.style, theme, path, errors);
    if let Some(intent) = &i.on_click {
        el = el.on_click(Action::Intent(intent.clone()));
    }
    if let Some(id) = &i.id {
        el = el.id(id);
    }
    el
}

fn image_node(
    i: &ImageNode,
    theme: &Theme,
    path: &str,
    budget: &mut usize,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    if i.label.trim().is_empty() {
        errors.push(DescribeError::new(
            format!("{path}/label"),
            "image requires a non-empty accessible label (alt text)".to_string(),
        ));
    }
    // Reuse a previously decoded copy of this exact payload if one is resident:
    // no re-decode, no re-copy, and no budget charge (it shares the allocation
    // already committed rather than adding a new one).
    let key = image_cache_key(&i.png);
    if let Some(data) = image_cache_get(key) {
        return image_element(data, i, theme, path, errors);
    }
    if i.png.len() > MAX_IMAGE_B64_LEN {
        errors.push(DescribeError::new(
            format!("{path}/png"),
            format!(
                "base64 payload must be <= {MAX_IMAGE_B64_LEN} characters; got {}",
                i.png.len()
            ),
        ));
        return apply_style(spacer(), &i.style, theme, path, errors);
    }
    let bytes = match decode_base64(&i.png) {
        Ok(b) => b,
        Err(msg) => {
            errors.push(DescribeError::new(format!("{path}/png"), msg));
            return apply_style(spacer(), &i.style, theme, path, errors);
        }
    };
    // `Limits` is `#[non_exhaustive]`, so it cannot be struct-literal
    // constructed here even with `..Default::default()` — set the caps on
    // the default instance instead. `max_alloc` is this *document's*
    // remaining aggregate image budget (see `MAX_TOTAL_IMAGE_BYTES`), not
    // just this image's own allowance: the `image` crate checks it against
    // the *native* decoded size (`decoder.total_bytes()`) before allocating
    // the native-format pixel buffer, refusing cleanly rather than OOMing.
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIM);
    limits.max_image_height = Some(MAX_IMAGE_DIM);
    limits.max_alloc = Some(*budget as u64);
    let mut reader = ImageReader::with_format(std::io::Cursor::new(bytes), ImageFormat::Png);
    reader.limits(limits);
    let img = match reader.decode() {
        Ok(img) => img,
        Err(e) => {
            errors.push(DescribeError::new(
                format!("{path}/png"),
                format!("PNG decode failed: {e}"),
            ));
            return apply_style(spacer(), &i.style, theme, path, errors);
        }
    };
    let (w, h) = (img.width(), img.height());
    // The crate's own `max_alloc` check above only bounds the *native*-format
    // decode, but every image here always gets converted to RGBA8 next —
    // for a non-RGBA source that conversion allocates a *fresh, separate*
    // buffer the crate's check never saw, and it can be larger than the
    // native decode (e.g. a native grayscale image is 4x smaller than its
    // RGBA8 form). Check the actual RGBA8 byte count this node is about to
    // commit against the remaining budget ourselves before paying for it.
    let needed = u64::from(w) * u64::from(h) * 4;
    if needed > *budget as u64 {
        errors.push(DescribeError::new(
            format!("{path}/png"),
            format!(
                "image needs {needed} decoded bytes, exceeding the {budget} bytes remaining in \
                 this document's aggregate image budget ({MAX_TOTAL_IMAGE_BYTES} total); \
                 degrading to an empty spacer"
            ),
        ));
        return apply_style(spacer(), &i.style, theme, path, errors);
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "needed <= *budget (checked above), and budget is itself a usize"
    )]
    let needed_bytes = needed as usize;
    *budget -= needed_bytes;
    let pixels = img.to_rgba8().into_raw();
    DECODE_COUNT.with(|n| n.set(n.get() + 1));
    let data = image_payload(w, h, pixels);
    image_cache_put(key, data.clone(), needed_bytes);
    image_element(data, i, theme, path, errors)
}

// ── Toast helpers ─────────────────────────────────────────────────────────────

fn toast_stack_node(
    t: &ToastStackNode,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    if t.items.len() > MAX_LIST_ITEMS {
        errors.push(DescribeError::new(
            format!("{path}/items"),
            format!(
                "toast items must be <= {MAX_LIST_ITEMS}; got {}",
                t.items.len()
            ),
        ));
    }
    let toasts: Vec<(String, KitStatus)> = t
        .items
        .iter()
        .take(MAX_LIST_ITEMS)
        .enumerate()
        .map(|(i, item)| {
            let status =
                kit_status_from_str(&item.status, &format!("{path}/items/{i}"), "status", errors);
            (item.message.clone(), status)
        })
        .collect();
    let mut w = toast_stack(toasts);
    if let Some(width) = t.width {
        w = w.width(width);
    }
    if let Some(key) = &t.id {
        w = w.id(key);
    }
    if let Some(intent) = &t.on_dismiss {
        let intent = intent.clone();
        w = w.on_dismiss(move |_| Action::Intent(intent.clone()));
    }
    let el: Element<Action> = w.into();
    if let Some(id) = &t.id { el.id(id) } else { el }
}

// ── Data helpers ──────────────────────────────────────────────────────────────

fn data_table_node(
    d: &DataTableNode,
    path: &str,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    if d.columns.len() > MAX_TABLE_COLUMNS {
        errors.push(DescribeError::new(
            format!("{path}/columns"),
            format!(
                "data_table columns must be <= {MAX_TABLE_COLUMNS}; got {}",
                d.columns.len()
            ),
        ));
    }
    if d.rows.len() > MAX_LIST_ITEMS {
        errors.push(DescribeError::new(
            format!("{path}/rows"),
            format!(
                "data_table rows must be <= {MAX_LIST_ITEMS}; got {}",
                d.rows.len()
            ),
        ));
    }
    let columns: Vec<String> = d.columns.iter().take(MAX_TABLE_COLUMNS).cloned().collect();
    let rows: Vec<Vec<String>> = d.rows.iter().take(MAX_LIST_ITEMS).cloned().collect();
    let mut w = data_table(columns, rows);
    if let Some(key) = &d.id {
        w = w.id(key);
    }
    if let Some(sort) = &d.sort {
        w = w.sort(sort.column, sort.ascending);
    }
    w = w.selected(d.selected);
    if let Some(flags) = &d.selection {
        w = w.selection(flags.iter().copied());
    }
    if d.sticky_header {
        w = w.sticky_header(true);
    }
    if let Some(widths) = &d.column_widths {
        w = w.column_widths(widths.iter().copied());
    }
    if d.pinned_left > 0 {
        w = w.pinned_left(d.pinned_left);
    }
    if d.pinned_right > 0 {
        w = w.pinned_right(d.pinned_right);
    }
    if let Some(filter) = &d.filter {
        w = w.filter(filter.iter().cloned());
    }
    // Column resize/reorder are not authorable here — see the `DataTableNode`
    // doc comment; only the static `column_widths`/`pinned_left`/
    // `pinned_right` layout knobs are.
    if let Some(intent) = &d.on_sort {
        let intent = intent.clone();
        w = w.on_sort(move |_| Action::Intent(intent.clone()));
    }
    if let Some(intent) = &d.on_select {
        let intent = intent.clone();
        w = w.on_select(move |_| Action::Intent(intent.clone()));
    }
    if let Some(intent) = &d.on_select_row {
        let intent = intent.clone();
        w = w.on_select_row(move |_| Action::Intent(intent.clone()));
    }
    if let Some(intent) = &d.on_select_all {
        w = w.on_select_all(Action::Intent(intent.clone()));
    }
    if let Some(intent) = &d.on_filter {
        let intent = intent.clone();
        w = w.on_filter(move |_, _| Action::Intent(intent.clone()));
    }
    let el: Element<Action> = w.into();
    if let Some(id) = &d.id { el.id(id) } else { el }
}

fn virtual_list_node(
    v: &VirtualListNode,
    theme: &Theme,
    state: &StateMap,
    path: &str,
    budget: &mut usize,
    errors: &mut Vec<DescribeError>,
) -> Element<Action> {
    if v.items.len() > MAX_LIST_ITEMS {
        errors.push(DescribeError::new(
            format!("{path}/items"),
            format!(
                "virtual_list items must be <= {MAX_LIST_ITEMS}; got {}",
                v.items.len()
            ),
        ));
    }
    let items: Vec<Node> = v.items.iter().take(MAX_LIST_ITEMS).cloned().collect();
    // Validate every row now, eagerly, so parse errors surface on this call —
    // the built elements are discarded; the closure below rebuilds them
    // lazily (from the same source `Node`s) whenever a row scrolls into
    // view, since `Element` is not `Clone` and the closure must outlive this
    // function. That rebuild is real, ordinary node parsing — never a code
    // closure supplied by the author — so it carries the same "never
    // executable" guarantee as every other child in the tree. This eager
    // pass shares the *caller's* `budget`, so a `virtual_list` of many
    // large images is bounded by the same aggregate cap as everything else
    // in the document, not decoded unconditionally regardless of size.
    for (i, item) in items.iter().enumerate() {
        let _ = node_to_element(
            item,
            theme,
            state,
            &format!("{path}/items/{i}"),
            budget,
            errors,
        );
    }
    let row_height = if v.row_height.is_finite() && v.row_height > 0.0 {
        v.row_height
    } else {
        errors.push(DescribeError::new(
            format!("{path}/row_height"),
            format!(
                "row_height must be a finite positive number; got {}",
                v.row_height
            ),
        ));
        24.0
    };
    let items = Rc::new(items);
    let theme = theme.clone();
    let state = state.clone();
    let count = items.len();
    // One image-decode budget shared across every row the window materializes
    // in a single frame, reset each frame (keyed by `frame_epoch`). A per-row
    // reset was unsound: the virtual window is *not* bounded by a viewport-full
    // of rows — a tiny `row_height` collapses it onto every item at once
    // (`frame::virtual_window` has no `row_height` floor), and `expand_virtual`
    // materializes the whole window into one `Vec` in a single build. So a
    // per-row full budget let a `virtual_list` of large images decode the whole
    // list simultaneously on the paint path, defeating the aggregate cap. With
    // a shared per-frame budget, once it is spent the remaining rows' images are
    // refused *before* allocation (their `Limits::max_alloc` is near zero, so
    // the decode fails fast) and degrade to spacers — bounding both peak memory
    // and per-frame decode work regardless of how many rows the window covers.
    let frame_budget = Rc::new(Cell::new((u64::MAX, 0usize)));
    let el = virtual_list(count, row_height, move |i| {
        let epoch = frame_epoch();
        let (seen, remaining) = frame_budget.get();
        let mut budget = if seen == epoch {
            remaining
        } else {
            MAX_TOTAL_IMAGE_BYTES
        };
        let mut scratch = Vec::new();
        let el = node_to_element(
            &items[i],
            &theme,
            &state,
            "virtual_list/items",
            &mut budget,
            &mut scratch,
        );
        frame_budget.set((epoch, budget));
        el
    });
    if let Some(id) = &v.id { el.id(id) } else { el }
}

// ── Icon registry ─────────────────────────────────────────────────────────────

/// Maps a kebab-case icon name to a Lucide element, or `None` for unknown names.
/// Delegates to the kit's vendored registry so the authorable set never drifts
/// from what the kit ships (a hand-maintained copy here had already gone stale).
/// A few names earlier releases advertised are kept as back-compat aliases for
/// the kit's current canonical spellings (the kit renamed them to match Lucide).
fn named_icon(name: &str) -> Option<Element<Action>> {
    let name = match name {
        "home" => "house",
        "alert-triangle" => "triangle-alert",
        "trash" => "trash-2",
        other => other,
    };
    fenestra_kit::icons::lucide::by_name(name)
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

/// Maps a [`FilterSpec`] to a core [`ElementFilter`]. Infallible.
fn filter_of(spec: &FilterSpec) -> ElementFilter {
    match spec {
        FilterSpec::Blur(r) => ElementFilter::Blur(*r),
        FilterSpec::Brightness(m) => ElementFilter::Brightness(*m),
        FilterSpec::Saturate(m) => ElementFilter::Saturate(*m),
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
    budget: &mut usize,
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
            node_to_element(
                child,
                theme,
                state,
                &format!("{path}/children/{i}"),
                budget,
                errors,
            )
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
    if let Some([tl, tr, br, bl]) = style.corners {
        let all_finite = finite_num(tl, path, "corners/0", errors)
            && finite_num(tr, path, "corners/1", errors)
            && finite_num(br, path, "corners/2", errors)
            && finite_num(bl, path, "corners/3", errors);
        if all_finite {
            el = el.corners(tl, tr, br, bl);
        }
    }
    if style.rounded_full == Some(true) {
        el = el.rounded_full();
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
        el = el.backdrop_blur(v.clamp(0.0, MAX_BLUR_PX));
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
    // ── Transforms (paint-time; no layout effect) ─────────────────────────────
    if let Some([x, y]) = style.translate
        && finite_num(x, path, "translate/0", errors)
        && finite_num(y, path, "translate/1", errors)
    {
        el = el.translate(x, y);
    }
    if let Some(v) = style.rotate
        && finite_num(v, path, "rotate", errors)
    {
        el = el.rotate(v);
    }
    if let Some([x, y]) = style.skew
        && finite_num(x, path, "skew/0", errors)
        && finite_num(y, path, "skew/1", errors)
    {
        el = el.skew(x, y);
    }
    // ── Foreground filter + path trim ─────────────────────────────────────────
    if let Some(spec) = &style.element_filter {
        // A foreground filter samples the pre-transform layout rect, so it does not
        // compose with a paint transform on the same node (the filtered crop would
        // come from the wrong region). Reject the pair rather than emit a misaligned
        // render; apply them on separate nested nodes.
        if style.translate.is_some() || style.rotate.is_some() || style.skew.is_some() {
            errors.push(DescribeError::new(
                format!("{path}/style/element_filter"),
                "element_filter does not compose with translate / rotate / skew on the \
                 same node; apply them on separate nested nodes"
                    .to_string(),
            ));
        } else {
            el = el.element_filter(filter_of(spec));
        }
    }
    if let Some(v) = style.trim
        && finite_num(v, path, "trim", errors)
    {
        el = el.trim(v);
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
    // ── Material vibrancy background (custom-glass fill + its backdrop blur) ────
    if let Some(mat) = &style.material {
        match resolve_color(&mat.tint, theme) {
            Ok(base) => {
                let all_finite = finite_num(mat.fill_alpha, path, "material/fill_alpha", errors)
                    && finite_num(mat.blur, path, "material/blur", errors)
                    && finite_num(mat.saturation, path, "material/saturation", errors);
                if all_finite {
                    let blur = mat.blur.clamp(0.0, MAX_BLUR_PX);
                    let tint = Material::new(mat.fill_alpha, blur, mat.saturation).tint(base);
                    el = el.bg(tint).backdrop_blur(blur);
                }
            }
            Err(e) => errors.push(relocate(e, format!("{path}/style/material/tint"))),
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

#[cfg(test)]
mod cache_tests {
    use super::*;

    /// A 1×1 transparent PNG, base64-encoded — decodes to a 4-byte RGBA8 buffer.
    const TINY_PNG_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

    fn decode_count() -> u64 {
        DECODE_COUNT.with(Cell::get)
    }

    #[test]
    fn same_image_decodes_once_then_reuses_the_cache() {
        // Rebuilding the tree (as every frame / interaction step does) must not
        // re-run the decode: the second and later builds reuse the cached blob.
        clear_image_cache();
        let before = decode_count();
        let json = format!(
            r#"{{"schema":"fenestra/1","root":{{"image":{{"png":"{TINY_PNG_B64}","label":"pixel"}}}}}}"#
        );
        let desc: Description = serde_json::from_str(&json).unwrap();
        let theme = Theme::light();
        for _ in 0..5 {
            let (_, errs) = to_element_lenient(&desc, &theme);
            assert!(errs.is_empty(), "{errs:?}");
        }
        assert_eq!(
            decode_count() - before,
            1,
            "the same image must decode once, then reuse the cache on later builds"
        );
    }
}
