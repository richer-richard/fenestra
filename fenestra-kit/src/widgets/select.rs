//! Select: a Secondary-styled trigger with a chevron and a toggle-overlay
//! listbox. Keyboard: arrows step the value, Enter/Space toggles the menu,
//! and typing a letter jumps to the first matching option.
//!
//! ```
//! use fenestra_kit::select;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Pick(usize),
//! }
//!
//! let el: fenestra_core::Element<Msg> =
//!     select(0, ["Daily", "Weekly", "Monthly"]).on_change(Msg::Pick).into();
//! ```

use fenestra_core::{
    Cursor, Element, Key, Overlay, R_MD, SP1, SP2, SP3, Semantics, Surface, Theme, Transition, col,
    row, spacer, text,
};

use super::ControlSize;
use crate::icons;

/// A select under construction; converts into an [`Element`].
pub struct Select<Msg> {
    selected: usize,
    options: Vec<String>,
    size: ControlSize,
    width: f32,
    disabled: bool,
    on_change: Option<std::rc::Rc<dyn Fn(usize) -> Msg>>,
    key: Option<String>,
}

/// A select over `options` showing the `selected` index.
pub fn select<Msg>(
    selected: usize,
    options: impl IntoIterator<Item = impl Into<String>>,
) -> Select<Msg> {
    Select {
        selected,
        options: options.into_iter().map(Into::into).collect(),
        size: ControlSize::default(),
        width: 200.0,
        disabled: false,
        on_change: None,
        key: None,
    }
}

impl<Msg> Select<Msg> {
    /// Sets the control size.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Sets the trigger width in logical px (200 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Disables interaction.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Maps a newly selected option index to a message.
    pub fn on_change(mut self, f: impl Fn(usize) -> Msg + 'static) -> Self {
        self.on_change = Some(std::rc::Rc::new(f));
        self
    }

    /// Stable identity key (recommended: the open state is kept per id).
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

/// Maximum listbox height before it scrolls.
const MAX_MENU_HEIGHT: f32 = 240.0;

impl<Msg: 'static> From<Select<Msg>> for Element<Msg> {
    fn from(sel: Select<Msg>) -> Self {
        let selected = sel.selected.min(sel.options.len().saturating_sub(1));
        let label = sel.options.get(selected).cloned().unwrap_or_default();

        // The listbox: options on a raised surface, selected one tinted.
        let listbox = col()
            .id("listbox")
            .overlay(Overlay::menu())
            .scroll_y()
            .max_h(MAX_MENU_HEIGHT)
            .w(sel.width)
            .p(SP1)
            .gap(2.0)
            .surface(Surface::Menu)
            .children(sel.options.iter().enumerate().map(|(i, opt)| {
                let is_selected = i == selected;
                let mut option = row()
                    .items_center()
                    .px(SP2)
                    .h(30.0)
                    // Concentric with the listbox panel (Surface::Menu): the
                    // option radius is the panel outer radius minus its SP1
                    // padding, so options nest cleanly inside the panel.
                    .rounded(Surface::Menu.bundle().radius.inner(SP1))
                    .shrink0()
                    .cursor(Cursor::Pointer)
                    .children([text(opt.clone()).size(sel.size.text_size()).themed(
                        move |t: &Theme, s| {
                            if is_selected {
                                s.color(t.accent_text)
                            } else {
                                s.color(t.text)
                            }
                        },
                    )])
                    .transition(Transition::colors())
                    .state_layer(|t| t.text);
                if is_selected {
                    option = option.themed(|t: &Theme, s| s.bg(t.accent_bg));
                }
                if let Some(f) = &sel.on_change {
                    let f = f.clone();
                    option = option.on_click(f(i));
                }
                option
            }));

        let chevron = icons::chevron_down().themed(|t: &Theme, s| s.color(t.text_muted));

        let mut trigger = row()
            .items_center()
            .gap(SP2)
            .w(sel.width)
            .h(sel.size.height())
            .px(SP3)
            .rounded(R_MD)
            .shrink0()
            .focusable(true)
            .cursor(Cursor::Pointer)
            .disabled(sel.disabled)
            .transition(Transition::colors())
            .themed(|t: &Theme, s| s.bg(t.surface_raised).border(1.0, t.border))
            .semantics(Semantics::ComboBox)
            .label(label.clone())
            .children([text(label).size(sel.size.text_size())])
            .children([spacer(), chevron])
            .children([listbox]);

        if let Some(f) = sel.on_change {
            let options: Vec<String> = sel.options.clone();
            let count = options.len();
            let nav = std::rc::Rc::new(f);
            let pick = std::rc::Rc::clone(&nav);
            trigger = trigger.on_key(move |k| match k.key {
                Key::ArrowDown => (selected + 1 < count).then(|| nav(selected + 1)),
                Key::ArrowUp => (selected > 0).then(|| nav(selected - 1)),
                Key::Home => (count > 0).then(|| nav(0)),
                Key::End => count.checked_sub(1).map(|i| nav(i)),
                _ => None,
            });
            // Type-ahead, both idioms: a single letter cycles through
            // entries with that initial (excluding the current one, so
            // repeats advance), while a growing buffer prefix-matches
            // from the current selection inclusive — "ce" finds Cedar
            // without bouncing off Cherry.
            trigger = trigger.on_type_ahead(move |buffer| {
                let needle = buffer.to_lowercase();
                let start = if needle.chars().count() == 1 { 1 } else { 0 };
                (start..start + count)
                    .map(|step| (selected + step) % count)
                    .find(|i| options[*i].to_lowercase().starts_with(&needle))
                    .map(|i| pick(i))
            });
        }
        if sel.disabled {
            // Disabled keeps the simple subtree dim; the state layer (which
            // would otherwise also fade the container) is for the live trigger.
            trigger = trigger.opacity(0.5);
        } else {
            trigger = trigger.state_layer(|t| t.text);
        }
        if let Some(key) = &sel.key {
            trigger = trigger.id(key);
        }
        trigger
    }
}
