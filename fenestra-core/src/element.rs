//! The element IR: a plain tree of boxes and text, with typed styles,
//! interaction variant overlays, and `Msg`-carrying handlers. `view()`
//! rebuilds this tree on every redraw; it must stay cheap to construct.

use peniko::Color;

use crate::events::KeyInput;
use crate::style::{Length, Paint, Style, TextAlign, ThemedFn, Transition};
use crate::theme::Theme;
use crate::tokens::{ShadowToken, TextSize, Weight};

/// Maps a key press on a focused element to an optional message.
pub(crate) type KeyFn<Msg> = Box<dyn Fn(&KeyInput) -> Option<Msg>>;
/// Maps a pointer position (as fractions of the element rect) to a message.
pub(crate) type DragFn<Msg> = Box<dyn Fn(f32, f32) -> Option<Msg>>;
/// Maps the edited text to a message.
pub(crate) type InputFn<Msg> = Box<dyn Fn(&str) -> Msg>;

/// What an element fundamentally is.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// A container box.
    Box,
    /// A text run (single style; rich text is out of scope for v1).
    Text(String),
    /// A themed hairline rule (resolved to `border_subtle`).
    Divider,
    /// A vector path (icons, check marks), scaled from its viewbox to the
    /// element rect and painted in the resolved text color.
    Path(PathData),
    /// A single-line editable text field driven by parley's `PlainEditor`.
    /// The app's value is the source of truth; edits emit `on_input`.
    Input(InputData),
}

/// Payload for [`Kind::Input`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputData {
    /// The current value (app state).
    pub value: String,
    /// Placeholder shown when the value is empty.
    pub placeholder: String,
}

/// Path payload for [`Kind::Path`].
#[derive(Debug, Clone, PartialEq)]
pub struct PathData {
    /// The path, in viewbox coordinates.
    pub path: std::sync::Arc<kurbo::BezPath>,
    /// Design-space size the path was drawn in.
    pub viewbox: (f64, f64),
    /// Stroke width in viewbox units; `None` fills the path instead.
    pub stroke: Option<f64>,
}

/// How an overlay child opens and closes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlayMode {
    /// Open while present in the tree; the app controls it (modals).
    /// Outside clicks and Esc emit the overlay's `on_close` message.
    Open,
    /// Clicking the anchor (parent) toggles it; outside clicks and Esc
    /// close it, and a click on any clickable inside closes it (menus).
    Toggle,
    /// Shows after hovering the anchor for the delay; never hit-tested.
    Hover {
        /// Hover delay in milliseconds.
        delay_ms: f32,
    },
}

/// Where an overlay is positioned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlayPlacement {
    /// Below the anchor, left edges aligned, with a vertical gap. Flips
    /// above when there is no room below.
    Below {
        /// Gap in logical px.
        gap: f32,
    },
    /// Below the anchor, horizontally centered on it.
    BelowCenter {
        /// Gap in logical px.
        gap: f32,
    },
    /// Centered in the canvas.
    Center,
}

/// Marks an element as an overlay child of its parent (the anchor):
/// excluded from normal layout, laid out against the canvas, painted after
/// the root, and hit-tested first.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Overlay {
    /// Open/close behavior.
    pub mode: OverlayMode,
    /// Positioning relative to the anchor or canvas.
    pub placement: OverlayPlacement,
    /// Dim everything beneath with black at 0.4 alpha.
    pub backdrop: bool,
    /// Tab cycles only inside this overlay while it is open.
    pub trap_focus: bool,
}

impl Overlay {
    /// A click-toggled menu below its anchor (select listboxes).
    pub fn menu() -> Self {
        Self {
            mode: OverlayMode::Toggle,
            placement: OverlayPlacement::Below { gap: 4.0 },
            backdrop: false,
            trap_focus: false,
        }
    }

    /// A hover tooltip: 400ms delay, 6px below, centered, untouchable.
    pub fn tooltip() -> Self {
        Self {
            mode: OverlayMode::Hover { delay_ms: 400.0 },
            placement: OverlayPlacement::BelowCenter { gap: 6.0 },
            backdrop: false,
            trap_focus: false,
        }
    }

    /// An app-driven centered modal with backdrop and focus trap.
    pub fn modal() -> Self {
        Self {
            mode: OverlayMode::Open,
            placement: OverlayPlacement::Center,
            backdrop: true,
            trap_focus: true,
        }
    }
}

/// Mouse cursor shown while hovering an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cursor {
    /// The platform default arrow.
    Default,
    /// Pointing hand (buttons, links).
    Pointer,
    /// Text I-beam.
    Text,
    /// Action not available.
    NotAllowed,
}

/// One node in the view tree. `Msg` is the app's message type; handlers
/// carry `Msg` values, not closures over state.
pub struct Element<Msg> {
    pub(crate) kind: Kind,
    pub(crate) style: Style,
    pub(crate) children: Vec<Element<Msg>>,
    /// User key for stable identity (`.id()`).
    pub(crate) key: Option<String>,
    /// Forces children of a z-stack into the same grid cell.
    pub(crate) stack: bool,
    pub(crate) focusable: bool,
    pub(crate) cursor: Option<Cursor>,
    pub(crate) disabled: bool,
    pub(crate) on_click: Option<Msg>,
    pub(crate) on_hover: Option<Msg>,
    pub(crate) on_key: Option<KeyFn<Msg>>,
    pub(crate) on_drag: Option<DragFn<Msg>>,
    pub(crate) on_input: Option<InputFn<Msg>>,
    pub(crate) on_close: Option<Msg>,
    pub(crate) overlay: Option<Overlay>,
    /// Continuous rotation period in ms (spinners); paint-time, clock-driven.
    pub(crate) spin: Option<f32>,
    pub(crate) themed: Option<ThemedFn>,
    pub(crate) hover_style: Option<ThemedFn>,
    pub(crate) active_style: Option<ThemedFn>,
    pub(crate) focus_style: Option<ThemedFn>,
    pub(crate) transition: Option<Transition>,
}

impl<Msg> Element<Msg> {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            style: Style::default(),
            children: Vec::new(),
            key: None,
            stack: false,
            focusable: false,
            cursor: None,
            disabled: false,
            on_click: None,
            on_hover: None,
            on_key: None,
            on_drag: None,
            on_input: None,
            on_close: None,
            overlay: None,
            spin: None,
            themed: None,
            hover_style: None,
            active_style: None,
            focus_style: None,
            transition: None,
        }
    }

    /// The element's style (read access for tests and tooling).
    pub fn style(&self) -> &Style {
        &self.style
    }

    /// Appends children. Anything convertible to an element works, so kit
    /// widget builders drop in next to `text()`/`div()` trees.
    pub fn children<T: Into<Element<Msg>>>(
        mut self,
        children: impl IntoIterator<Item = T>,
    ) -> Self {
        self.children.extend(children.into_iter().map(Into::into));
        self
    }

    /// Appends one child.
    pub fn child(mut self, child: impl Into<Element<Msg>>) -> Self {
        self.children.push(child.into());
        self
    }

    /// Sets a user key for stable identity across reorders.
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }

    /// Emits this message when the element is clicked.
    pub fn on_click(mut self, msg: Msg) -> Self {
        self.on_click = Some(msg);
        self.focusable = true;
        if self.cursor.is_none() {
            self.cursor = Some(Cursor::Pointer);
        }
        self
    }

    /// Style overlay applied while hovered.
    pub fn hover(mut self, f: impl Fn(Style) -> Style + 'static) -> Self {
        self.hover_style = Some(Box::new(move |_, s| f(s)));
        self
    }

    /// Theme-aware hover overlay (used by kit widgets, which have no theme
    /// in scope at build time).
    pub fn hover_themed(mut self, f: impl Fn(&Theme, Style) -> Style + 'static) -> Self {
        self.hover_style = Some(Box::new(f));
        self
    }

    /// Style overlay applied while pressed.
    pub fn active(mut self, f: impl Fn(Style) -> Style + 'static) -> Self {
        self.active_style = Some(Box::new(move |_, s| f(s)));
        self
    }

    /// Theme-aware active overlay.
    pub fn active_themed(mut self, f: impl Fn(&Theme, Style) -> Style + 'static) -> Self {
        self.active_style = Some(Box::new(f));
        self
    }

    /// Style overlay applied while focused.
    pub fn focus(mut self, f: impl Fn(Style) -> Style + 'static) -> Self {
        self.focus_style = Some(Box::new(move |_, s| f(s)));
        self
    }

    /// Theme-aware focus overlay.
    pub fn focus_themed(mut self, f: impl Fn(&Theme, Style) -> Style + 'static) -> Self {
        self.focus_style = Some(Box::new(f));
        self
    }

    /// Theme-deferred base styling, applied during style resolution. This is
    /// how kit widgets route every color through tokens without a theme in
    /// scope: `view()` has no theme parameter.
    pub fn themed(mut self, f: impl Fn(&Theme, Style) -> Style + 'static) -> Self {
        self.themed = Some(match self.themed.take() {
            Some(prev) => Box::new(move |t, s| f(t, prev(t, s))),
            None => Box::new(f),
        });
        self
    }

    /// Emits this message when the pointer enters the element.
    pub fn on_hover(mut self, msg: Msg) -> Self {
        self.on_hover = Some(msg);
        self
    }

    /// Maps key presses (while focused) to messages.
    pub fn on_key(mut self, f: impl Fn(&KeyInput) -> Option<Msg> + 'static) -> Self {
        self.on_key = Some(Box::new(f));
        self.focusable = true;
        self
    }

    /// Maps pointer presses and captured drags to messages. The callback
    /// receives the pointer position as fractions (0..=1) of the element
    /// rect on both axes.
    pub fn on_drag(mut self, f: impl Fn(f32, f32) -> Option<Msg> + 'static) -> Self {
        self.on_drag = Some(Box::new(f));
        self
    }

    /// Maps each text edit of an input element to a message.
    pub fn on_input(mut self, f: impl Fn(&str) -> Msg + 'static) -> Self {
        self.on_input = Some(Box::new(f));
        self
    }

    /// Marks this element as an overlay child of its parent (the anchor).
    pub fn overlay(mut self, overlay: Overlay) -> Self {
        self.overlay = Some(overlay);
        self
    }

    /// Emitted when an app-driven overlay is dismissed (outside click, Esc).
    pub fn on_close(mut self, msg: Msg) -> Self {
        self.on_close = Some(msg);
        self
    }

    /// Rotates the element's painted content continuously (one turn per
    /// `period_ms`). Only path elements rotate; used by spinners.
    pub fn spin(mut self, period_ms: f32) -> Self {
        self.spin = Some(period_ms);
        self
    }

    /// Disables interaction: handlers and variants stop applying.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Declares which properties animate between style states.
    pub fn transition(mut self, t: Transition) -> Self {
        self.transition = Some(t);
        self
    }

    /// Marks the element keyboard-focusable.
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    /// Sets the hover cursor.
    pub fn cursor(mut self, cursor: Cursor) -> Self {
        self.cursor = Some(cursor);
        self
    }

    // ----- style delegation: every fluent `Style` method, on the element -----

    /// Padding on all edges.
    pub fn p(mut self, v: f32) -> Self {
        self.style = self.style.p(v);
        self
    }

    /// Horizontal padding.
    pub fn px(mut self, v: f32) -> Self {
        self.style = self.style.px(v);
        self
    }

    /// Vertical padding.
    pub fn py(mut self, v: f32) -> Self {
        self.style = self.style.py(v);
        self
    }

    /// Top padding.
    pub fn pt(mut self, v: f32) -> Self {
        self.style = self.style.pt(v);
        self
    }

    /// Right padding.
    pub fn pr(mut self, v: f32) -> Self {
        self.style = self.style.pr(v);
        self
    }

    /// Bottom padding.
    pub fn pb(mut self, v: f32) -> Self {
        self.style = self.style.pb(v);
        self
    }

    /// Left padding.
    pub fn pl(mut self, v: f32) -> Self {
        self.style = self.style.pl(v);
        self
    }

    /// Margin on all edges.
    pub fn m(mut self, v: f32) -> Self {
        self.style = self.style.m(v);
        self
    }

    /// Horizontal margin.
    pub fn mx(mut self, v: f32) -> Self {
        self.style = self.style.mx(v);
        self
    }

    /// Vertical margin.
    pub fn my(mut self, v: f32) -> Self {
        self.style = self.style.my(v);
        self
    }

    /// Top margin.
    pub fn mt(mut self, v: f32) -> Self {
        self.style = self.style.mt(v);
        self
    }

    /// Right margin.
    pub fn mr(mut self, v: f32) -> Self {
        self.style = self.style.mr(v);
        self
    }

    /// Bottom margin.
    pub fn mb(mut self, v: f32) -> Self {
        self.style = self.style.mb(v);
        self
    }

    /// Left margin.
    pub fn ml(mut self, v: f32) -> Self {
        self.style = self.style.ml(v);
        self
    }

    /// Gap between children.
    pub fn gap(mut self, v: f32) -> Self {
        self.style = self.style.gap(v);
        self
    }

    /// Preferred width (raw `f32` = logical px).
    pub fn w(mut self, v: impl Into<Length>) -> Self {
        self.style = self.style.w(v);
        self
    }

    /// Preferred height.
    pub fn h(mut self, v: impl Into<Length>) -> Self {
        self.style = self.style.h(v);
        self
    }

    /// Minimum width.
    pub fn min_w(mut self, v: impl Into<Length>) -> Self {
        self.style = self.style.min_w(v);
        self
    }

    /// Maximum width.
    pub fn max_w(mut self, v: impl Into<Length>) -> Self {
        self.style = self.style.max_w(v);
        self
    }

    /// Minimum height.
    pub fn min_h(mut self, v: impl Into<Length>) -> Self {
        self.style = self.style.min_h(v);
        self
    }

    /// Maximum height.
    pub fn max_h(mut self, v: impl Into<Length>) -> Self {
        self.style = self.style.max_h(v);
        self
    }

    /// Width 100%.
    pub fn w_full(mut self) -> Self {
        self.style = self.style.w_full();
        self
    }

    /// Height 100%.
    pub fn h_full(mut self) -> Self {
        self.style = self.style.h_full();
        self
    }

    /// Flex grow 1.
    pub fn grow(mut self) -> Self {
        self.style = self.style.grow();
        self
    }

    /// Flex shrink 0.
    pub fn shrink0(mut self) -> Self {
        self.style = self.style.shrink0();
        self
    }

    /// Align children to the cross-axis start.
    pub fn items_start(mut self) -> Self {
        self.style = self.style.items_start();
        self
    }

    /// Center children on the cross axis.
    pub fn items_center(mut self) -> Self {
        self.style = self.style.items_center();
        self
    }

    /// Align children to the cross-axis end.
    pub fn items_end(mut self) -> Self {
        self.style = self.style.items_end();
        self
    }

    /// Align children on their first text baseline (rows only).
    pub fn items_baseline(mut self) -> Self {
        self.style = self.style.items_baseline();
        self
    }

    /// Pack children toward the main-axis start.
    pub fn justify_start(mut self) -> Self {
        self.style = self.style.justify_start();
        self
    }

    /// Center children on the main axis.
    pub fn justify_center(mut self) -> Self {
        self.style = self.style.justify_center();
        self
    }

    /// Pack children toward the main-axis end.
    pub fn justify_end(mut self) -> Self {
        self.style = self.style.justify_end();
        self
    }

    /// Distribute children with space between.
    pub fn justify_between(mut self) -> Self {
        self.style = self.style.justify_between();
        self
    }

    /// Allow flex children to wrap.
    pub fn wrap(mut self) -> Self {
        self.style = self.style.wrap();
        self
    }

    /// Position absolutely against the nearest relative ancestor.
    pub fn absolute(mut self) -> Self {
        self.style = self.style.absolute();
        self
    }

    /// Offset from the top.
    pub fn top(mut self, v: f32) -> Self {
        self.style = self.style.top(v);
        self
    }

    /// Offset from the right.
    pub fn right(mut self, v: f32) -> Self {
        self.style = self.style.right(v);
        self
    }

    /// Offset from the bottom.
    pub fn bottom(mut self, v: f32) -> Self {
        self.style = self.style.bottom(v);
        self
    }

    /// Offset from the left.
    pub fn left(mut self, v: f32) -> Self {
        self.style = self.style.left(v);
        self
    }

    /// Clip children to the bounds.
    pub fn overflow_hidden(mut self) -> Self {
        self.style = self.style.overflow_hidden();
        self
    }

    /// Vertical scrolling with clipped content.
    pub fn scroll_y(mut self) -> Self {
        self.style = self.style.scroll_y();
        self
    }

    /// Background fill.
    pub fn bg(mut self, paint: impl Into<Paint>) -> Self {
        self.style = self.style.bg(paint);
        self
    }

    /// Uniform border.
    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.style = self.style.border(width, color);
        self
    }

    /// The same corner radius on all corners.
    pub fn rounded(mut self, r: f32) -> Self {
        self.style = self.style.rounded(r);
        self
    }

    /// Fully-rounded corners.
    pub fn rounded_full(mut self) -> Self {
        self.style = self.style.rounded_full();
        self
    }

    /// A themed shadow elevation token.
    pub fn shadow(mut self, token: ShadowToken) -> Self {
        self.style = self.style.shadow(token);
        self
    }

    /// Subtree opacity.
    pub fn opacity(mut self, v: f32) -> Self {
        self.style = self.style.opacity(v);
        self
    }

    /// Draw progress for path elements (0 = nothing, 1 = full path).
    pub fn trim(mut self, v: f32) -> Self {
        self.style = self.style.trim(v);
        self
    }

    /// Text size.
    pub fn size(mut self, size: TextSize) -> Self {
        self.style = self.style.size(size);
        self
    }

    /// Font weight.
    pub fn weight(mut self, weight: Weight) -> Self {
        self.style = self.style.weight(weight);
        self
    }

    /// Text color.
    pub fn color(mut self, color: Color) -> Self {
        self.style = self.style.color(color);
        self
    }

    /// Mono family role.
    pub fn mono(mut self) -> Self {
        self.style = self.style.mono();
        self
    }

    /// Truncate to one line with an ellipsis.
    pub fn truncate(mut self) -> Self {
        self.style = self.style.truncate();
        self
    }

    /// Horizontal text alignment.
    pub fn text_align(mut self, align: TextAlign) -> Self {
        self.style = self.style.text_align(align);
        self
    }
}

/// A plain container box (flex row by default, like taffy).
pub fn div<Msg>() -> Element<Msg> {
    Element::new(Kind::Box)
}

/// A flex row.
pub fn row<Msg>() -> Element<Msg> {
    Element::new(Kind::Box)
}

/// A flex column.
pub fn col<Msg>() -> Element<Msg> {
    let mut el = Element::new(Kind::Box);
    el.style.direction = crate::style::Direction::Column;
    el
}

/// A z-stack: children occupy the same rect and paint in order.
pub fn stack<Msg>() -> Element<Msg> {
    let mut el = Element::new(Kind::Box);
    el.style.display = crate::style::Display::Grid;
    el.stack = true;
    el
}

/// A text run.
pub fn text<Msg>(content: impl Into<String>) -> Element<Msg> {
    Element::new(Kind::Text(content.into()))
}

/// Flexible empty space (flex grow 1).
pub fn spacer<Msg>() -> Element<Msg> {
    Element::new(Kind::Box).grow()
}

/// A horizontal hairline rule in `border_subtle`, 1px tall and full width.
/// For a vertical rule, override with `.w(1.0)` and `.h_full()`.
pub fn divider<Msg>() -> Element<Msg> {
    Element::new(Kind::Divider).w_full().h(1.0).shrink0()
}

/// A bare single-line text input leaf. Most apps want the styled
/// `fenestra_kit` `text_input` instead; this is the primitive it wraps.
/// Focusable, shows the text I-beam, and emits `on_input` per edit.
pub fn raw_input<Msg>(value: impl Into<String>, placeholder: impl Into<String>) -> Element<Msg> {
    Element::new(Kind::Input(InputData {
        value: value.into(),
        placeholder: placeholder.into(),
    }))
    .focusable(true)
    .cursor(Cursor::Text)
}

/// A vector path drawn in `viewbox` coordinates and scaled to the element
/// rect (sized to the viewbox by default). `stroke` is a width in viewbox
/// units; `None` fills instead. Painted in the resolved text color.
pub fn path<Msg>(bez: kurbo::BezPath, viewbox: (f64, f64), stroke: Option<f64>) -> Element<Msg> {
    #[expect(clippy::cast_possible_truncation, reason = "viewbox sizes are small")]
    Element::new(Kind::Path(PathData {
        path: std::sync::Arc::new(bez),
        viewbox,
        stroke,
    }))
    .w(viewbox.0 as f32)
    .h(viewbox.1 as f32)
    .shrink0()
}

impl<Msg: 'static> Element<Msg> {
    /// Converts every message this subtree can emit with `f`, so a
    /// component written around its own message type drops into any parent:
    /// the Elm composition tool.
    ///
    /// ```
    /// use fenestra_core::{Element, div, text};
    ///
    /// #[derive(Clone)]
    /// enum CardMsg { Open }
    /// #[derive(Clone)]
    /// enum AppMsg { Card(usize, CardMsg) }
    ///
    /// fn card() -> Element<CardMsg> {
    ///     div().on_click(CardMsg::Open).child(text("open"))
    /// }
    ///
    /// let el: Element<AppMsg> = card().map(|m| AppMsg::Card(0, m));
    /// ```
    pub fn map<B: 'static>(self, f: impl Fn(Msg) -> B + Clone + 'static) -> Element<B> {
        Element {
            kind: self.kind,
            style: self.style,
            children: self
                .children
                .into_iter()
                .map(|c| c.map(f.clone()))
                .collect(),
            key: self.key,
            stack: self.stack,
            focusable: self.focusable,
            cursor: self.cursor,
            disabled: self.disabled,
            on_click: self.on_click.map(&f),
            on_hover: self.on_hover.map(&f),
            on_key: self.on_key.map(|k| {
                let f = f.clone();
                Box::new(move |key: &KeyInput| k(key).map(&f)) as KeyFn<B>
            }),
            on_drag: self.on_drag.map(|d| {
                let f = f.clone();
                Box::new(move |x: f32, y: f32| d(x, y).map(&f)) as DragFn<B>
            }),
            on_input: self.on_input.map(|i| {
                let f = f.clone();
                Box::new(move |s: &str| f(i(s))) as InputFn<B>
            }),
            on_close: self.on_close.map(&f),
            overlay: self.overlay,
            spin: self.spin,
            themed: self.themed,
            hover_style: self.hover_style,
            active_style: self.active_style,
            focus_style: self.focus_style,
            transition: self.transition,
        }
    }
}

impl<Msg> Element<Msg> {
    /// Grid template columns (switches display to grid).
    pub fn grid_cols(mut self, tracks: impl IntoIterator<Item = crate::style::Track>) -> Self {
        self.style = self.style.grid_cols(tracks);
        self
    }

    /// Grid template rows (switches display to grid).
    pub fn grid_rows(mut self, tracks: impl IntoIterator<Item = crate::style::Track>) -> Self {
        self.style = self.style.grid_rows(tracks);
        self
    }

    /// Places this element at a 1-based grid column, spanning `span` tracks.
    pub fn grid_col(mut self, start: i16, span: u16) -> Self {
        self.style = self.style.grid_col(start, span);
        self
    }

    /// Places this element at a 1-based grid row, spanning `span` tracks.
    pub fn grid_row(mut self, start: i16, span: u16) -> Self {
        self.style = self.style.grid_row(start, span);
        self
    }
}
