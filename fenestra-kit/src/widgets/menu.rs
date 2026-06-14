//! Menus and popovers: floating panels of actions.
//!
//! ```
//! use fenestra_kit::dropdown_menu;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Rename,
//!     Delete,
//! }
//!
//! // As a child of a clickable anchor: opens on anchor click, closes on
//! // item click, outside click, or Escape.
//! let el: fenestra_core::Element<Msg> =
//!     dropdown_menu([("Rename", Msg::Rename), ("Delete", Msg::Delete)]);
//! ```

use fenestra_core::{
    Cursor, Element, Overlay, SP1, SP2, Semantics, Surface, TextSize, Transition, col, row, text,
};

/// The styled panel of menu items (no overlay attached): rows that emit
/// their message on click. Compose with [`dropdown_menu`] /
/// [`context_menu`], or attach your own [`Overlay`].
pub fn menu<Msg: Clone + 'static>(
    items: impl IntoIterator<Item = (impl Into<String>, Msg)>,
) -> Element<Msg> {
    col()
        .p(SP1)
        .gap(2.0)
        .min_w(160.0)
        .surface(Surface::Menu)
        .children(items.into_iter().map(|(label, msg)| {
            let label = label.into();
            row()
                .items_center()
                .px(SP2)
                .h(30.0)
                // Concentric with the menu panel: the item radius is the
                // panel's outer radius minus its SP1 padding, so the rounded
                // item nests inside the rounded panel without bulging.
                .rounded(Surface::Menu.bundle().radius.inner(SP1))
                .shrink0()
                .cursor(Cursor::Pointer)
                .on_click(msg)
                .semantics(Semantics::Button)
                .label(label.clone())
                .transition(Transition::colors())
                .state_layer(|t| t.text)
                .children([text(label).size(TextSize::Sm)])
        }))
}

/// A dropdown menu: place it as a child of a clickable anchor — clicking
/// the anchor toggles it, and item clicks, outside clicks, and Escape
/// close it.
pub fn dropdown_menu<Msg: Clone + 'static>(
    items: impl IntoIterator<Item = (impl Into<String>, Msg)>,
) -> Element<Msg> {
    menu(items).overlay(Overlay::menu())
}

/// A context menu pinned at the right-click position. App-driven: pair
/// the target's `.on_right_click(open_msg)` with an app-owned flag that
/// mounts this as a child, give the menu `.on_close(close_msg)` (outside
/// click / Escape), and close on item messages in `update`.
pub fn context_menu<Msg: Clone + 'static>(
    items: impl IntoIterator<Item = (impl Into<String>, Msg)>,
) -> Element<Msg> {
    menu(items).overlay(Overlay::context())
}

/// A general floating panel anchored below its parent: elevated surface,
/// border, shadow, padding. Defaults to click-to-toggle ([`Overlay::menu`]);
/// override with `.overlay(..)` for app-driven popovers.
pub fn popover<Msg: 'static>(content: impl Into<Element<Msg>>) -> Element<Msg> {
    col()
        .p(SP2)
        .surface(Surface::Popover)
        .child(content)
        .overlay(Overlay::menu())
}
