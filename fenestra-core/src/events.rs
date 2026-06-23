//! Input events and dispatch: hit testing against the laid-out frame,
//! hover/active/focus bookkeeping with active capture, keyboard focus
//! cycling, and message extraction from element handlers.

use std::collections::HashMap;

use kurbo::Point;

use crate::element::{Cursor, Element, Kind};
use crate::frame::Frame;
use crate::frame_state::FrameState;
use crate::id::WidgetId;
use crate::input;
use crate::text::Fonts;

/// A logical keyboard key (expanded for text editing in M5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Enter / Return.
    Enter,
    /// Space bar.
    Space,
    /// Escape.
    Escape,
    /// Left arrow.
    ArrowLeft,
    /// Right arrow.
    ArrowRight,
    /// Up arrow.
    ArrowUp,
    /// Down arrow.
    ArrowDown,
    /// Home.
    Home,
    /// End.
    End,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Page up (scrolls the focused scrollable).
    PageUp,
    /// Page down.
    PageDown,
    /// A printable character.
    Char(char),
}

/// A key press with modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyInput {
    /// The logical key.
    pub key: Key,
    /// Shift held.
    pub shift: bool,
    /// Control held.
    pub ctrl: bool,
    /// Alt/Option held.
    pub alt: bool,
    /// Command (macOS) / Windows key.
    pub meta: bool,
}

impl KeyInput {
    /// A plain, unmodified key press.
    pub fn plain(key: Key) -> Self {
        Self {
            key,
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
        }
    }
}

/// A logical input event, shared by the windowed runner and headless
/// `SyntheticEvent` injection.
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    /// Pointer moved to logical coordinates.
    PointerMove {
        /// Logical x.
        x: f32,
        /// Logical y.
        y: f32,
    },
    /// Primary button pressed.
    PointerDown,
    /// Primary button released.
    PointerUp,
    /// Secondary (right) button pressed: the context-menu gesture.
    RightDown,
    /// Secondary (right) button released.
    RightUp,
    /// An OS file was dropped onto the window (one event per file).
    FileDrop(std::path::PathBuf),
    /// Wheel / trackpad scroll. Winit convention: positive `dy` moves the
    /// content down (scrolling toward the top of the document); positive `dx`
    /// moves the content right.
    Wheel {
        /// Horizontal delta in logical px (0.0 for a pure vertical event).
        dx: f32,
        /// Vertical delta in logical px.
        dy: f32,
    },
    /// Modifier keys changed (runners forward this so pointer gestures
    /// can honor Shift — e.g. shift-click selection extension).
    Modifiers {
        /// Shift held.
        shift: bool,
        /// Control held.
        ctrl: bool,
        /// Alt/Option held.
        alt: bool,
        /// Command / Windows key held.
        meta: bool,
    },
    /// The pointer left the surface: hover state clears.
    PointerLeave,
    /// Focus next.
    Tab,
    /// Focus previous.
    ShiftTab,
    /// A key press.
    Key(KeyInput),
    /// Committed text input (typing or IME commit).
    Text(String),
    /// IME preedit update (composition in progress).
    ImePreedit {
        /// The composition text ("" clears it).
        text: String,
        /// Cursor range within the composition, in bytes.
        cursor: Option<(usize, usize)>,
    },
}

/// The result of dispatching one event.
pub struct Dispatch<Msg> {
    /// Messages emitted by handlers, in order.
    pub msgs: Vec<Msg>,
    /// Whether visual state changed (hover/active/focus/scroll).
    pub redraw: bool,
    /// Cursor to show; `None` leaves the current cursor unchanged
    /// (non-pointer events must not reset it).
    pub cursor: Option<Cursor>,
}

impl<Msg> Default for Dispatch<Msg> {
    fn default() -> Self {
        Self {
            msgs: Vec::new(),
            redraw: false,
            cursor: None,
        }
    }
}

/// A handler entry: borrowed from the declared tree, or an owned,
/// materialized virtual row (addressed by a child path inside it).
enum ElemRef<'a, Msg> {
    Borrowed(&'a Element<Msg>),
    Owned {
        row: std::rc::Rc<Element<Msg>>,
        path: Vec<usize>,
    },
}

impl<Msg> ElemRef<'_, Msg> {
    fn resolve(&self) -> &Element<Msg> {
        match self {
            Self::Borrowed(el) => el,
            Self::Owned { row, path } => {
                let mut el: &Element<Msg> = row;
                for &i in path {
                    el = &el.children[i];
                }
                el
            }
        }
    }
}

/// Index from widget id to the element carrying its handlers, rebuilt per
/// dispatch from the same id derivation the frame build uses. Virtual
/// containers materialize the same row window the frame build used, so
/// handlers on virtual rows resolve like any other element.
struct Handlers<'a, Msg> {
    map: HashMap<WidgetId, ElemRef<'a, Msg>>,
}

impl<'a, Msg> Handlers<'a, Msg> {
    fn collect(root: &'a Element<Msg>, state: &FrameState, viewport: f32) -> Self {
        let mut handlers = Self {
            map: HashMap::new(),
        };
        handlers.walk_borrowed(root, WidgetId::ROOT, state, viewport);
        handlers
    }

    fn walk_borrowed(
        &mut self,
        el: &'a Element<Msg>,
        id: WidgetId,
        state: &FrameState,
        viewport: f32,
    ) {
        self.expand_virtual(el, id, state, viewport);
        self.map.insert(id, ElemRef::Borrowed(el));
        for (i, child) in el.children.iter().enumerate() {
            self.walk_borrowed(child, id.child(i, child.key.as_deref()), state, viewport);
        }
    }

    /// Materializes the same window of rows the frame build produced
    /// (spacer at child index 0; rows are keyed, so the index is moot).
    fn expand_virtual(
        &mut self,
        el: &Element<Msg>,
        id: WidgetId,
        state: &FrameState,
        viewport: f32,
    ) {
        let Some(v) = &el.virtual_rows else {
            return;
        };
        // Variable lists: reuse the exact window the frame materialized
        // (stashed at build), so handler ids line up with painted rows.
        let window = if v.variable {
            state.virtual_windows.get(&id).cloned().unwrap_or(0..0)
        } else {
            crate::frame::virtual_window(v.count, v.row_height, state.scroll_offset(id), viewport)
        };
        for (j, i) in window.enumerate() {
            let row = if v.variable {
                let mut row = (v.builder)(i);
                if row.key.is_none() {
                    row = row.id(&format!("v{i}"));
                }
                std::rc::Rc::new(row.shrink0())
            } else {
                std::rc::Rc::new(crate::frame::materialize_virtual_row(v, i))
            };
            let rid = id.child(1 + j, row.key.as_deref());
            self.walk_owned(&row, Vec::new(), rid, state, viewport);
        }
    }

    fn walk_owned(
        &mut self,
        row: &std::rc::Rc<Element<Msg>>,
        path: Vec<usize>,
        id: WidgetId,
        state: &FrameState,
        viewport: f32,
    ) {
        let el = {
            let mut el: &Element<Msg> = row;
            for &i in &path {
                el = &el.children[i];
            }
            el
        };
        self.expand_virtual(el, id, state, viewport);
        let child_ids: Vec<WidgetId> = el
            .children
            .iter()
            .enumerate()
            .map(|(i, c)| id.child(i, c.key.as_deref()))
            .collect();
        self.map.insert(
            id,
            ElemRef::Owned {
                row: std::rc::Rc::clone(row),
                path: path.clone(),
            },
        );
        for (i, child_id) in child_ids.into_iter().enumerate() {
            let mut child_path = path.clone();
            child_path.push(i);
            self.walk_owned(row, child_path, child_id, state, viewport);
        }
    }

    fn get(&self, id: WidgetId) -> Option<&Element<Msg>> {
        self.map.get(&id).map(ElemRef::resolve)
    }
}

/// Recomputes the hover set and cursor at `point`, emitting hover-enter
/// messages in chain (root-to-deepest) order.
fn update_hover<Msg: Clone>(
    handlers: &Handlers<'_, Msg>,
    frame: &Frame,
    state: &mut FrameState,
    point: Point,
    out: &mut Dispatch<Msg>,
) {
    let chain = frame.hit_chain(point);
    let now = state.now();
    let hovered: HashMap<WidgetId, f64> = chain
        .iter()
        .copied()
        .filter(|id| {
            handlers.get(*id).is_some_and(|el| {
                !el.disabled
                    && (el.hover_style.is_some()
                        || el.state_layer.is_some()
                        || el.on_click.is_some()
                        || el.on_hover.is_some()
                        || frame.toggle_overlay_of(*id).is_some()
                        || has_hover_overlay(el))
            })
        })
        // Elements still hovered keep their original enter time.
        .map(|id| (id, state.hovered.get(&id).copied().unwrap_or(now)))
        .collect();
    // Hover-enter messages, in deterministic root-to-deepest order.
    for id in &chain {
        if hovered.contains_key(id)
            && !state.hovered.contains_key(id)
            && let Some(el) = handlers.get(*id)
            && let Some(msg) = &el.on_hover
        {
            out.msgs.push(msg.clone());
        }
    }
    let changed = hovered.len() != state.hovered.len()
        || hovered.keys().any(|id| !state.hovered.contains_key(id));
    if changed {
        state.hovered = hovered;
        out.redraw = true;
    }
    out.cursor = Some(cursor_of(handlers, &chain));
}

/// Whether any direct child is a hover overlay (tooltip): hovering the
/// anchor must be tracked even without hover styling.
fn has_hover_overlay<Msg>(el: &Element<Msg>) -> bool {
    el.children.iter().any(|c| {
        matches!(
            c.overlay,
            Some(crate::element::Overlay {
                mode: crate::element::OverlayMode::Hover { .. },
                ..
            })
        )
    })
}

/// Recomputes the hover set against a freshly-built frame without emitting
/// messages — used after scrolling moves content under a stationary
/// pointer. Returns `true` when the hover set changed.
pub fn refresh_hover<Msg>(root: &Element<Msg>, frame: &Frame, state: &mut FrameState) -> bool {
    let Some((x, y)) = state.pointer else {
        return false;
    };
    let handlers = Handlers::collect(root, state, frame.canvas_height());
    let chain = frame.hit_chain(Point::new(f64::from(x), f64::from(y)));
    let now = state.now();
    let hovered: HashMap<WidgetId, f64> = chain
        .iter()
        .copied()
        .filter(|id| {
            handlers.get(*id).is_some_and(|el| {
                !el.disabled
                    && (el.hover_style.is_some()
                        || el.state_layer.is_some()
                        || el.on_click.is_some()
                        || el.on_hover.is_some()
                        || frame.toggle_overlay_of(*id).is_some()
                        || has_hover_overlay(el))
            })
        })
        .map(|id| (id, state.hovered.get(&id).copied().unwrap_or(now)))
        .collect();
    let changed = hovered.len() != state.hovered.len()
        || hovered.keys().any(|id| !state.hovered.contains_key(id));
    if changed {
        state.hovered = hovered;
        true
    } else {
        false
    }
}

/// The message an enabled element with the given id would emit when
/// clicked, if any. The shell uses it to honor accessibility action
/// requests (AccessKit `Action::Click`) without synthesizing pointer
/// events.
pub fn click_msg_of<Msg: Clone>(
    root: &Element<Msg>,
    frame: &Frame,
    state: &FrameState,
    id: WidgetId,
) -> Option<Msg> {
    let handlers = Handlers::collect(root, state, frame.canvas_height());
    handlers
        .get(id)
        .filter(|el| !el.disabled)
        .and_then(|el| el.on_click.clone())
}

/// The first element in tree order carrying an OS file-drop handler.
fn first_file_drop<Msg>(el: &Element<Msg>) -> Option<&(dyn Fn(&std::path::Path) -> Msg + '_)> {
    if let Some(f) = &el.on_file_drop {
        return Some(&**f);
    }
    el.children.iter().find_map(first_file_drop)
}

/// Dispatches one event against the last laid-out frame, updating retained
/// interaction state and collecting emitted messages.
pub fn dispatch<Msg: Clone>(
    root: &Element<Msg>,
    frame: &Frame,
    state: &mut FrameState,
    fonts: &mut Fonts,
    event: InputEvent,
) -> Dispatch<Msg> {
    let handlers = Handlers::collect(root, state, frame.canvas_height());
    let mut out = Dispatch::default();

    match event {
        InputEvent::PointerMove { x, y } => {
            let point = Point::new(f64::from(x), f64::from(y));
            state.pointer = Some((x, y));

            if let Some(active) = state.active {
                // Active capture: the pressed element keeps receiving events.
                if let Some(el) = handlers.get(active)
                    && !el.disabled
                {
                    if matches!(el.kind, Kind::Input(_)) {
                        // Drag extends the text selection.
                        let now = state.now();
                        if let Some((lx, ly)) = input_local(frame, state, el, active, point) {
                            if let Some(editor) = state.editors.get_mut(&active) {
                                input::pointer_drag(editor, fonts, lx, ly);
                                editor.last_activity = now;
                            }
                            out.redraw = true;
                        }
                    } else if el.selectable && matches!(el.kind, Kind::Text(_) | Kind::Rich(_)) {
                        if let Some((sid, sel, true)) = state.static_sel
                            && sid == active
                            && let Some((text, style)) = frame.static_text_of(active)
                            && let Some(rect) = frame.rect_of(active)
                        {
                            #[expect(
                                clippy::cast_possible_truncation,
                                reason = "text coords fit in f32"
                            )]
                            let sel = fonts.static_extend(
                                &text,
                                style,
                                Some(rect.width() as f32),
                                sel,
                                (point.x - rect.x0) as f32,
                                (point.y - rect.y0) as f32,
                            );
                            state.static_sel = Some((active, sel, true));
                            out.redraw = true;
                        }
                    } else if let Some(f) = &el.on_drag
                        && let Some((fx, fy)) = frame.fraction_in(active, point)
                        && let Some(msg) = f(fx, fy)
                    {
                        out.msgs.push(msg);
                    }
                }
                out.cursor = Some(cursor_of(&handlers, &[active]));
                return out;
            }

            update_hover(&handlers, frame, state, point, &mut out);
        }
        InputEvent::Modifiers {
            shift,
            ctrl,
            alt,
            meta,
        } => {
            state.mods = (shift, ctrl, alt, meta);
        }
        InputEvent::PointerLeave => {
            state.pointer = None;
            if !state.hovered.is_empty() {
                state.hovered.clear();
                out.redraw = true;
            }
            out.cursor = Some(Cursor::Default);
        }
        InputEvent::PointerDown => {
            // A press with no known pointer position cannot hit anything.
            let Some((px, py)) = state.pointer else {
                return out;
            };
            let point = Point::new(f64::from(px), f64::from(py));
            let chain = frame.hit_chain(point);
            // Internal drag: the deepest drag source under the press.
            state.dragging = chain.iter().rev().find_map(|id| {
                handlers
                    .get(*id)
                    .filter(|el| !el.disabled)
                    .and_then(|el| el.drag_source.clone())
            });

            // Outside-click handling for open overlays: clicking outside an
            // open toggle (menu) closes it and swallows the press; clicking
            // a modal backdrop asks the app to close via on_close.
            let hit_overlay = chain
                .first()
                .and_then(|deepest| frame.overlay_containing(*deepest));
            let mut closed_any = false;
            for (overlay_id, mode) in frame.open_overlays_top_down() {
                if matches!(mode, crate::element::OverlayMode::Toggle)
                    && hit_overlay != Some(overlay_id)
                    && !chain
                        .iter()
                        .any(|id| frame.toggle_overlay_of(*id) == Some(overlay_id))
                {
                    state.close_overlay(overlay_id);
                    closed_any = true;
                }
            }
            if closed_any {
                out.redraw = true;
                out.cursor = Some(cursor_of(&handlers, &chain));
                return out;
            }
            if chain.is_empty()
                && let Some(modal) = frame.top_overlay_is_modal()
            {
                // The backdrop swallowed the press.
                if let Some(el) = handlers.get(modal)
                    && let Some(msg) = &el.on_close
                {
                    out.msgs.push(msg.clone());
                }
                out.redraw = true;
                return out;
            }
            // Deepest interactive node wins the press.
            // A press anywhere ends the previous static selection;
            // selecting below re-establishes one.
            if state.static_sel.take().is_some() {
                out.redraw = true;
            }
            let target = chain.iter().rev().copied().find(|id| {
                handlers.get(*id).is_some_and(|el| {
                    !el.disabled
                        && (el.on_click.is_some()
                            || el.on_drag.is_some()
                            || el.focusable
                            || el.selectable
                            || frame.toggle_overlay_of(*id).is_some())
                })
            });
            if let Some(id) = target {
                state.active = Some(id);
                if let Some(el) = handlers.get(id) {
                    if el.focusable && state.focus != Some(id) {
                        state.focus = Some(id);
                        state.focus_visible = false;
                    } else if el.focusable {
                        state.focus_visible = false;
                    }
                    // Clicking a toggle-overlay anchor opens/closes its menu.
                    if let Some(overlay_id) = frame.toggle_overlay_of(id) {
                        if state.overlay_open(overlay_id) {
                            state.close_overlay(overlay_id);
                        } else {
                            state.open_overlay(overlay_id);
                        }
                    }
                    if matches!(el.kind, Kind::Input(_)) {
                        let now = state.now();
                        // Press chains: 1 = place caret, 2 = word, 3 = line
                        // (platform convention: selection happens on press).
                        let count = match state.last_press {
                            Some((pid, at, c)) if pid == id && now - at <= 0.4 => (c % 3) + 1,
                            _ => 1,
                        };
                        state.last_press = Some((id, now, count));
                        if let Some((lx, ly)) = input_local(frame, state, el, id, point)
                            && let Some(editor) = state.editors.get_mut(&id)
                        {
                            match count {
                                2 => input::select_word_at(editor, fonts, lx, ly),
                                3 => input::select_line_at(editor, fonts, lx, ly),
                                _ => input::pointer_down(editor, fonts, lx, ly, state.mods.0),
                            }
                            editor.last_activity = now;
                        }
                    } else if el.selectable && matches!(el.kind, Kind::Text(_) | Kind::Rich(_)) {
                        let now = state.now();
                        let count = match state.last_press {
                            Some((pid, at, c)) if pid == id && now - at <= 0.4 => (c % 3) + 1,
                            _ => 1,
                        };
                        state.last_press = Some((id, now, count));
                        if let Some((text, style)) = frame.static_text_of(id)
                            && let Some(rect) = frame.rect_of(id)
                        {
                            #[expect(
                                clippy::cast_possible_truncation,
                                reason = "text coords fit in f32"
                            )]
                            let sel = fonts.static_select(
                                &text,
                                style,
                                Some(rect.width() as f32),
                                count,
                                (point.x - rect.x0) as f32,
                                (point.y - rect.y0) as f32,
                            );
                            state.static_sel = Some((id, sel, true));
                        }
                    } else if let Some(f) = &el.on_drag
                        && let Some((fx, fy)) = frame.fraction_in(id, point)
                        && let Some(msg) = f(fx, fy)
                    {
                        out.msgs.push(msg);
                    }
                }
                out.redraw = true;
            } else if state.focus.is_some() {
                // Clicking empty space drops focus.
                state.focus = None;
                state.focus_visible = false;
                out.redraw = true;
            }
            out.cursor = Some(cursor_of(&handlers, &chain));
        }
        InputEvent::FileDrop(path) => {
            // Deepest enabled handler under the pointer, else the first
            // handler in the declared tree (predictable fallback).
            let hit = state.pointer.and_then(|(px, py)| {
                frame
                    .hit_chain(Point::new(f64::from(px), f64::from(py)))
                    .iter()
                    .rev()
                    .find_map(|id| {
                        handlers
                            .get(*id)
                            .filter(|el| !el.disabled && el.on_file_drop.is_some())
                            .map(|_| *id)
                    })
            });
            let msg = match hit {
                Some(id) => handlers
                    .get(id)
                    .and_then(|el| el.on_file_drop.as_ref())
                    .map(|f| f(&path)),
                None => first_file_drop(root).map(|f| f(&path)),
            };
            if let Some(msg) = msg {
                out.msgs.push(msg);
                out.redraw = true;
            }
        }
        InputEvent::RightDown => {
            if let Some((px, py)) = state.pointer {
                let chain = frame.hit_chain(Point::new(f64::from(px), f64::from(py)));
                // Deepest enabled element with a right-click handler wins.
                if let Some(msg) = chain.iter().rev().find_map(|id| {
                    handlers
                        .get(*id)
                        .filter(|el| !el.disabled)
                        .and_then(|el| el.on_right_click.clone())
                }) {
                    out.msgs.push(msg);
                    out.redraw = true;
                }
            }
        }
        InputEvent::RightUp => {}
        InputEvent::PointerUp => {
            if let Some((sid, sel, true)) = state.static_sel {
                state.static_sel = Some((sid, sel, false));
            }
            // Internal drag completion: deliver the payload to the deepest
            // drop target under the release, if any.
            if let Some(payload) = state.dragging.take()
                && let Some((px, py)) = state.pointer
                && let Some(msg) = frame
                    .hit_chain(Point::new(f64::from(px), f64::from(py)))
                    .iter()
                    .rev()
                    .find_map(|id| {
                        handlers
                            .get(*id)
                            .filter(|el| !el.disabled)
                            .and_then(|el| el.on_drop.as_ref())
                            .and_then(|f| f(&payload))
                    })
            {
                out.msgs.push(msg);
                out.redraw = true;
            }
            if let Some(active) = state.active.take() {
                // Click = press + release on the same element.
                if let Some((px, py)) = state.pointer
                    && frame
                        .hit_chain(Point::new(f64::from(px), f64::from(py)))
                        .contains(&active)
                    && let Some(el) = handlers.get(active)
                    && !el.disabled
                {
                    if let Some(msg) = &el.on_click {
                        out.msgs.push(msg.clone());
                        // Menus close when something inside them is chosen.
                        if let Some(overlay_id) = frame.overlay_containing(active) {
                            state.close_overlay(overlay_id);
                        }
                    }
                    // Double click: a second completed click on the same
                    // element within the window. Both singles also fire.
                    let now = state.now();
                    let doubled = state
                        .last_click
                        .is_some_and(|(id, at)| id == active && now - at <= 0.4);
                    if doubled {
                        if let Some(msg) = &el.on_double_click {
                            out.msgs.push(msg.clone());
                        }
                        state.last_click = None;
                    } else {
                        state.last_click = Some((active, now));
                    }
                }
                out.redraw = true;
            }
            // Capture ended: hover reflects whatever is now under the
            // pointer (it was frozen at press-time contents during capture).
            if let Some((x, y)) = state.pointer {
                update_hover(
                    &handlers,
                    frame,
                    state,
                    Point::new(f64::from(x), f64::from(y)),
                    &mut out,
                );
            }
        }
        InputEvent::Wheel { dx, dy } => {
            if let Some((x, y)) = state.pointer {
                let p = Point::new(f64::from(x), f64::from(y));
                // dy and dx route to the nearest scroller on their OWN axis —
                // which may be different containers (e.g. a horizontal pane
                // nested in a vertical one).
                if dy.abs() > 1e-3
                    && let Some(id) = frame.scrollable_y_at(p)
                {
                    state.scroll_by(id, -dy);
                    out.redraw = true;
                }
                if dx.abs() > 1e-3
                    && let Some(id) = frame.scrollable_x_at(p)
                {
                    state.scroll_by_x(id, -dx);
                    out.redraw = true;
                }
            }
        }
        InputEvent::Tab | InputEvent::ShiftTab => {
            let order = frame.focusables();
            if !order.is_empty() {
                let next = match state
                    .focus
                    .and_then(|f| order.iter().position(|id| *id == f))
                {
                    Some(i) if matches!(event, InputEvent::Tab) => order[(i + 1) % order.len()],
                    Some(i) => order[(i + order.len() - 1) % order.len()],
                    None if matches!(event, InputEvent::Tab) => order[0],
                    None => order[order.len() - 1],
                };
                state.focus = Some(next);
                state.focus_visible = true;
                out.redraw = true;
            }
        }
        InputEvent::Key(key) => {
            let mut key_handled = false;
            if let Some(focus) = state.focus
                && let Some(el) = handlers.get(focus)
                && !el.disabled
            {
                // Focused inputs consume editing and navigation keys first.
                if matches!(el.kind, Kind::Input(_)) {
                    let now = state.now();
                    let st = &mut *state;
                    if let Some(editor) = st.editors.get_mut(&focus) {
                        let outcome = input::handle_key(editor, fonts, st.clipboard.as_mut(), &key);
                        editor.last_activity = now;
                        if outcome.changed {
                            if let Some(f) = &el.on_input {
                                out.msgs.push(f(editor.editor.raw_text()));
                            }
                            out.redraw = true;
                            return out;
                        }
                        if outcome.consumed {
                            out.redraw = true;
                            return out;
                        }
                    }
                }
                if let Some(f) = &el.on_key
                    && let Some(msg) = f(&key)
                {
                    out.msgs.push(msg);
                    out.redraw = true;
                    key_handled = true;
                }
                if let Some(f) = &el.on_type_ahead {
                    if matches!(key.key, Key::Escape) {
                        state.type_ahead = None;
                    } else if let Key::Char(c) = key.key
                        && !key.ctrl
                        && !key.meta
                        && !c.is_control()
                    {
                        let now = state.now();
                        let buffer = match state.type_ahead.take() {
                            // Continue the buffer within the window.
                            Some((id, mut b, at)) if id == focus && now - at <= 1.0 => {
                                b.push(c);
                                b
                            }
                            _ => c.to_string(),
                        };
                        if let Some(msg) = f(&buffer) {
                            out.msgs.push(msg);
                            out.redraw = true;
                            key_handled = true;
                        }
                        state.type_ahead = Some((focus, buffer, now));
                    }
                }
                // Enter/Space activate clickables and toggle anchored menus.
                if matches!(key.key, Key::Enter | Key::Space) {
                    if let Some(msg) = &el.on_click {
                        out.msgs.push(msg.clone());
                        out.redraw = true;
                    }
                    if let Some(overlay_id) = frame.toggle_overlay_of(focus) {
                        if state.overlay_open(overlay_id) {
                            state.close_overlay(overlay_id);
                        } else {
                            state.open_overlay(overlay_id);
                        }
                        out.redraw = true;
                    }
                }
            }
            // Cmd/Ctrl+C copies an active static-text selection when no
            // focused editor consumed the chord.
            if !key_handled
                && (key.meta || key.ctrl)
                && matches!(key.key, Key::Char(c) if c.eq_ignore_ascii_case(&'c'))
                && let Some((sid, sel, _)) = state.static_sel
            {
                let range = sel.text_range();
                if range.start < range.end
                    && let Some((text, _)) = frame.static_text_of(sid)
                {
                    let full = text.to_text();
                    if let Some(slice) = full.get(range) {
                        state.clipboard.as_mut().set(slice.to_owned());
                    }
                }
            }

            // Keyboard paging drives the focused element's nearest
            // scrollable (or the first one) unless on_key consumed the key.
            if !key_handled
                && matches!(key.key, Key::PageUp | Key::PageDown | Key::Home | Key::End)
                && let Some((target, rect)) = frame.scroll_target_for(state.focus)
            {
                #[expect(clippy::cast_possible_truncation, reason = "viewports fit in f32")]
                let page = (rect.height() * 0.9) as f32;
                match key.key {
                    Key::PageDown => state.scroll_by(target, page),
                    Key::PageUp => state.scroll_by(target, -page),
                    Key::End => state.scroll_to(target, f32::MAX),
                    Key::Home => state.scroll_to(target, 0.0),
                    _ => {}
                }
                out.redraw = true;
            }
            // Esc closes the top overlay: toggles close directly, app-driven
            // overlays are asked via on_close.
            if matches!(key.key, Key::Escape)
                && let Some((overlay_id, mode)) = frame.open_overlays_top_down().first().copied()
            {
                match mode {
                    crate::element::OverlayMode::Toggle => {
                        state.close_overlay(overlay_id);
                        out.redraw = true;
                    }
                    crate::element::OverlayMode::Open => {
                        if let Some(el) = handlers.get(overlay_id)
                            && let Some(msg) = &el.on_close
                        {
                            out.msgs.push(msg.clone());
                            out.redraw = true;
                        }
                    }
                    crate::element::OverlayMode::Hover { .. } => {}
                }
            }
        }
        InputEvent::Text(text) => {
            if let Some(focus) = state.focus
                && let Some(el) = handlers.get(focus)
                && !el.disabled
            {
                if matches!(el.kind, Kind::Input(_)) {
                    let now = state.now();
                    if let Some(editor) = state.editors.get_mut(&focus) {
                        let outcome = input::handle_text(editor, fonts, &text);
                        editor.last_activity = now;
                        if outcome.changed {
                            if let Some(f) = &el.on_input {
                                out.msgs.push(f(editor.editor.raw_text()));
                            }
                            out.redraw = true;
                        }
                    }
                } else if text == " "
                    && let Some(msg) = &el.on_click
                {
                    // The runner sends printable keys as Text; Space must
                    // still activate focused buttons.
                    out.msgs.push(msg.clone());
                    out.redraw = true;
                }
            }
        }
        InputEvent::ImePreedit { text, cursor } => {
            let now = state.now();
            if let Some(focus) = state.focus
                && let Some(el) = handlers.get(focus)
                && !el.disabled
                && matches!(el.kind, Kind::Input(_))
                && let Some(editor) = state.editors.get_mut(&focus)
            {
                input::handle_preedit(editor, fonts, &text, cursor);
                editor.last_activity = now;
                out.redraw = true;
            }
        }
    }
    out
}

/// The cursor of the deepest element in the chain that sets one.
fn cursor_of<Msg>(handlers: &Handlers<'_, Msg>, chain: &[WidgetId]) -> Cursor {
    chain
        .iter()
        .rev()
        .find_map(|id| {
            handlers.get(*id).and_then(|el| {
                if el.disabled && el.cursor.is_some() {
                    Some(Cursor::NotAllowed)
                } else {
                    el.cursor
                }
            })
        })
        .unwrap_or(Cursor::Default)
}

/// Maps a screen point into editor-layout coordinates for an input element:
/// inside the padding box, with the horizontal follow-scroll applied. The y
/// is the vertical middle (single-line editors clamp to the line anyway).
fn input_local<Msg>(
    frame: &Frame,
    state: &FrameState,
    el: &Element<Msg>,
    id: WidgetId,
    point: Point,
) -> Option<(f64, f64)> {
    let rect = frame.rect_of(id)?;
    let scroll_x = state.editors.get(&id).map_or(0.0, |e| e.scroll_x);
    let pad = f64::from(el.style().padding.left);
    Some((point.x - rect.x0 - pad + scroll_x, rect.height() * 0.5))
}
