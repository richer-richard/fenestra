//! Checkbox with an animated check stroke (120ms draw-on); two-state or
//! tri-state ([`Checkbox::indeterminate`]).
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

use fenestra_core::{
    Cursor, Element, MotionDuration, R_FULL, SP2, Semantics, TextSize, Theme, Transition, div, row,
    text,
};

use super::ControlSize;
use crate::icons;

/// A checkbox under construction; converts into an [`Element`].
pub struct Checkbox<Msg> {
    checked: bool,
    indeterminate: bool,
    size: ControlSize,
    label: Option<String>,
    disabled: bool,
    on_toggle: Option<Msg>,
    key: Option<String>,
}

/// A checkbox reflecting `checked`; emits its toggle message on click.
pub fn checkbox<Msg>(checked: bool) -> Checkbox<Msg> {
    Checkbox {
        checked,
        indeterminate: false,
        size: ControlSize::Md,
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

    /// Puts the checkbox in the indeterminate (mixed) state: a dash instead of
    /// a check, and `aria-checked="mixed"`. Common for a "select all" box whose
    /// children are partially selected.
    pub fn indeterminate(mut self, on: bool) -> Self {
        self.indeterminate = on;
        self
    }

    /// Sets the box size via [`ControlSize`] (Xs 12 / Sm 14 / Md 16 / Lg 20 px).
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
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
        let mixed = c.indeterminate;
        // Md is byte-identical to the pre-size-variant box (16 / r4 / 12px glyph).
        let (box_sz, radius) = match c.size {
            ControlSize::Xs => (12.0, 3.0),
            ControlSize::Sm => (14.0, 3.0),
            ControlSize::Md => (16.0, 4.0),
            ControlSize::Lg => (20.0, 5.0),
        };
        let glyph = box_sz * 0.75;

        let inner: Element<Msg> = if mixed {
            // Indeterminate: a centered horizontal dash.
            div()
                .w(glyph * 0.8)
                .h(2.0)
                .rounded(R_FULL)
                .themed(|t: &Theme, s| s.bg(t.on_accent))
        } else {
            // The check stroke draws on over 120ms via path trim.
            icons::check()
                .w(glyph)
                .h(glyph)
                .trim(if checked { 1.0 } else { 0.0 })
                .themed(|t: &Theme, s| s.color(t.on_accent))
                .transition(
                    Transition::colors()
                        .lengths(true)
                        .duration(MotionDuration::Fast),
                )
        };

        let filled = checked || mixed;
        let boxed = row()
            .items_center()
            .justify_center()
            .w(box_sz)
            .h(box_sz)
            .rounded(radius)
            .shrink0()
            .children([inner])
            .transition(Transition::colors())
            .themed(move |t: &Theme, s| {
                if filled {
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
            .disabled(c.disabled)
            .semantics(Semantics::Checkbox { checked, mixed });
        if let Some(label) = c.label {
            el = el
                .label(label.clone())
                .children([text(label).size(TextSize::Sm)]);
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
