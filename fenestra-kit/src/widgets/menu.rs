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
    Color, Cursor, Element, Key, Overlay, SP1, SP2, SP3, Semantics, Surface, TextSize, Theme,
    Transition, Weight, col, div, row, spacer, text,
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
    submenu: Option<Vec<MenuItem<Msg>>>,
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
        submenu: None,
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
        submenu: None,
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

    /// Nest a flyout submenu under this item: a trailing chevron marks it, and
    /// clicking (or pressing Enter on) the row toggles the submenu to its right.
    /// The item's own `on_select` is ignored — activating it opens the submenu.
    #[must_use]
    pub fn submenu(mut self, items: impl IntoIterator<Item = MenuItem<Msg>>) -> Self {
        self.submenu = Some(items.into_iter().collect());
        self
    }
}

/// Renders one rich menu entry into a row (or a separator rule). `active` draws
/// the keyboard cursor (accent fill); under `roving` the row is not a tab stop
/// (the panel owns focus) and skips its own hover veil.
fn menu_row<Msg: Clone + 'static>(it: MenuItem<Msg>, active: bool, roving: bool) -> Element<Msg> {
    if it.separator {
        return row().py(4.0).children([div()
            .h(1.0)
            .w_full()
            .themed(|t: &Theme, s| s.bg(t.border_subtle))]);
    }
    let disabled = it.disabled;
    let label_text = it.label.clone();
    // Label inks accent on the cursor row, the disabled token when disabled.
    let strong: fn(&Theme) -> Color = move_color(disabled, active, true);
    let muted: fn(&Theme) -> Color = move_color(disabled, active, false);
    let mut kids: Vec<Element<Msg>> = Vec::new();
    if let Some(icon) = it.icon {
        kids.push(
            icon.w(16.0)
                .h(16.0)
                .shrink0()
                .themed(move |t: &Theme, s| s.color(muted(t))),
        );
    }
    kids.push(
        text(it.label)
            .size(TextSize::Sm)
            .themed(move |t: &Theme, s| s.color(strong(t))),
    );
    kids.push(spacer());
    if it.submenu.is_some() {
        // Submenu parents show a trailing right-chevron instead of a shortcut.
        kids.push(
            crate::icons::chevron_right()
                .w(14.0)
                .h(14.0)
                .shrink0()
                .themed(move |t: &Theme, s| s.color(muted(t))),
        );
    } else if let Some(sc) = it.shortcut {
        kids.push(
            text(sc)
                .size(TextSize::Xs)
                .themed(move |t: &Theme, s| s.color(muted(t))),
        );
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
    if active {
        row_el = row_el.themed(|t: &Theme, s| s.bg(t.accent_bg));
    }
    // A submenu parent anchors a click-toggled flyout to its right. It must stay
    // focusable to receive the hit (and Enter) that toggles the flyout; its own
    // on_select is ignored.
    if let Some(sub) = it.submenu {
        return row_el
            .cursor(Cursor::Pointer)
            .focusable(true)
            .transition(Transition::colors())
            .state_layer(|t| t.text)
            .child(Element::from(menu_items(sub)).overlay(Overlay::submenu()));
    }
    if !disabled {
        row_el = row_el
            .cursor(Cursor::Pointer)
            .transition(Transition::colors());
        // Roving menus drive the cursor from the keyboard; mouse hover still
        // works, but the per-item hover veil would fight the accent cursor.
        if !roving {
            row_el = row_el.state_layer(|t| t.text);
        }
        if let Some(msg) = it.on_select {
            row_el = row_el.on_click(msg);
        }
        if roving {
            // The panel is the single focusable; items are not tab stops.
            row_el = row_el.focusable(false);
        }
    }
    row_el
}

/// The text token for a menu row: disabled dims, the cursor row inks accent,
/// else the strong (label) or muted (icon/shortcut) role.
fn move_color(disabled: bool, active: bool, strong: bool) -> fn(&Theme) -> Color {
    match (disabled, active, strong) {
        (true, _, _) => |t| t.text_disabled,
        (false, true, _) => |t| t.accent_text,
        (false, false, true) => |t| t.text,
        (false, false, false) => |t| t.text_muted,
    }
}

/// The next navigable item index from `current`, stepping by `dir` and clamped
/// to the ends (separators and disabled items are not in `navigable`).
fn step_navigable(navigable: &[usize], current: Option<usize>, dir: i32) -> Option<usize> {
    if navigable.is_empty() {
        return None;
    }
    match current.and_then(|c| navigable.iter().position(|&i| i == c)) {
        Some(p) => {
            let np = if dir > 0 {
                (p + 1).min(navigable.len() - 1)
            } else {
                p.saturating_sub(1)
            };
            Some(navigable[np])
        }
        None => Some(if dir > 0 {
            navigable[0]
        } else {
            navigable[navigable.len() - 1]
        }),
    }
}

/// A rich menu under construction (see [`menu_items`]); converts into an
/// [`Element`].
pub struct Menu<Msg> {
    items: Vec<MenuItem<Msg>>,
    highlighted: Option<usize>,
    on_navigate: Option<std::rc::Rc<dyn Fn(usize) -> Msg>>,
    on_close: Option<Msg>,
}

/// A rich menu panel (no overlay attached) built from [`menu_item`]s and
/// [`menu_separator`]s: leading icons, trailing shortcut hints, disabled rows,
/// and separator rules. Attach an overlay (e.g. `.overlay(Overlay::menu())`) to
/// float it. The simpler [`menu`] takes `(label, message)` tuples.
///
/// Wire [`Menu::highlighted`] + [`Menu::on_navigate`] (and [`Menu::on_close`])
/// for APG keyboard navigation: the panel becomes one focusable element, Up/Down
/// and Home/End move the cursor (skipping separators and disabled items),
/// Enter/Space activate the cursor row, and Escape closes.
pub fn menu_items<Msg>(items: impl IntoIterator<Item = MenuItem<Msg>>) -> Menu<Msg> {
    Menu {
        items: items.into_iter().collect(),
        highlighted: None,
        on_navigate: None,
        on_close: None,
    }
}

impl<Msg> Menu<Msg> {
    /// The keyboard-cursor item index (app-owned). Setting it (or
    /// [`Menu::on_navigate`]) switches the panel to single-focusable keyboard
    /// navigation instead of per-item tab stops.
    #[must_use]
    pub fn highlighted(mut self, index: Option<usize>) -> Self {
        self.highlighted = index;
        self
    }

    /// Maps an arrow / Home / End step to a message carrying the new cursor
    /// index (clamped; separators and disabled items are skipped).
    #[must_use]
    pub fn on_navigate(mut self, f: impl Fn(usize) -> Msg + 'static) -> Self {
        self.on_navigate = Some(std::rc::Rc::new(f));
        self
    }

    /// Emitted on Escape (close the menu).
    #[must_use]
    pub fn on_close(mut self, msg: Msg) -> Self {
        self.on_close = Some(msg);
        self
    }
}

impl<Msg: Clone + 'static> From<Menu<Msg>> for Element<Msg> {
    fn from(m: Menu<Msg>) -> Self {
        let keyboard = m.on_navigate.is_some() || m.highlighted.is_some();
        let highlighted = m.highlighted;
        // Navigable indices (skip separators + disabled) and the per-index
        // activation message, captured before the items are consumed into rows.
        let navigable: Vec<usize> = m
            .items
            .iter()
            .enumerate()
            .filter(|(_, it)| !it.separator && !it.disabled)
            .map(|(i, _)| i)
            .collect();
        let activate: Vec<Option<Msg>> = m
            .items
            .iter()
            .map(|it| {
                (!it.separator && !it.disabled)
                    .then(|| it.on_select.clone())
                    .flatten()
            })
            .collect();

        let rows: Vec<Element<Msg>> = m
            .items
            .into_iter()
            .enumerate()
            .map(|(i, it)| menu_row(it, keyboard && highlighted == Some(i), keyboard))
            .collect();

        let mut panel = col()
            .p(SP1)
            .gap(2.0)
            .min_w(200.0)
            .surface(Surface::Menu)
            .children(rows);

        if keyboard {
            let nav = m.on_navigate;
            let close = m.on_close;
            panel = panel.focusable(true).on_key(move |k| match k.key {
                Key::ArrowDown => step_navigable(&navigable, highlighted, 1)
                    .and_then(|i| nav.as_ref().map(|f| f(i))),
                Key::ArrowUp => step_navigable(&navigable, highlighted, -1)
                    .and_then(|i| nav.as_ref().map(|f| f(i))),
                Key::Home => navigable
                    .first()
                    .copied()
                    .and_then(|i| nav.as_ref().map(|f| f(i))),
                Key::End => navigable
                    .last()
                    .copied()
                    .and_then(|i| nav.as_ref().map(|f| f(i))),
                Key::Enter | Key::Space => {
                    highlighted.and_then(|h| activate.get(h).cloned().flatten())
                }
                Key::Escape => close.clone(),
                _ => None,
            });
        }
        panel
    }
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

#[cfg(test)]
mod tests {
    use super::step_navigable;

    #[test]
    fn keyboard_nav_skips_separators_and_disabled_and_clamps() {
        // Items 0 and 1 are actionable, 2 is a separator, 3 actionable, 4
        // disabled — so the navigable indices are [0, 1, 3].
        let nav = [0usize, 1, 3];
        assert_eq!(
            step_navigable(&nav, None, 1),
            Some(0),
            "first on Down from none"
        );
        assert_eq!(
            step_navigable(&nav, None, -1),
            Some(3),
            "last on Up from none"
        );
        // Down from 1 skips the separator at 2 and lands on 3.
        assert_eq!(step_navigable(&nav, Some(1), 1), Some(3));
        // Up from 3 skips the separator and lands on 1.
        assert_eq!(step_navigable(&nav, Some(3), -1), Some(1));
        // Clamp at both ends (never wraps, never lands on disabled 4).
        assert_eq!(step_navigable(&nav, Some(3), 1), Some(3));
        assert_eq!(step_navigable(&nav, Some(0), -1), Some(0));
        // A cursor that isn't navigable resets to an end.
        assert_eq!(step_navigable(&nav, Some(2), 1), Some(0));
        // An empty menu has nowhere to go.
        assert_eq!(step_navigable(&[], Some(0), 1), None);
    }
}
