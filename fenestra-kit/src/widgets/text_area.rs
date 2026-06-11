//! Multiline text area on the same parley editor as `text_input`: wraps to
//! its width, grows with the content, Enter inserts newlines, and arrows
//! move by line.
//!
//! ```
//! use fenestra_kit::text_area;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Notes(String),
//! }
//!
//! let el: fenestra_core::Element<Msg> = text_area("dear diary…")
//!     .placeholder("Write something…")
//!     .on_input(Msg::Notes)
//!     .into();
//! ```

use fenestra_core::{Element, R_MD, SP2, SP3, TextSize, Theme, Transition, raw_text_area};

/// A text area under construction; converts into an [`Element`].
pub struct TextArea<Msg> {
    value: String,
    placeholder: String,
    width: f32,
    min_height: f32,
    invalid: bool,
    disabled: bool,
    on_input: Option<std::rc::Rc<dyn Fn(String) -> Msg>>,
    key: Option<String>,
}

/// A multiline text area showing the app-owned `value`. The box grows with
/// the wrapped content from `min_height` (80 by default); cap it with an
/// outer scroll container if needed.
pub fn text_area<Msg>(value: impl Into<String>) -> TextArea<Msg> {
    TextArea {
        value: value.into(),
        placeholder: String::new(),
        width: 320.0,
        min_height: 80.0,
        invalid: false,
        disabled: false,
        on_input: None,
        key: None,
    }
}

impl<Msg> TextArea<Msg> {
    /// Placeholder shown while the value is empty.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Sets the width in logical px (320 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Minimum height in logical px before content grows it (80 by default).
    pub fn min_height(mut self, min_height: f32) -> Self {
        self.min_height = min_height;
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

impl<Msg: 'static> From<TextArea<Msg>> for Element<Msg> {
    fn from(t: TextArea<Msg>) -> Self {
        let invalid = t.invalid;
        let mut el = raw_text_area(t.value, t.placeholder)
            .w(t.width)
            .min_h(t.min_height)
            .px(SP3)
            .py(SP2)
            .rounded(R_MD)
            .shrink0()
            .size(TextSize::Sm)
            .transition(Transition::colors())
            .disabled(t.disabled);

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
