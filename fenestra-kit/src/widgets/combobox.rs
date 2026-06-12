//! Combobox: a text input that filters a pickable listbox. Elm-pure — the
//! app owns the text and the open flag.
//!
//! ```
//! use fenestra_kit::combobox;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Type(String),
//!     Pick(String),
//!     Close,
//! }
//!
//! let el: fenestra_core::Element<Msg> =
//!     combobox("ru", true, ["Rust", "Ruby", "Python"])
//!         .on_input(Msg::Type)
//!         .on_pick(Msg::Pick)
//!         .on_close(Msg::Close)
//!         .id("lang")
//!         .into();
//! ```

use fenestra_core::{Element, Overlay, OverlayMode, OverlayPlacement, Semantics, col};

use crate::{menu, text_input};

/// A combobox under construction; converts into an [`Element`].
pub struct Combobox<Msg> {
    value: String,
    open: bool,
    options: Vec<String>,
    width: f32,
    placeholder: String,
    on_input: Option<std::rc::Rc<dyn Fn(String) -> Msg>>,
    on_pick: Option<std::rc::Rc<dyn Fn(String) -> Msg>>,
    on_close: Option<Msg>,
    key: Option<String>,
}

/// An editable select: typing filters `options` (case-insensitive
/// contains) while `open` is true; picking an option emits its text. The
/// app owns `value` and `open` (set `open` in your `on_input` handler,
/// clear it on pick/close).
pub fn combobox<Msg>(
    value: impl Into<String>,
    open: bool,
    options: impl IntoIterator<Item = impl Into<String>>,
) -> Combobox<Msg> {
    Combobox {
        value: value.into(),
        open,
        options: options.into_iter().map(Into::into).collect(),
        width: 220.0,
        placeholder: String::new(),
        on_input: None,
        on_pick: None,
        on_close: None,
        key: None,
    }
}

impl<Msg> Combobox<Msg> {
    /// Sets the trigger width in logical px (220 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Placeholder shown while the value is empty.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Maps every edit of the text to a message (set your open flag here).
    pub fn on_input(mut self, f: impl Fn(String) -> Msg + 'static) -> Self {
        self.on_input = Some(std::rc::Rc::new(f));
        self
    }

    /// Maps a picked option's text to a message.
    pub fn on_pick(mut self, f: impl Fn(String) -> Msg + 'static) -> Self {
        self.on_pick = Some(std::rc::Rc::new(f));
        self
    }

    /// Emitted when the listbox wants to close (outside click, Escape).
    pub fn on_close(mut self, msg: Msg) -> Self {
        self.on_close = Some(msg);
        self
    }

    /// Stable identity key (recommended: editor state is kept per id).
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

impl<Msg: Clone + 'static> From<Combobox<Msg>> for Element<Msg> {
    fn from(c: Combobox<Msg>) -> Self {
        let needle = c.value.to_lowercase();
        let filtered: Vec<String> = c
            .options
            .iter()
            .filter(|o| needle.is_empty() || o.to_lowercase().contains(&needle))
            .cloned()
            .collect();

        let mut input = text_input(&c.value)
            .placeholder(c.placeholder.clone())
            .width(c.width);
        if let Some(f) = &c.on_input {
            let f = std::rc::Rc::clone(f);
            input = input.on_input(move |s| f(s));
        }

        let mut anchor = col()
            .w(c.width)
            .semantics(Semantics::ComboBox)
            .children([Element::from(input)]);

        if c.open
            && !filtered.is_empty()
            && let Some(pick) = c.on_pick.clone()
        {
            let mut listbox = menu(filtered.into_iter().map(move |option| {
                let msg = pick(option.clone());
                (option, msg)
            }))
            .w(c.width)
            .overlay(Overlay {
                mode: OverlayMode::Open,
                placement: OverlayPlacement::Below { gap: 4.0 },
                backdrop: false,
                trap_focus: false,
            });
            if let Some(close) = c.on_close.clone() {
                listbox = listbox.on_close(close);
            }
            anchor = anchor.child(listbox);
        }
        if let Some(key) = &c.key {
            anchor = anchor.id(key);
        }
        anchor
    }
}
