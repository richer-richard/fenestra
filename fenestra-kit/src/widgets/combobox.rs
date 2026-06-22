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

use fenestra_core::{
    Cursor, Element, Key, Overlay, OverlayMode, OverlayPlacement, SP1, SP2, Semantics, Surface,
    TextSize, Theme, Transition, col, row, text,
};

use crate::text_input;

/// A combobox under construction; converts into an [`Element`].
pub struct Combobox<Msg> {
    value: String,
    open: bool,
    options: Vec<String>,
    width: f32,
    placeholder: String,
    highlighted: Option<usize>,
    on_input: Option<std::rc::Rc<dyn Fn(String) -> Msg>>,
    on_pick: Option<std::rc::Rc<dyn Fn(String) -> Msg>>,
    on_navigate: Option<std::rc::Rc<dyn Fn(usize) -> Msg>>,
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
        highlighted: None,
        on_input: None,
        on_pick: None,
        on_navigate: None,
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

    /// The app-owned keyboard cursor: the index (into the currently visible,
    /// filtered options) drawn as the active option and picked by Enter. Pair
    /// with [`Combobox::on_navigate`] and reset it to `Some(0)` when the query
    /// changes. `None` draws no cursor (Enter still accepts the top match).
    pub fn highlighted(mut self, index: Option<usize>) -> Self {
        self.highlighted = index;
        self
    }

    /// Maps an arrow-key step to a message carrying the new highlight index
    /// (clamped to the visible range, like [`crate::select`]). Store it so the
    /// next render moves the cursor.
    pub fn on_navigate(mut self, f: impl Fn(usize) -> Msg + 'static) -> Self {
        self.on_navigate = Some(std::rc::Rc::new(f));
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

        let show_list = c.open && !filtered.is_empty() && c.on_pick.is_some();
        // The keyboard cursor, clamped into the visible (filtered) range:
        // `active` is the option Enter accepts; `cursor` draws the veil, but
        // only once the app owns a highlight (an un-wired combobox keeps a
        // plain list, yet Enter still accepts the top match).
        let active = filtered
            .len()
            .checked_sub(1)
            .map_or(0, |last| c.highlighted.unwrap_or(0).min(last));
        let cursor = (show_list && c.highlighted.is_some()).then_some(active);

        let mut input = text_input(&c.value)
            .placeholder(c.placeholder.clone())
            .width(c.width);
        if let Some(f) = &c.on_input {
            let f = std::rc::Rc::clone(f);
            input = input.on_input(move |s| f(s));
        }
        let mut input = Element::from(input);

        // The focused input drives the open list: Up/Down step the cursor
        // (clamped, matching `select`), Enter picks the active option. Home/End
        // keep moving the text caret (the focused editor consumes them before
        // `on_key`), and Escape closes via the listbox overlay's `on_close`.
        if show_list {
            let opts = filtered.clone();
            let pick = c.on_pick.clone();
            let nav = c.on_navigate.clone();
            input = input.on_key(move |k| match k.key {
                Key::ArrowDown => (active + 1 < opts.len())
                    .then_some(active + 1)
                    .and_then(|i| nav.as_ref().map(|f| f(i))),
                Key::ArrowUp => active
                    .checked_sub(1)
                    .and_then(|i| nav.as_ref().map(|f| f(i))),
                Key::Enter => pick.as_ref().map(|f| f(opts[active].clone())),
                _ => None,
            });
        }

        let mut anchor = col()
            .w(c.width)
            .semantics(Semantics::ComboBox)
            .children([input]);

        if let Some(pick) = c.on_pick.clone().filter(|_| show_list) {
            let mut listbox = col()
                .w(c.width)
                .p(SP1)
                .gap(2.0)
                .surface(Surface::Menu)
                .overlay(Overlay {
                    mode: OverlayMode::Open,
                    placement: OverlayPlacement::Below { gap: 4.0 },
                    backdrop: false,
                    trap_focus: false,
                })
                .children(filtered.iter().enumerate().map(|(i, option)| {
                    option_row(option, cursor == Some(i), pick(option.clone()))
                }));
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

/// One option row in the combobox listbox: a pointer target carrying its pick
/// message, tinted with the accent veil when it is the keyboard cursor. Like
/// the listbox it is not a tab stop — the input owns focus — so it opts out of
/// the focus ring after `on_click`.
fn option_row<Msg: Clone + 'static>(label: &str, active: bool, on_click: Msg) -> Element<Msg> {
    let mut text_el = text(label.to_owned()).size(TextSize::Sm);
    if active {
        text_el = text_el.themed(|t: &Theme, s| s.color(t.accent_text));
    }
    let mut item = row()
        .items_center()
        .px(SP2)
        .h(30.0)
        .themed(|t: &Theme, s| s.rounded((t.radius.lg - SP1).max(0.0)))
        .shrink0()
        .cursor(Cursor::Pointer)
        .semantics(Semantics::Button)
        .label(label.to_owned())
        .transition(Transition::colors())
        .state_layer(|t| t.text)
        .children([text_el])
        .on_click(on_click)
        .focusable(false);
    if active {
        item = item.themed(|t: &Theme, s| s.bg(t.accent_bg));
    }
    item
}
