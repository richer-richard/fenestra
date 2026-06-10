//! Checkbox with an animated check stroke (120ms draw-on).
//!
//! ```
//! use fenestra_kit::checkbox;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Toggle,
//! }
//!
//! let el: fenestra_core::Element<Msg> =
//!     checkbox(true).label("Enable notifications").on_toggle(Msg::Toggle).into();
//! ```

use fenestra_core::{Cursor, Element, MotionDuration, SP2, TextSize, Theme, Transition, row, text};

use crate::icons;

/// A checkbox under construction; converts into an [`Element`].
pub struct Checkbox<Msg> {
    checked: bool,
    label: Option<String>,
    disabled: bool,
    on_toggle: Option<Msg>,
    key: Option<String>,
}

/// A checkbox reflecting `checked`; emits its toggle message on click.
pub fn checkbox<Msg>(checked: bool) -> Checkbox<Msg> {
    Checkbox {
        checked,
        label: None,
        disabled: false,
        on_toggle: None,
        key: None,
    }
}

impl<Msg> Checkbox<Msg> {
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

    /// Emits this message when toggled.
    pub fn on_toggle(mut self, msg: Msg) -> Self {
        self.on_toggle = Some(msg);
        self
    }

    /// Stable identity key.
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

impl<Msg> From<Checkbox<Msg>> for Element<Msg> {
    fn from(c: Checkbox<Msg>) -> Self {
        let checked = c.checked;
        // The check stroke draws on over 120ms via path trim.
        let check = icons::check()
            .w(12.0)
            .h(12.0)
            .trim(if checked { 1.0 } else { 0.0 })
            .themed(|t: &Theme, s| s.color(t.on_accent))
            .transition(
                Transition::colors()
                    .lengths(true)
                    .duration(MotionDuration::Fast),
            );

        let boxed = row()
            .items_center()
            .justify_center()
            .w(16.0)
            .h(16.0)
            .rounded(4.0)
            .shrink0()
            .children([check])
            .transition(Transition::colors())
            .themed(move |t: &Theme, s| {
                if checked {
                    s.bg(t.accent).border(1.0, t.accent)
                } else {
                    s.bg(t.surface_raised).border(1.0, t.border_strong)
                }
            });

        let mut el = row()
            .items_center()
            .gap(SP2)
            .shrink0()
            .children([boxed])
            .focusable(true)
            .cursor(Cursor::Pointer)
            .disabled(c.disabled);
        if let Some(label) = c.label {
            el = el.children([text(label).size(TextSize::Sm)]);
        }
        if c.disabled {
            el = el.opacity(0.5);
        }
        if let Some(key) = &c.key {
            el = el.id(key);
        }
        if let Some(msg) = c.on_toggle {
            el = el.on_click(msg);
        }
        el
    }
}
