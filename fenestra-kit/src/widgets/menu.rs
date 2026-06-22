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
    Cursor, Element, Overlay, SP1, SP2, SP3, Semantics, Surface, TextSize, Theme, Transition,
    Weight, col, div, row, spacer, text,
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
                .themed(|t: &Theme, s| s.rounded((t.radius.lg - SP1).max(0.0)))
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

/// One entry of a rich [`menu_items`] panel: an action (with an optional
/// leading icon, trailing shortcut hint, and disabled state) or a separator.
pub struct MenuItem<Msg> {
    label: String,
    icon: Option<Element<Msg>>,
    shortcut: Option<String>,
    disabled: bool,
    separator: bool,
    on_select: Option<Msg>,
}

/// An actionable menu entry with the given label.
pub fn menu_item<Msg>(label: impl Into<String>) -> MenuItem<Msg> {
    MenuItem {
        label: label.into(),
        icon: None,
        shortcut: None,
        disabled: false,
        separator: false,
        on_select: None,
    }
}

/// A horizontal separator rule between menu groups.
pub fn menu_separator<Msg>() -> MenuItem<Msg> {
    MenuItem {
        label: String::new(),
        icon: None,
        shortcut: None,
        disabled: false,
        separator: true,
        on_select: None,
    }
}

impl<Msg> MenuItem<Msg> {
    /// A leading icon, inked to the menu's muted role.
    #[must_use]
    pub fn icon(mut self, icon: impl Into<Element<Msg>>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// A trailing keyboard-shortcut hint (e.g. `"⌘K"`), shown muted.
    #[must_use]
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Dim and disable the item: not activatable and skipped from focus.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The message emitted when the item is chosen.
    #[must_use]
    pub fn on_select(mut self, msg: Msg) -> Self {
        self.on_select = Some(msg);
        self
    }
}

/// Renders one rich menu entry into a row (or a separator rule).
fn menu_row<Msg: Clone + 'static>(it: MenuItem<Msg>) -> Element<Msg> {
    if it.separator {
        return row().py(4.0).children([div()
            .h(1.0)
            .w_full()
            .themed(|t: &Theme, s| s.bg(t.border_subtle))]);
    }
    let disabled = it.disabled;
    let label_text = it.label.clone();
    let mut kids: Vec<Element<Msg>> = Vec::new();
    if let Some(icon) = it.icon {
        kids.push(icon.w(16.0).h(16.0).shrink0().themed(move |t: &Theme, s| {
            s.color(if disabled {
                t.text_disabled
            } else {
                t.text_muted
            })
        }));
    }
    kids.push(
        text(it.label)
            .size(TextSize::Sm)
            .themed(move |t: &Theme, s| s.color(if disabled { t.text_disabled } else { t.text })),
    );
    kids.push(spacer());
    if let Some(sc) = it.shortcut {
        kids.push(text(sc).size(TextSize::Xs).themed(move |t: &Theme, s| {
            s.color(if disabled {
                t.text_disabled
            } else {
                t.text_muted
            })
        }));
    }
    let mut row_el = row()
        .items_center()
        .gap(SP2)
        .px(SP2)
        .h(30.0)
        .themed(|t: &Theme, s| s.rounded((t.radius.lg - SP1).max(0.0)))
        .shrink0()
        .semantics(Semantics::Button)
        .label(label_text)
        .children(kids);
    if !disabled {
        row_el = row_el
            .cursor(Cursor::Pointer)
            .transition(Transition::colors())
            .state_layer(|t| t.text);
        if let Some(msg) = it.on_select {
            row_el = row_el.on_click(msg);
        }
    }
    row_el
}

/// A rich menu panel (no overlay attached) built from [`menu_item`]s and
/// [`menu_separator`]s: leading icons, trailing shortcut hints, disabled rows,
/// and separator rules. Attach an overlay (e.g. `.overlay(Overlay::menu())`) to
/// float it. The simpler [`menu`] takes `(label, message)` tuples.
pub fn menu_items<Msg: Clone + 'static>(
    items: impl IntoIterator<Item = MenuItem<Msg>>,
) -> Element<Msg> {
    col()
        .p(SP1)
        .gap(2.0)
        .min_w(200.0)
        .surface(Surface::Menu)
        .children(items.into_iter().map(menu_row))
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

/// A menu bar under construction; converts into an [`Element`].
pub struct Menubar<Msg> {
    triggers: Vec<Element<Msg>>,
}

/// An application menu bar: a full-width strip of top-level triggers, each of
/// which toggles its own [`dropdown_menu`] on click. Chain [`Menubar::menu`]
/// once per top-level menu.
///
/// ```
/// use fenestra_kit::menubar;
///
/// #[derive(Clone)]
/// enum Msg {
///     New,
///     Open,
///     Undo,
/// }
///
/// let el: fenestra_core::Element<Msg> = menubar()
///     .menu("File", [("New", Msg::New), ("Open", Msg::Open)])
///     .menu("Edit", [("Undo", Msg::Undo)])
///     .into();
/// ```
pub fn menubar<Msg>() -> Menubar<Msg> {
    Menubar {
        triggers: Vec::new(),
    }
}

impl<Msg: Clone + 'static> Menubar<Msg> {
    /// Appends a top-level menu: a labelled trigger that toggles a dropdown of
    /// `(label, message)` items on click.
    #[must_use]
    pub fn menu(
        mut self,
        title: impl Into<String>,
        items: impl IntoIterator<Item = (impl Into<String>, Msg)>,
    ) -> Self {
        let title = title.into();
        let trigger = row()
            .items_center()
            .px(SP3)
            .h(32.0)
            .shrink0()
            .themed(|t: &Theme, s| s.rounded(t.radius.md))
            .transition(Transition::colors())
            .state_layer(|t| t.text)
            .focusable(true)
            .cursor(Cursor::Pointer)
            .semantics(Semantics::Button)
            .label(title.clone())
            .children([text(title)
                .size(TextSize::Sm)
                .weight(Weight::Medium)
                .themed(|t: &Theme, s| s.color(t.text))])
            // The dropdown anchors to (and toggles from) this trigger.
            .child(dropdown_menu(items));
        self.triggers.push(trigger);
        self
    }
}

impl<Msg> From<Menubar<Msg>> for Element<Msg> {
    fn from(mb: Menubar<Msg>) -> Self {
        row()
            .items_center()
            .gap(2.0)
            .px(SP2)
            .h(40.0)
            .w_full()
            .themed(|t: &Theme, s| s.bg(t.surface_raised).border_bottom(1.0, t.border))
            .children(mb.triggers)
    }
}
