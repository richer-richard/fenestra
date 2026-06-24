//! [`spin_button`]: a compact number stepper — a value flanked by − / + step
//! buttons. App-driven: the value and the per-bound enablement come from app
//! state, and the buttons emit step messages.
//!
//! ```
//! use fenestra_kit::spin_button;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Less,
//!     More,
//! }
//!
//! let el: fenestra_core::Element<Msg> = spin_button("3")
//!     .label("Quantity")
//!     .on_decrement(Msg::Less)
//!     .on_increment(Msg::More)
//!     .into();
//! ```

use fenestra_core::{
    Cursor, Element, Key, SP3, Semantics, TextSize, Theme, Transition, Weight, div, row, text,
};

use crate::icons;

/// A number stepper under construction; converts into an [`Element`].
pub struct SpinButton<Msg> {
    value: String,
    label: Option<String>,
    on_decrement: Option<Msg>,
    on_increment: Option<Msg>,
    can_decrement: bool,
    can_increment: bool,
    value_num: f32,
    min: f32,
    max: f32,
}

/// A compact number stepper showing `value` between − and + buttons. The value
/// is app-formatted (so `"3"`, `"$5.00"`, and `"2.5×"` all work); wire
/// [`SpinButton::on_decrement`] / [`SpinButton::on_increment`] and gate the
/// buttons at the range ends with [`SpinButton::can_decrement`] /
/// [`SpinButton::can_increment`].
pub fn spin_button<Msg>(value: impl Into<String>) -> SpinButton<Msg> {
    SpinButton {
        value: value.into(),
        label: None,
        on_decrement: None,
        on_increment: None,
        can_decrement: true,
        can_increment: true,
        value_num: 0.0,
        min: 0.0,
        max: 0.0,
    }
}

impl<Msg> SpinButton<Msg> {
    /// Sets an accessible name for the stepper (e.g. "Quantity").
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Emits this message when the − button is pressed.
    #[must_use]
    pub fn on_decrement(mut self, msg: Msg) -> Self {
        self.on_decrement = Some(msg);
        self
    }

    /// Emits this message when the + button is pressed.
    #[must_use]
    pub fn on_increment(mut self, msg: Msg) -> Self {
        self.on_increment = Some(msg);
        self
    }

    /// Enables/disables the − button (gate it off at the minimum). Default on.
    #[must_use]
    pub fn can_decrement(mut self, can: bool) -> Self {
        self.can_decrement = can;
        self
    }

    /// Enables/disables the + button (gate it off at the maximum). Default on.
    #[must_use]
    pub fn can_increment(mut self, can: bool) -> Self {
        self.can_increment = can;
        self
    }

    /// Sets the numeric value and bounds exposed to assistive tech — the ARIA
    /// `spinbutton` role's `valuenow` / `valuemin` / `valuemax`. The visible text
    /// stays the formatted string you passed to [`spin_button`].
    #[must_use]
    pub fn range(mut self, value: f32, min: f32, max: f32) -> Self {
        self.value_num = value;
        self.min = min;
        self.max = max;
        self
    }
}

/// One 36×36 step button; dimmed and inert when disabled.
fn step<Msg>(icon: Element<Msg>, label: &str, enabled: bool, msg: Option<Msg>) -> Element<Msg> {
    let mut cell = row()
        .items_center()
        .justify_center()
        .w(36.0)
        .h(36.0)
        .shrink0()
        .transition(Transition::colors())
        .semantics(Semantics::Button)
        .label(label.to_owned())
        .children([icon
            .w(16.0)
            .h(16.0)
            .themed(move |t: &Theme, s| s.color(if enabled { t.text } else { t.text_disabled }))]);
    if enabled {
        cell = cell
            .state_layer(|t| t.text)
            .focusable(true)
            .cursor(Cursor::Pointer);
        if let Some(msg) = msg {
            cell = cell.on_click(msg);
        }
    } else {
        cell = cell.disabled(true);
    }
    cell
}

/// A 36px-tall hairline divider between the cells.
fn rule<Msg>() -> Element<Msg> {
    div()
        .w(1.0)
        .h(36.0)
        .shrink0()
        .themed(|t: &Theme, s| s.bg(t.border))
}

impl<Msg: Clone + 'static> From<SpinButton<Msg>> for Element<Msg> {
    fn from(sb: SpinButton<Msg>) -> Self {
        // Captured for the accessible value and the arrow-key handlers, before
        // the originals move into the visible text and the step buttons.
        let display = sb.value.clone();
        let (inc, dec) = (sb.on_increment.clone(), sb.on_decrement.clone());
        let (can_inc, can_dec) = (sb.can_increment, sb.can_decrement);
        let value = row()
            .items_center()
            .justify_center()
            .min_w(52.0)
            .h(36.0)
            .px(SP3)
            .shrink0()
            .children([text(sb.value)
                .size(TextSize::Sm)
                .tabular()
                .weight(Weight::Medium)
                .themed(|t: &Theme, s| s.color(t.text))]);

        let mut group = row()
            .items_center()
            .self_start()
            .overflow_hidden()
            .themed(|t: &Theme, s| {
                s.rounded(t.radius.md)
                    .border(1.0, t.border)
                    .bg(t.surface_raised)
            })
            .children([
                step(
                    icons::minus(),
                    "Decrease",
                    sb.can_decrement,
                    sb.on_decrement,
                ),
                rule(),
                value,
                rule(),
                step(icons::plus(), "Increase", sb.can_increment, sb.on_increment),
            ])
            // ARIA spinbutton: the group is the focusable control; ↑/↓ step it
            // (gated at the bounds), and the formatted text is its value.
            .semantics(Semantics::Spinbutton {
                value: sb.value_num,
                min: sb.min,
                max: sb.max,
            })
            .value(display)
            .focusable(true)
            .on_key(move |k| match k.key {
                Key::ArrowUp if can_inc => inc.clone(),
                Key::ArrowDown if can_dec => dec.clone(),
                _ => None,
            });
        if let Some(label) = sb.label {
            group = group.label(label);
        }
        group
    }
}
