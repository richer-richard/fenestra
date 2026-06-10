//! Input events and dispatch: hit testing against the laid-out frame,
//! hover/active/focus bookkeeping with active capture, keyboard focus
//! cycling, and message extraction from element handlers.

use std::collections::{HashMap, HashSet};

use kurbo::Point;

use crate::element::{Cursor, Element};
use crate::frame::Frame;
use crate::frame_state::FrameState;
use crate::id::WidgetId;

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
    /// Wheel / trackpad scroll. Winit convention: positive `dy` moves the
    /// content down (scrolling toward the top of the document).
    Wheel {
        /// Vertical delta in logical px.
        dy: f32,
    },
    /// The pointer left the surface: hover state clears.
    PointerLeave,
    /// Focus next.
    Tab,
    /// Focus previous.
    ShiftTab,
    /// A key press.
    Key(KeyInput),
    /// Committed text input (M5).
    Text(String),
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

/// Index from widget id to the element carrying its handlers, rebuilt per
/// dispatch from the same id derivation the frame build uses.
struct Handlers<'a, Msg> {
    map: HashMap<WidgetId, &'a Element<Msg>>,
}

impl<'a, Msg> Handlers<'a, Msg> {
    fn collect(root: &'a Element<Msg>) -> Self {
        fn walk<'a, Msg>(
            el: &'a Element<Msg>,
            id: WidgetId,
            map: &mut HashMap<WidgetId, &'a Element<Msg>>,
        ) {
            map.insert(id, el);
            for (i, child) in el.children.iter().enumerate() {
                walk(child, id.child(i, child.key.as_deref()), map);
            }
        }
        let mut map = HashMap::new();
        walk(root, WidgetId::ROOT, &mut map);
        Self { map }
    }

    fn get(&self, id: WidgetId) -> Option<&'a Element<Msg>> {
        self.map.get(&id).copied()
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
    let hovered: HashSet<WidgetId> = chain
        .iter()
        .copied()
        .filter(|id| {
            handlers.get(*id).is_some_and(|el| {
                !el.disabled
                    && (el.hover_style.is_some() || el.on_click.is_some() || el.on_hover.is_some())
            })
        })
        .collect();
    // Hover-enter messages, in deterministic root-to-deepest order.
    for id in &chain {
        if hovered.contains(id)
            && !state.hovered.contains(id)
            && let Some(el) = handlers.get(*id)
            && let Some(msg) = &el.on_hover
        {
            out.msgs.push(msg.clone());
        }
    }
    if hovered != state.hovered {
        state.hovered = hovered;
        out.redraw = true;
    }
    out.cursor = Some(cursor_of(handlers, &chain));
}

/// Recomputes the hover set against a freshly-built frame without emitting
/// messages — used after scrolling moves content under a stationary
/// pointer. Returns `true` when the hover set changed.
pub fn refresh_hover<Msg>(root: &Element<Msg>, frame: &Frame, state: &mut FrameState) -> bool {
    let Some((x, y)) = state.pointer else {
        return false;
    };
    let handlers = Handlers::collect(root);
    let chain = frame.hit_chain(Point::new(f64::from(x), f64::from(y)));
    let hovered: HashSet<WidgetId> = chain
        .iter()
        .copied()
        .filter(|id| {
            handlers.get(*id).is_some_and(|el| {
                !el.disabled
                    && (el.hover_style.is_some() || el.on_click.is_some() || el.on_hover.is_some())
            })
        })
        .collect();
    if hovered != state.hovered {
        state.hovered = hovered;
        true
    } else {
        false
    }
}

/// Dispatches one event against the last laid-out frame, updating retained
/// interaction state and collecting emitted messages.
pub fn dispatch<Msg: Clone>(
    root: &Element<Msg>,
    frame: &Frame,
    state: &mut FrameState,
    event: InputEvent,
) -> Dispatch<Msg> {
    let handlers = Handlers::collect(root);
    let mut out = Dispatch::default();

    match event {
        InputEvent::PointerMove { x, y } => {
            let point = Point::new(f64::from(x), f64::from(y));
            state.pointer = Some((x, y));

            if let Some(active) = state.active {
                // Active capture: the pressed element keeps receiving events.
                if let Some(el) = handlers.get(active)
                    && !el.disabled
                    && let Some(f) = &el.on_drag
                    && let Some((fx, fy)) = frame.fraction_in(active, point)
                    && let Some(msg) = f(fx, fy)
                {
                    out.msgs.push(msg);
                }
                out.cursor = Some(cursor_of(&handlers, &[active]));
                return out;
            }

            update_hover(&handlers, frame, state, point, &mut out);
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
            // Deepest interactive node wins the press.
            let target = chain.iter().rev().copied().find(|id| {
                handlers.get(*id).is_some_and(|el| {
                    !el.disabled && (el.on_click.is_some() || el.on_drag.is_some() || el.focusable)
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
                    if let Some(f) = &el.on_drag
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
        InputEvent::PointerUp => {
            if let Some(active) = state.active.take() {
                // Click = press + release on the same element.
                if let Some((px, py)) = state.pointer
                    && frame
                        .hit_chain(Point::new(f64::from(px), f64::from(py)))
                        .contains(&active)
                    && let Some(el) = handlers.get(active)
                    && !el.disabled
                    && let Some(msg) = &el.on_click
                {
                    out.msgs.push(msg.clone());
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
        InputEvent::Wheel { dy } => {
            if let Some((x, y)) = state.pointer
                && let Some(id) = frame.scrollable_at(Point::new(f64::from(x), f64::from(y)))
            {
                state.scroll_by(id, -dy);
                out.redraw = true;
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
            if let Some(focus) = state.focus
                && let Some(el) = handlers.get(focus)
                && !el.disabled
            {
                if let Some(f) = &el.on_key
                    && let Some(msg) = f(&key)
                {
                    out.msgs.push(msg);
                    out.redraw = true;
                }
                // Enter/Space activate clickables.
                if matches!(key.key, Key::Enter | Key::Space)
                    && let Some(msg) = &el.on_click
                {
                    out.msgs.push(msg.clone());
                    out.redraw = true;
                }
            }
        }
        InputEvent::Text(_) => {
            // Text input lands in M5.
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
