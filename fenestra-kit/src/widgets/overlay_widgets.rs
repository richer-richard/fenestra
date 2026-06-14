//! Tooltip and Modal: overlay widgets on the core overlay stack.

use fenestra_core::{
    Element, Overlay, SP2, SP3, SP6, Semantics, Surface, TextSize, Theme, Weight, col, row, text,
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
