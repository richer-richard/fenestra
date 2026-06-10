//! Switch: a 36x20 track with a 16px thumb that travels over 160ms.
//!
//! ```
//! use fenestra_kit::switch;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Toggle,
//! }
//!
//! let el: fenestra_core::Element<Msg> = switch(true).on_toggle(Msg::Toggle).into();
//! ```

use fenestra_core::{
    Cursor, Element, R_FULL, SP2, ShadowToken, TextSize, Theme, Transition, row, text,
};

/// A switch under construction; converts into an [`Element`].
pub struct Switch<Msg> {
    on: bool,
    label: Option<String>,
    disabled: bool,
    on_toggle: Option<Msg>,
    key: Option<String>,
}

/// A toggle switch reflecting `on`.
pub fn switch<Msg>(on: bool) -> Switch<Msg> {
    Switch {
        on,
        label: None,
        disabled: false,
        on_toggle: None,
        key: None,
    }
}

impl<Msg> Switch<Msg> {
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

/// Track and thumb geometry per the spec: 36x20 track, 16px thumb, 2px gap.
const TRACK_W: f32 = 36.0;
const TRACK_H: f32 = 20.0;
const THUMB: f32 = 16.0;
const GAP: f32 = 2.0;
/// Thumb travel duration.
const TRAVEL_MS: f32 = 160.0;

impl<Msg> From<Switch<Msg>> for Element<Msg> {
    fn from(sw: Switch<Msg>) -> Self {
        let on = sw.on;
        let thumb_left = if on { TRACK_W - THUMB - GAP } else { GAP };

        let thumb = fenestra_core::div()
            .absolute()
            .top(GAP)
            .left(thumb_left)
            .w(THUMB)
            .h(THUMB)
            .rounded_full()
            .shadow(ShadowToken::Sm)
            .themed(|t: &Theme, s| s.bg(t.surface_raised))
            .transition(Transition::colors().offsets(true).duration_ms(TRAVEL_MS));

        let track = fenestra_core::div()
            .w(TRACK_W)
            .h(TRACK_H)
            .rounded(R_FULL)
            .shrink0()
            .children([thumb])
            .transition(Transition::colors())
            .themed(move |t: &Theme, s| {
                if on {
                    s.bg(t.accent)
                } else {
                    s.bg(t.neutrals.step(6))
                }
            });

        let mut el = row()
            .items_center()
            .gap(SP2)
            .shrink0()
            .children([track])
            .focusable(true)
            .cursor(Cursor::Pointer)
            .disabled(sw.disabled);
        if let Some(label) = sw.label {
            el = el.children([text(label).size(TextSize::Sm)]);
        }
        if sw.disabled {
            el = el.opacity(0.5);
        }
        if let Some(key) = &sw.key {
            el = el.id(key);
        }
        if let Some(msg) = sw.on_toggle {
            el = el.on_click(msg);
        }
        el
    }
}
