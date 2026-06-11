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
    Cursor, Element, Key, Overlay, R_MD, SP2, SP3, Semantics, ShadowToken, Theme, Transition, col,
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
            .p(4.0)
            .gap(2.0)
            .rounded(R_MD)
            .shadow(ShadowToken::Lg)
            .themed(|t: &Theme, s| s.bg(t.elevated_surface(2)).border(1.0, t.border_subtle))
            .children(sel.options.iter().enumerate().map(|(i, opt)| {
                let is_selected = i == selected;
                let mut option = row()
                    .items_center()
                    .px(SP2)
                    .h(30.0)
                    .rounded(R_MD - 4.0)
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
                    .hover_themed(|t, s| s.bg(t.neutrals.step(3)));
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
            .hover_themed(|t, s| s.bg(t.neutrals.step(3)))
            .semantics(Semantics::ComboBox)
            .label(label.clone())
            .children([text(label).size(sel.size.text_size())])
            .children([spacer(), chevron])
            .children([listbox]);

        if let Some(f) = sel.on_change {
            let options: Vec<String> = sel.options.clone();
            let count = options.len();
            trigger = trigger.on_key(move |k| match k.key {
                Key::ArrowDown => (selected + 1 < count).then(|| f(selected + 1)),
                Key::ArrowUp => (selected > 0).then(|| f(selected - 1)),
                Key::Home => (count > 0).then(|| f(0)),
                Key::End => count.checked_sub(1).map(&*f),
                // First-letter type-ahead, scanning forward from the
                // current selection and wrapping.
                Key::Char(c) if !k.ctrl && !k.meta => {
                    let c = c.to_lowercase().next()?;
                    (1..=count)
                        .map(|step| (selected + step) % count)
                        .find(|i| {
                            options[*i]
                                .chars()
                                .next()
                                .and_then(|f| f.to_lowercase().next())
                                == Some(c)
                        })
                        .map(&*f)
                }
                _ => None,
            });
        }
        if sel.disabled {
            trigger = trigger.opacity(0.5);
        }
        if let Some(key) = &sel.key {
            trigger = trigger.id(key);
        }
        trigger
    }
}
