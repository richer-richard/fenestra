//! [`toolbar`]: a surface-framed bar that groups action controls (buttons,
//! toggles, selects) with orientation-aware [`Toolbar::separator`] dividers.
//! Horizontal by default; each control keeps its own focus and activation, so a
//! toolbar is just the frame and the grouping.
//!
//! ```
//! use fenestra_kit::{icon_button, toolbar};
//! use fenestra_kit::icons;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Bold,
//!     Italic,
//! }
//!
//! let el: fenestra_core::Element<Msg> = toolbar()
//!     .item(icon_button(icons::check()).label("Bold").on_click(Msg::Bold))
//!     .separator()
//!     .item(icon_button(icons::x()).label("Italic").on_click(Msg::Italic))
//!     .into();
//! ```

use fenestra_core::{Element, SP1, Theme, col, div, row};

/// A toolbar entry: a control or a separator rule. The control is boxed so the
/// separator variant doesn't bloat every slot to an `Element`'s width.
enum ToolItem<Msg> {
    Control(Box<Element<Msg>>),
    Separator,
}

/// A toolbar under construction; converts into an [`Element`].
pub struct Toolbar<Msg> {
    items: Vec<ToolItem<Msg>>,
    vertical: bool,
    label: Option<String>,
}

/// An empty toolbar frame. Chain [`Toolbar::item`] and [`Toolbar::separator`] to
/// fill it; [`Toolbar::vertical`] stands it on end.
pub fn toolbar<Msg>() -> Toolbar<Msg> {
    Toolbar {
        items: Vec::new(),
        vertical: false,
        label: None,
    }
}

impl<Msg> Toolbar<Msg> {
    /// Appends a control (any [`Element`]) to the bar.
    #[must_use]
    pub fn item(mut self, control: impl Into<Element<Msg>>) -> Self {
        self.items.push(ToolItem::Control(Box::new(control.into())));
        self
    }

    /// Appends a separator rule between the surrounding controls. The rule is
    /// drawn across the toolbar's minor axis (a vertical hairline in a
    /// horizontal bar, a horizontal one when [`Toolbar::vertical`]).
    #[must_use]
    pub fn separator(mut self) -> Self {
        self.items.push(ToolItem::Separator);
        self
    }

    /// Stacks the controls vertically instead of in a row.
    #[must_use]
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }

    /// Sets an accessible name for the bar (e.g. "Formatting").
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// The hairline rule between toolbar groups, oriented across the bar's minor axis.
fn rule<Msg>(vertical: bool) -> Element<Msg> {
    let r = div().shrink0().themed(|t: &Theme, s| s.bg(t.border));
    if vertical {
        r.h(1.0).w(20.0)
    } else {
        r.w(1.0).h(20.0)
    }
}

impl<Msg> From<Toolbar<Msg>> for Element<Msg> {
    fn from(tb: Toolbar<Msg>) -> Self {
        let vertical = tb.vertical;
        let kids: Vec<Element<Msg>> = tb
            .items
            .into_iter()
            .map(|item| match item {
                ToolItem::Control(el) => *el,
                ToolItem::Separator => rule(vertical),
            })
            .collect();

        let base = if vertical { col() } else { row() };
        let mut bar = base
            .items_center()
            .gap(SP1)
            .p(4.0)
            .shrink0()
            // Hug the controls rather than stretch across a wider parent.
            .self_start()
            .themed(|t: &Theme, s| {
                s.bg(t.surface_raised)
                    .border(1.0, t.border)
                    .rounded(t.radius.lg)
            })
            .children(kids);
        if let Some(label) = tb.label {
            bar = bar.label(label);
        }
        bar
    }
}
