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

use fenestra_core::{Element, SP3, Theme, Transition, raw_input, row, stack};

use super::{ControlSize, Density};

/// A text input under construction; converts into an [`Element`].
pub struct TextInput<Msg> {
    value: String,
    placeholder: String,
    size: ControlSize,
    density: Density,
    width: f32,
    invalid: bool,
    disabled: bool,
    on_input: Option<std::rc::Rc<dyn Fn(String) -> Msg>>,
    key: Option<String>,
    prefix: Option<Element<Msg>>,
    suffix: Option<Element<Msg>>,
}

/// A single-line text input showing the app-owned `value`.
pub fn text_input<Msg>(value: impl Into<String>) -> TextInput<Msg> {
    TextInput {
        value: value.into(),
        placeholder: String::new(),
        size: ControlSize::default(),
        density: Density::default(),
        width: 220.0,
        invalid: false,
        disabled: false,
        on_input: None,
        key: None,
        prefix: None,
        suffix: None,
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

    /// Sets the packing density ([`Density`]). `Comfortable` (default) is
    /// byte-identical to no call.
    pub fn density(mut self, density: Density) -> Self {
        self.density = density;
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

    /// A leading adornment (an icon or short text like `$`) shown inside the
    /// field at the start edge; the text is padded clear of it.
    pub fn prefix(mut self, adornment: impl Into<Element<Msg>>) -> Self {
        self.prefix = Some(adornment.into());
        self
    }

    /// A trailing adornment (an icon, unit, or affordance) shown inside the
    /// field at the end edge; the text is padded clear of it.
    pub fn suffix(mut self, adornment: impl Into<Element<Msg>>) -> Self {
        self.suffix = Some(adornment.into());
        self
    }
}

/// Width of an adornment slot — a square the field's height, so the icon sits
/// on the text's optical center and the text clears it.
const ADORN_SLOT: f32 = 36.0;

/// An adornment positioned absolutely at one edge of the field, filling the
/// height so its content centers vertically.
fn adornment_slot<Msg: 'static>(content: Element<Msg>, leading: bool) -> Element<Msg> {
    let slot = row()
        .absolute()
        .top(0.0)
        .bottom(0.0)
        .w(ADORN_SLOT)
        .items_center()
        .justify_center();
    let slot = if leading {
        slot.left(0.0)
    } else {
        slot.right(0.0)
    };
    slot.children([content])
}

impl<Msg: 'static> From<TextInput<Msg>> for Element<Msg> {
    fn from(t: TextInput<Msg>) -> Self {
        let invalid = t.invalid;
        let placeholder = t.placeholder.clone();
        let width = t.width;
        let (prefix, suffix) = (t.prefix, t.suffix);
        // Density scales the field height on the shared grid; the text size is
        // held (density is spacing, not type).
        let m = t.size.metrics_at(t.density);
        let mut el = raw_input(t.value, t.placeholder)
            .w(t.width)
            .h(m.height)
            .px(SP3)
            .themed(|t: &Theme, s| s.rounded(t.radius.md))
            .shrink0()
            .size(m.font)
            .transition(Transition::colors())
            .disabled(t.disabled)
            .invalid(invalid);
        // Reserve room for adornments so the text clears them.
        if prefix.is_some() {
            el = el.pl(ADORN_SLOT);
        }
        if suffix.is_some() {
            el = el.pr(ADORN_SLOT);
        }
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
        if prefix.is_none() && suffix.is_none() {
            return el;
        }
        // Adorned: the bordered, focusable input fills a sized wrapper; the
        // adornments overlay its padded ends (the focus ring stays on the input).
        let mut layers = vec![el.w_full()];
        if let Some(p) = prefix {
            layers.push(adornment_slot(p, true));
        }
        if let Some(s) = suffix {
            layers.push(adornment_slot(s, false));
        }
        stack().w(width).shrink0().children(layers)
    }
}
