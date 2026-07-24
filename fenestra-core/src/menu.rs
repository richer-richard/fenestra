//! Declarative native menus: [`App::menu`](crate::App::menu) describes the
//! menu bar from state, the runner reconciles it after every update —
//! exactly like [`App::windows`](crate::App::windows) — and chosen items
//! come back as ordinary messages.
//!
//! Platform reality (recorded honestly): the native menu bar attaches on
//! macOS. Windows would require `unsafe` HWND plumbing (the workspace
//! forbids `unsafe`; revisit deliberately), and Linux menu bars are a GTK
//! concept a winit window cannot host — on both, the kit's in-window
//! `menubar` widget is the answer, and the runner simply ignores this
//! spec.

/// The menu bar: top-level menus after the platform's app menu (which the
/// runner provides, with Quit).
#[derive(Debug, Clone)]
pub struct MenuSpec<Msg> {
    /// The menus, in order.
    pub menus: Vec<MenuDesc<Msg>>,
}

/// One top-level menu.
#[derive(Debug, Clone)]
pub struct MenuDesc<Msg> {
    /// The menu title.
    pub title: String,
    /// The entries, in order.
    pub items: Vec<MenuItemDesc<Msg>>,
}

/// One menu entry.
#[derive(Debug, Clone)]
pub enum MenuItemDesc<Msg> {
    /// A choosable item emitting `msg`.
    Item {
        /// The item title.
        title: String,
        /// The message chosen items emit.
        msg: Msg,
        /// Accelerator in muda's grammar, e.g. `"CmdOrCtrl+S"`,
        /// `"Shift+CmdOrCtrl+Z"`. Unparseable accelerators drop with a
        /// stderr note; the item still works by mouse. Note that on macOS
        /// a menu accelerator handles the chord *before* the window sees
        /// it — that is the point for app commands, but do not shadow
        /// text-editing chords the focused widget needs.
        accelerator: Option<String>,
        /// Greyed out when false.
        enabled: bool,
    },
    /// A separator line.
    Separator,
    /// A nested submenu.
    Submenu {
        /// The submenu title.
        title: String,
        /// The nested entries.
        items: Vec<MenuItemDesc<Msg>>,
    },
}

impl<Msg> MenuSpec<Msg> {
    /// A menu bar from `(title, items)` pairs.
    #[must_use]
    pub fn new(menus: impl IntoIterator<Item = MenuDesc<Msg>>) -> Self {
        Self {
            menus: menus.into_iter().collect(),
        }
    }

    /// A structural fingerprint (titles, separators, enabled flags,
    /// accelerators — everything but the messages): the runner rebuilds
    /// the native menu only when this changes.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        fn item<Msg>(out: &mut String, it: &MenuItemDesc<Msg>) {
            match it {
                MenuItemDesc::Item {
                    title,
                    accelerator,
                    enabled,
                    ..
                } => {
                    out.push_str("i:");
                    out.push_str(title);
                    out.push('\u{1}');
                    if let Some(a) = accelerator {
                        out.push_str(a);
                    }
                    out.push(if *enabled { '\u{2}' } else { '\u{3}' });
                }
                MenuItemDesc::Separator => out.push_str("s;"),
                MenuItemDesc::Submenu { title, items } => {
                    out.push_str("m:");
                    out.push_str(title);
                    out.push('[');
                    for it in items {
                        item(out, it);
                    }
                    out.push(']');
                }
            }
        }
        let mut out = String::new();
        for menu in &self.menus {
            out.push_str("M:");
            out.push_str(&menu.title);
            out.push('[');
            for it in &menu.items {
                item(&mut out, it);
            }
            out.push(']');
        }
        out
    }
}

impl<Msg> MenuDesc<Msg> {
    /// A menu from a title and items.
    #[must_use]
    pub fn new(
        title: impl Into<String>,
        items: impl IntoIterator<Item = MenuItemDesc<Msg>>,
    ) -> Self {
        Self {
            title: title.into(),
            items: items.into_iter().collect(),
        }
    }
}

impl<Msg> MenuItemDesc<Msg> {
    /// A choosable item.
    #[must_use]
    pub fn item(title: impl Into<String>, msg: Msg) -> Self {
        Self::Item {
            title: title.into(),
            msg,
            accelerator: None,
            enabled: true,
        }
    }

    /// Adds an accelerator (muda grammar, e.g. `"CmdOrCtrl+S"`).
    #[must_use]
    pub fn accelerator(self, accel: impl Into<String>) -> Self {
        match self {
            Self::Item {
                title,
                msg,
                enabled,
                ..
            } => Self::Item {
                title,
                msg,
                accelerator: Some(accel.into()),
                enabled,
            },
            other => other,
        }
    }

    /// Greys the item out.
    #[must_use]
    pub fn disabled(self) -> Self {
        match self {
            Self::Item {
                title,
                msg,
                accelerator,
                ..
            } => Self::Item {
                title,
                msg,
                accelerator,
                enabled: false,
            },
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(enabled: bool) -> MenuSpec<u32> {
        MenuSpec::new([MenuDesc::new(
            "File",
            [
                MenuItemDesc::item("Open", 1).accelerator("CmdOrCtrl+O"),
                MenuItemDesc::Separator,
                MenuItemDesc::Submenu {
                    title: "Recent".into(),
                    items: vec![if enabled {
                        MenuItemDesc::item("a.txt", 2)
                    } else {
                        MenuItemDesc::item("a.txt", 2).disabled()
                    }],
                },
            ],
        )])
    }

    #[test]
    fn fingerprint_ignores_messages_but_sees_structure() {
        assert_eq!(spec(true).fingerprint(), spec(true).fingerprint());
        assert_ne!(
            spec(true).fingerprint(),
            spec(false).fingerprint(),
            "enabled flags are structural"
        );
    }
}
