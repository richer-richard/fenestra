//! Multi-select: a wrapping set of toggleable option chips. Each chip is a
//! checkbox — selected chips fill with the accent and a check, unselected ones
//! are outlined; a click or Space/Enter toggles it through `on_toggle`. Elm-pure:
//! the app owns which options are selected and flips one per toggle. For free
//! text instead of a fixed option set, reach for [`tag_input`](crate::tag_input);
//! for a single choice, [`select`](crate::select).
//!
//! ```
//! use fenestra_kit::multi_select;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Toggle(usize),
//! }
//!
//! let el: fenestra_core::Element<Msg> =
//!     multi_select([0, 2], ["Rust", "Go", "Zig"]).on_toggle(Msg::Toggle).into();
//! ```

use std::rc::Rc;

use fenestra_core::{
    Cursor, Element, Key, R_FULL, SP1, SP2, SP3, Semantics, TextSize, Theme, Transition, row, text,
};

use crate::icons;

/// A multi-select under construction; converts into an [`Element`].
pub struct MultiSelect<Msg> {
    options: Vec<String>,
    selected: Vec<bool>,
    disabled: bool,
    on_toggle: Option<Rc<dyn Fn(usize) -> Msg>>,
    key: Option<String>,
}

/// A multi-select over `options`, with the indices in `selected` pre-checked.
pub fn multi_select<Msg>(
    selected: impl IntoIterator<Item = usize>,
    options: impl IntoIterator<Item = impl Into<String>>,
) -> MultiSelect<Msg> {
    let options: Vec<String> = options.into_iter().map(Into::into).collect();
    let mut flags = vec![false; options.len()];
    for i in selected {
        if let Some(slot) = flags.get_mut(i) {
            *slot = true;
        }
    }
    MultiSelect {
        options,
        selected: flags,
        disabled: false,
        on_toggle: None,
        key: None,
    }
}

impl<Msg> MultiSelect<Msg> {
    /// Disables interaction and dims the chips.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Maps the toggled option's index to a message (the app flips that option).
    pub fn on_toggle(mut self, f: impl Fn(usize) -> Msg + 'static) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }

    /// Stable identity key for the chip group.
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

impl<Msg: 'static> From<MultiSelect<Msg>> for Element<Msg> {
    fn from(ms: MultiSelect<Msg>) -> Self {
        let disabled = ms.disabled;
        let chips = ms.options.iter().enumerate().map(|(i, opt)| {
            let checked = ms.selected.get(i).copied().unwrap_or(false);
            let mut kids: Vec<Element<Msg>> = Vec::with_capacity(2);
            if checked {
                kids.push(
                    icons::check()
                        .w(16.0)
                        .h(16.0)
                        .themed(|t: &Theme, s| s.color(t.accent_text)),
                );
            }
            kids.push(
                text(opt.clone())
                    .size(TextSize::Sm)
                    .themed(move |t: &Theme, s| {
                        s.color(if checked { t.accent_text } else { t.text })
                    }),
            );
            let mut chip = row()
                .items_center()
                .gap(SP1)
                .px(SP3)
                .h(32.0)
                .rounded(R_FULL)
                .shrink0()
                .transition(Transition::colors())
                .semantics(Semantics::Checkbox {
                    checked,
                    mixed: false,
                })
                .label(opt.clone())
                .children(kids)
                .themed(move |t: &Theme, s| {
                    if checked {
                        s.bg(t.accent_bg).border(1.0, t.accent)
                    } else {
                        s.bg(t.surface_raised).border(1.0, t.border)
                    }
                });
            if disabled {
                chip = chip.disabled(true).opacity(0.5);
            } else {
                chip = chip
                    .focusable(true)
                    .cursor(Cursor::Pointer)
                    .state_layer(|t| t.text);
                if let Some(f) = &ms.on_toggle {
                    let (click, press) = (Rc::clone(f), Rc::clone(f));
                    chip = chip.on_click(click(i)).on_key(move |k| match k.key {
                        Key::Enter | Key::Space => Some(press(i)),
                        _ => None,
                    });
                }
            }
            chip
        });

        let mut group = row().wrap().gap(SP2).items_center().children(chips);
        if let Some(key) = &ms.key {
            group = group.id(key);
        }
        group
    }
}
