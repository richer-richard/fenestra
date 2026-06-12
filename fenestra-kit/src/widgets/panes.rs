//! Split panes: two children separated by a draggable divider; the app
//! owns the split fraction (Elm-pure).

use fenestra_core::{Cursor, Element, Length, Theme, col, div, row};

/// A split pane under construction; converts into an [`Element`].
pub struct SplitPane<Msg> {
    fraction: f32,
    first: Element<Msg>,
    second: Element<Msg>,
    vertical: bool,
    on_resize: Option<std::rc::Rc<dyn Fn(f32) -> Msg>>,
    key: Option<String>,
}

/// Two panes split at `fraction` (0..1 of the container) with a
/// draggable divider. The app owns the fraction: wire `.on_resize` and
/// store the new value. Horizontal (side by side) by default.
pub fn split_pane<Msg>(
    fraction: f32,
    first: impl Into<Element<Msg>>,
    second: impl Into<Element<Msg>>,
) -> SplitPane<Msg> {
    SplitPane {
        fraction: fraction.clamp(0.05, 0.95),
        first: first.into(),
        second: second.into(),
        vertical: false,
        on_resize: None,
        key: None,
    }
}

impl<Msg> SplitPane<Msg> {
    /// Stacks the panes vertically (divider drags up/down).
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }

    /// Maps a dragged fraction (0..1 of the container, already clamped)
    /// to a message. Dragging anywhere that no inner widget captures
    /// resizes; interactive content inside the panes wins hit-testing.
    pub fn on_resize(mut self, f: impl Fn(f32) -> Msg + 'static) -> Self {
        self.on_resize = Some(std::rc::Rc::new(f));
        self
    }

    /// Stable identity key.
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

impl<Msg: Clone + 'static> From<SplitPane<Msg>> for Element<Msg> {
    fn from(p: SplitPane<Msg>) -> Self {
        let vertical = p.vertical;
        let pct = p.fraction * 100.0;
        let divider = if vertical {
            div()
                .w_full()
                .h(6.0)
                .shrink0()
                .cursor(Cursor::Pointer)
                .themed(|t: &Theme, s| s.bg(t.border_subtle))
        } else {
            div()
                .w(6.0)
                .h_full()
                .shrink0()
                .cursor(Cursor::Pointer)
                .themed(|t: &Theme, s| s.bg(t.border_subtle))
        };
        let (first, second) = (
            if vertical {
                col().w_full().h(Length::Pct(pct)).overflow_hidden()
            } else {
                col().h_full().w(Length::Pct(pct)).overflow_hidden()
            }
            .child(p.first),
            col().grow().overflow_hidden().child(p.second),
        );
        let mut container = if vertical { col() } else { row() }
            .w_full()
            .h_full()
            .children((first, divider, second));
        if let Some(f) = p.on_resize {
            container = container.on_drag(move |fx, fy| {
                let raw = if vertical { fy } else { fx };
                Some(f(raw.clamp(0.05, 0.95)))
            });
        }
        if let Some(key) = &p.key {
            container = container.id(key);
        }
        container
    }
}
