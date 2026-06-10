//! The element IR: a plain tree of boxes and text, with typed styles,
//! interaction variant overlays, and `Msg`-carrying handlers. `view()`
//! rebuilds this tree on every redraw; it must stay cheap to construct.

use peniko::Color;

use crate::style::{Length, Paint, Style, StyleFn, TextAlign, Transition};
use crate::tokens::{ShadowToken, TextSize, Weight};

/// What an element fundamentally is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// A container box.
    Box,
    /// A text run (single style; rich text is out of scope for v1).
    Text(String),
    /// A themed hairline rule (resolved to `border_subtle`).
    Divider,
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
    pub(crate) hover_style: Option<StyleFn>,
    pub(crate) active_style: Option<StyleFn>,
    pub(crate) focus_style: Option<StyleFn>,
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

    /// Appends children.
    pub fn children(mut self, children: impl IntoIterator<Item = Element<Msg>>) -> Self {
        self.children.extend(children);
        self
    }

    /// Appends one child.
    pub fn child(mut self, child: Element<Msg>) -> Self {
        self.children.push(child);
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
        self.hover_style = Some(Box::new(f));
        self
    }

    /// Style overlay applied while pressed.
    pub fn active(mut self, f: impl Fn(Style) -> Style + 'static) -> Self {
        self.active_style = Some(Box::new(f));
        self
    }

    /// Style overlay applied while focused.
    pub fn focus(mut self, f: impl Fn(Style) -> Style + 'static) -> Self {
        self.focus_style = Some(Box::new(f));
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
