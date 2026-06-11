//! Single-line text input on parley's `PlainEditor`: caret blink, selection,
//! placeholder, clipboard, Home/End and word jumps, basic IME.
//!
//! ```
//! use fenestra_kit::text_input;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Query(String),
//! }
//!
//! let el: fenestra_core::Element<Msg> = text_input("hello")
//!     .placeholder("Search…")
//!     .on_input(Msg::Query)
//!     .into();
//! ```

use fenestra_core::{Element, R_MD, SP3, Theme, Transition, raw_input};

use super::ControlSize;

/// A text input under construction; converts into an [`Element`].
pub struct TextInput<Msg> {
    value: String,
    placeholder: String,
    size: ControlSize,
    width: f32,
    invalid: bool,
    disabled: bool,
    on_input: Option<std::rc::Rc<dyn Fn(String) -> Msg>>,
    key: Option<String>,
}

/// A single-line text input showing the app-owned `value`.
pub fn text_input<Msg>(value: impl Into<String>) -> TextInput<Msg> {
    TextInput {
        value: value.into(),
        placeholder: String::new(),
        size: ControlSize::default(),
        width: 220.0,
        invalid: false,
        disabled: false,
        on_input: None,
        key: None,
    }
}

impl<Msg> TextInput<Msg> {
    /// Placeholder shown while the value is empty.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Sets the control size.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Sets the width in logical px (220 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Shows the invalid state (danger border).
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Disables editing.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Maps every edit of the value to a message.
    pub fn on_input(mut self, f: impl Fn(String) -> Msg + 'static) -> Self {
        self.on_input = Some(std::rc::Rc::new(f));
        self
    }

    /// Stable identity key (recommended: editor state is kept per id).
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

impl<Msg: 'static> From<TextInput<Msg>> for Element<Msg> {
    fn from(t: TextInput<Msg>) -> Self {
        let invalid = t.invalid;
        let placeholder = t.placeholder.clone();
        let mut el = raw_input(t.value, t.placeholder)
            .w(t.width)
            .h(t.size.height())
            .px(SP3)
            .rounded(R_MD)
            .shrink0()
            .size(t.size.text_size())
            .transition(Transition::colors())
            .disabled(t.disabled);
        if !placeholder.is_empty() {
            el = el.label(placeholder);
        }

        el = el.themed(move |theme: &Theme, s| {
            let base = s.bg(theme.surface_raised);
            if invalid {
                base.border(1.0, theme.danger.border)
            } else {
                base.border(1.0, theme.border)
            }
        });
        if !t.invalid && !t.disabled {
            el = el
                .hover_themed(|theme, s| s.border(1.0, theme.border_strong))
                .focus_themed(|theme, s| s.border(1.0, theme.accent));
        }
        if t.disabled {
            el = el
                .opacity(0.5)
                .themed(|theme: &Theme, s| s.bg(theme.surface));
        }
        if let Some(key) = &t.key {
            el = el.id(key);
        }
        if let Some(f) = t.on_input {
            el = el.on_input(move |s| f(s.to_owned()));
        }
        el
    }
}
