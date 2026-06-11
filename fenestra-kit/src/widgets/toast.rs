//! Toasts: transient notifications stacked in the top-right corner. The
//! app owns the list (Elm-pure, like the modal); each toast shows a status
//! dot, its message, and a dismiss button. Auto-dismissal is an app
//! concern — send a removal message later through the `App::init` proxy.
//!
//! ```
//! use fenestra_kit::{Status, toast_stack};
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Dismiss(usize),
//! }
//!
//! let el: fenestra_core::Element<Msg> =
//!     toast_stack([("Report saved", Status::Success)])
//!         .on_dismiss(Msg::Dismiss)
//!         .into();
//! ```

use fenestra_core::{
    Cursor, Element, Overlay, R_SM, SP2, SP3, ShadowToken, TextSize, Theme, Transition, col, div,
    row, text,
};

use super::Status;
use crate::icons;

/// A toast stack under construction; converts into an [`Element`].
pub struct ToastStack<Msg> {
    toasts: Vec<(String, Status)>,
    width: f32,
    on_dismiss: Option<std::rc::Rc<dyn Fn(usize) -> Msg>>,
    key: Option<String>,
}

/// A stack of `(message, status)` toasts pinned to the top-right as an
/// overlay. Place it as a child anywhere in the tree (the root works);
/// an empty list renders nothing.
pub fn toast_stack<Msg>(
    toasts: impl IntoIterator<Item = (impl Into<String>, Status)>,
) -> ToastStack<Msg> {
    ToastStack {
        toasts: toasts.into_iter().map(|(m, s)| (m.into(), s)).collect(),
        width: 340.0,
        on_dismiss: None,
        key: None,
    }
}

impl<Msg> ToastStack<Msg> {
    /// Sets the stack width in logical px (340 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Maps a toast's dismiss button to a message carrying its index.
    pub fn on_dismiss(mut self, f: impl Fn(usize) -> Msg + 'static) -> Self {
        self.on_dismiss = Some(std::rc::Rc::new(f));
        self
    }

    /// Stable identity key (recommended when the stack moves around).
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

fn toast_row<Msg: 'static>(
    index: usize,
    message: String,
    status: Status,
    on_dismiss: Option<std::rc::Rc<dyn Fn(usize) -> Msg>>,
) -> Element<Msg> {
    let mut el = row()
        .items_center()
        .gap(SP3)
        .p(SP3)
        .w_full()
        .rounded(R_SM + 2.0)
        .shadow(ShadowToken::Lg)
        .shrink0()
        .themed(|t: &Theme, s| s.bg(t.elevated_surface(2)).border(1.0, t.border_subtle))
        .children([
            div()
                .w(8.0)
                .h(8.0)
                .rounded_full()
                .shrink0()
                .themed(move |t: &Theme, s| s.bg(status.colors(t).solid)),
            text(message).size(TextSize::Sm).grow(),
        ]);
    if let Some(f) = on_dismiss {
        el = el.child(
            div()
                .p(2.0)
                .rounded(R_SM)
                .shrink0()
                .cursor(Cursor::Pointer)
                .on_click(f(index))
                .transition(Transition::colors())
                .hover_themed(|t, s| s.bg(t.neutrals.step(3)))
                .child(icons::x().themed(|t: &Theme, s| s.color(t.text_muted))),
        );
    }
    el
}

impl<Msg: 'static> From<ToastStack<Msg>> for Element<Msg> {
    fn from(t: ToastStack<Msg>) -> Self {
        if t.toasts.is_empty() {
            return col();
        }
        let on_dismiss = t.on_dismiss;
        let mut stack = col()
            .gap(SP2)
            .w(t.width)
            .overlay(Overlay::toasts())
            .id(t.key.as_deref().unwrap_or("toast-stack"));
        stack = stack.children(
            t.toasts
                .into_iter()
                .enumerate()
                .map(|(i, (m, s))| toast_row(i, m, s, on_dismiss.clone())),
        );
        stack
    }
}
