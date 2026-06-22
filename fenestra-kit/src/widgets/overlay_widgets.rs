//! Tooltip and Modal: overlay widgets on the core overlay stack.

use fenestra_core::{
    DrawerSide, Element, Overlay, SP2, SP3, SP4, SP6, Semantics, Surface, TextSize, Theme, Weight,
    col, row, spacer, text,
};

use super::button::{ButtonVariant, icon_button};
use crate::icons;

/// Wraps `target` so a small inverted-color tooltip appears 6px below it
/// after a 400ms hover.
///
/// ```
/// use fenestra_kit::{button, tooltip};
///
/// #[derive(Clone)]
/// enum Msg {
///     Save,
/// }
///
/// let el: fenestra_core::Element<Msg> =
///     tooltip(button("Save").on_click(Msg::Save), "Save the document");
/// ```
pub fn tooltip<Msg>(target: impl Into<Element<Msg>>, label: impl Into<String>) -> Element<Msg> {
    let bubble = row()
        .overlay(Overlay::tooltip())
        .px(SP2)
        .py(4.0)
        .surface(Surface::Tooltip)
        .children([text(label)
            .size(TextSize::Sm)
            .themed(|t: &Theme, s| s.color(t.neutrals.step(1)))]);
    // The overlay child anchors to its parent; wrap targets that already
    // have children of their own.
    target.into().child(bubble)
}

/// A modal dialog under construction; converts into an [`Element`].
pub struct Modal<Msg> {
    title: String,
    content: Vec<Element<Msg>>,
    max_width: f32,
    on_close: Option<Msg>,
    key: Option<String>,
}

/// A centered modal with backdrop, focus trap, and enter animation. Render
/// it only while open (it is app-driven); `on_close` fires on Esc, outside
/// click, and the corner close button.
///
/// ```
/// use fenestra_core::text;
/// use fenestra_kit::modal;
///
/// #[derive(Clone)]
/// enum Msg {
///     Dismiss,
/// }
///
/// let el: fenestra_core::Element<Msg> = fenestra_core::div().children([
///     modal("Confirm")
///         .child(text("Are you sure?"))
///         .on_close(Msg::Dismiss),
/// ]);
/// ```
pub fn modal<Msg>(title: impl Into<String>) -> Modal<Msg> {
    Modal {
        title: title.into(),
        content: Vec::new(),
        max_width: 480.0,
        on_close: None,
        key: None,
    }
}

impl<Msg> Modal<Msg> {
    /// Appends content below the title.
    pub fn child(mut self, child: impl Into<Element<Msg>>) -> Self {
        self.content.push(child.into());
        self
    }

    /// Overrides the default 480px max width.
    pub fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = max_width;
        self
    }

    /// Emitted on Esc, outside click, and the close button.
    pub fn on_close(mut self, msg: Msg) -> Self {
        self.on_close = Some(msg);
        self
    }

    /// Stable identity key.
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

impl<Msg: Clone + 'static> From<Modal<Msg>> for Element<Msg> {
    fn from(m: Modal<Msg>) -> Self {
        let title = m.title.clone();
        let mut header = row()
            .items_center()
            .children([text(m.title).size(TextSize::Lg).weight(Weight::Semibold)]);
        if let Some(close) = &m.on_close {
            header = header.justify_between().children([Element::from(
                icon_button(icons::x())
                    .variant(ButtonVariant::Ghost)
                    .size(super::ControlSize::Sm)
                    .label("Close")
                    .on_click(close.clone()),
            )]);
        }

        let mut el = col()
            .overlay(Overlay::modal())
            .w(m.max_width)
            .p(SP6)
            .gap(SP3)
            .surface(Surface::Modal)
            .children([header])
            .children(m.content)
            .semantics(Semantics::Dialog)
            .label(title);
        if let Some(key) = &m.key {
            el = el.id(key);
        }
        if let Some(msg) = m.on_close {
            el = el.on_close(msg);
        }
        el
    }
}

/// A drawer / sheet under construction; converts into an [`Element`].
pub struct Drawer<Msg> {
    title: Option<String>,
    content: Vec<Element<Msg>>,
    side: DrawerSide,
    size: f32,
    on_close: Option<Msg>,
    key: Option<String>,
}

/// An edge-anchored panel (a left/right side drawer or a top/bottom sheet) on
/// the overlay stack, with a backdrop, focus trap, and slide-in. It fills the
/// anchored edge's full span and is flat against that edge (rounded only on the
/// inner corners). Render it only while open (app-driven); `on_close` fires on
/// Esc and an outside (scrim) click. `size` is the panel's thickness — width for
/// left/right, height for top/bottom.
///
/// ```
/// use fenestra_core::{DrawerSide, text};
/// use fenestra_kit::drawer;
///
/// #[derive(Clone)]
/// enum Msg {
///     Dismiss,
/// }
///
/// let el: fenestra_core::Element<Msg> = fenestra_core::div().child(
///     drawer(DrawerSide::Left)
///         .title("Filters")
///         .child(text("Body"))
///         .on_close(Msg::Dismiss),
/// );
/// ```
pub fn drawer<Msg>(side: DrawerSide) -> Drawer<Msg> {
    Drawer {
        title: None,
        content: Vec::new(),
        side,
        size: 320.0,
        on_close: None,
        key: None,
    }
}

impl<Msg> Drawer<Msg> {
    /// Sets a heading shown at the top of the panel (paired with a close button
    /// when `on_close` is set).
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Appends content below the title.
    pub fn child(mut self, child: impl Into<Element<Msg>>) -> Self {
        self.content.push(child.into());
        self
    }

    /// Sets the panel thickness: width for left/right drawers, height for
    /// top/bottom sheets (default 320px).
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Emitted on Esc, outside (scrim) click, and the close button.
    pub fn on_close(mut self, msg: Msg) -> Self {
        self.on_close = Some(msg);
        self
    }

    /// Stable identity key.
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

impl<Msg: Clone + 'static> From<Drawer<Msg>> for Element<Msg> {
    fn from(d: Drawer<Msg>) -> Self {
        let side = d.side;
        let title = d.title.clone();

        let mut kids: Vec<Element<Msg>> = Vec::new();
        if d.title.is_some() || d.on_close.is_some() {
            let heading: Element<Msg> = match &d.title {
                Some(t) => text(t.clone())
                    .size(TextSize::Lg)
                    .weight(Weight::Semibold)
                    .themed(|t: &Theme, s| s.color(t.text)),
                None => spacer(),
            };
            let mut header = row().items_center().justify_between().children([heading]);
            if let Some(close) = &d.on_close {
                header = header.children([Element::from(
                    icon_button(icons::x())
                        .variant(ButtonVariant::Ghost)
                        .size(super::ControlSize::Sm)
                        .label("Close")
                        .on_click(close.clone()),
                )]);
            }
            kids.push(header);
        }
        kids.extend(d.content);

        // Fill the anchored edge: full height for side drawers, full width for
        // top/bottom sheets; the other axis is the requested thickness.
        let panel = col().gap(SP4).p(SP6).children(kids);
        let panel = match side {
            DrawerSide::Left | DrawerSide::Right => panel.w(d.size).h_full(),
            DrawerSide::Top | DrawerSide::Bottom => panel.h(d.size).w_full(),
        };

        let mut el = panel
            .overlay(Overlay::drawer(side))
            .surface(Surface::Modal)
            // Flatten the corners flush to the anchored edge (keep the inner
            // ones rounded from the surface recipe).
            .themed(move |_t: &Theme, s| match side {
                DrawerSide::Left => s.rounded_l(0.0),
                DrawerSide::Right => s.rounded_r(0.0),
                DrawerSide::Top => s.rounded_t(0.0),
                DrawerSide::Bottom => s.rounded_b(0.0),
            })
            .semantics(Semantics::Dialog);
        if let Some(title) = title {
            el = el.label(title);
        }
        if let Some(key) = &d.key {
            el = el.id(key);
        }
        if let Some(msg) = d.on_close {
            el = el.on_close(msg);
        }
        el
    }
}
