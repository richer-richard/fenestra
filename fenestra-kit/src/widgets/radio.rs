//! Radio button: a 16px circle whose selected state is an accent ring.
//!
//! ```
//! use fenestra_kit::radio;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Pick(u8),
//! }
//!
//! let el: fenestra_core::Element<Msg> =
//!     radio(true).label("Annual billing").on_select(Msg::Pick(0)).into();
//! ```

use fenestra_core::{Cursor, Element, SP2, Semantics, TextSize, Theme, Transition, row, text};

/// A radio button under construction; converts into an [`Element`].
pub struct Radio<Msg> {
    selected: bool,
    label: Option<String>,
    disabled: bool,
    on_select: Option<Msg>,
    key: Option<String>,
}

/// A radio button reflecting `selected`. Group behavior comes from the
/// app: each radio emits its own select message.
pub fn radio<Msg>(selected: bool) -> Radio<Msg> {
    Radio {
        selected,
        label: None,
        disabled: false,
        on_select: None,
        key: None,
    }
}

impl<Msg> Radio<Msg> {
    /// Adds a clickable text label to the right.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Disables interaction.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Emits this message when selected.
    pub fn on_select(mut self, msg: Msg) -> Self {
        self.on_select = Some(msg);
        self
    }

    /// Stable identity key.
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

impl<Msg> From<Radio<Msg>> for Element<Msg> {
    fn from(r: Radio<Msg>) -> Self {
        let selected = r.selected;
        let circle = fenestra_core::div()
            .w(16.0)
            .h(16.0)
            .rounded_full()
            .shrink0()
            .transition(Transition::colors())
            .themed(move |t: &Theme, s| {
                if selected {
                    // A 5px accent ring leaves a 6px surface hole: the dot.
                    s.bg(t.surface_raised).border(5.0, t.accent)
                } else {
                    s.bg(t.surface_raised).border(1.0, t.border_strong)
                }
            });

        let mut el = row()
            .items_center()
            .gap(SP2)
            .shrink0()
            .children([circle])
            .focusable(true)
            .cursor(Cursor::Pointer)
            .disabled(r.disabled)
            .semantics(Semantics::Radio { selected });
        if let Some(label) = r.label {
            el = el
                .label(label.clone())
                .children([text(label).size(TextSize::Sm)]);
        }
        if r.disabled {
            el = el.opacity(0.5);
        }
        if let Some(key) = &r.key {
            el = el.id(key);
        }
        if let Some(msg) = r.on_select {
            el = el.on_click(msg);
        }
        el
    }
}
