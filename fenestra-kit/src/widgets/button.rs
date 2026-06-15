//! Buttons: four variants, four sizes, a uniform state layer (neutral
//! variants) or ramp-step hover (solid brand), press-scale, and a Fast color
//! transition.
//!
//! ```
//! use fenestra_kit::{ButtonVariant, button};
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Save,
//! }
//!
//! let el: fenestra_core::Element<Msg> = fenestra_core::row().children([
//!     button("Save").on_click(Msg::Save),
//!     button("Cancel").variant(ButtonVariant::Secondary).on_click(Msg::Save),
//! ]);
//! ```

use fenestra_core::{Color, Cursor, Element, Semantics, Theme, Transition, Weight, row, text};

use super::ControlSize;

/// The label color for a button variant; dimmed to the disabled token for the
/// neutral variants (solids dim via subtree opacity instead).
fn label_color(t: &Theme, variant: ButtonVariant, disabled: bool) -> Color {
    match variant {
        ButtonVariant::Primary | ButtonVariant::Danger => t.on_accent,
        ButtonVariant::Secondary | ButtonVariant::Ghost => {
            if disabled {
                t.text_disabled
            } else {
                t.text
            }
        }
    }
}

/// The icon color for an icon button (ghost icons rest at the muted role).
fn icon_color(t: &Theme, variant: ButtonVariant, disabled: bool) -> Color {
    match variant {
        ButtonVariant::Primary | ButtonVariant::Danger => t.on_accent,
        ButtonVariant::Secondary => {
            if disabled {
                t.text_disabled
            } else {
                t.text
            }
        }
        ButtonVariant::Ghost => {
            if disabled {
                t.text_disabled
            } else {
                t.text_muted
            }
        }
    }
}

/// Applies a variant's resting fill and interaction model to a button body.
/// Solid brand variants step the accent/danger ramp on hover/press (their
/// gamut-mapped brand colors); neutral variants use the uniform state layer.
fn apply_variant<Msg>(el: Element<Msg>, variant: ButtonVariant) -> Element<Msg> {
    match variant {
        ButtonVariant::Primary => el
            .themed(|t: &Theme, s| s.bg(t.accent).highlight_top(t.on_accent.with_alpha(0.14)))
            .hover_themed(|t, s| s.bg(t.accent_hover))
            .active_themed(|t, s| s.bg(t.accent_active)),
        ButtonVariant::Danger => el
            .themed(|t: &Theme, s| {
                s.bg(t.danger.solid)
                    .highlight_top(t.on_accent.with_alpha(0.14))
            })
            .hover_themed(|t, s| s.bg(t.danger.solid_hover))
            .active_themed(|t, s| s.bg(t.danger.solid_active)),
        ButtonVariant::Secondary => el
            .themed(|t: &Theme, s| s.bg(t.surface_raised).border(1.0, t.border))
            .state_layer(|t| t.text),
        ButtonVariant::Ghost => el
            // Transparent ink base so the state-layer veil fades in from nothing.
            .themed(|t: &Theme, s| s.bg(t.text.with_alpha(0.0)))
            .state_layer(|t| t.text),
    }
}

/// Visual emphasis of a button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Solid accent: the one primary action on a surface.
    #[default]
    Primary,
    /// Raised surface with a border.
    Secondary,
    /// Transparent until hovered.
    Ghost,
    /// Solid danger red for destructive actions.
    Danger,
}

/// A button under construction; converts into an [`Element`].
pub struct Button<Msg> {
    label: String,
    variant: ButtonVariant,
    size: ControlSize,
    disabled: bool,
    on_click: Option<Msg>,
    key: Option<String>,
}

/// A push button with a text label.
pub fn button<Msg>(label: impl Into<String>) -> Button<Msg> {
    Button {
        label: label.into(),
        variant: ButtonVariant::default(),
        size: ControlSize::default(),
        disabled: false,
        on_click: None,
        key: None,
    }
}

impl<Msg> Button<Msg> {
    /// Sets the visual variant.
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets the control size.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Disables the button.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Emits this message on click (or Enter/Space while focused).
    pub fn on_click(mut self, msg: Msg) -> Self {
        self.on_click = Some(msg);
        self
    }

    /// Stable identity key for reorderable contexts.
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

impl<Msg> From<Button<Msg>> for Element<Msg> {
    fn from(b: Button<Msg>) -> Self {
        let variant = b.variant;
        let disabled = b.disabled;
        let label_text = b.label.clone();
        let label = text(b.label)
            .size(b.size.text_size())
            .weight(Weight::Medium)
            .themed(move |t: &Theme, s| s.color(label_color(t, variant, disabled)));

        let mut el = row()
            .items_center()
            .justify_center()
            .h(b.size.height())
            .px(b.size.padding_x())
            .gap(b.size.gap())
            .themed(|t: &Theme, s| s.rounded(t.radius.md))
            .shrink0()
            .children([label])
            .transition(Transition::colors())
            .press_scale()
            .focusable(true)
            .cursor(Cursor::Pointer)
            .disabled(b.disabled)
            .semantics(Semantics::Button)
            .label(label_text);

        el = apply_variant(el, variant);
        // Solid brand fills keep their gamut-mapped ramp steps; the state layer
        // handles disabled for the neutral variants, so only solids dim here.
        if disabled && matches!(variant, ButtonVariant::Primary | ButtonVariant::Danger) {
            el = el.opacity(0.5);
        }
        if let Some(key) = &b.key {
            el = el.id(key);
        }
        if let Some(msg) = b.on_click {
            el = el.on_click(msg);
        }
        el
    }
}

/// A square button holding an icon; Ghost by default.
pub struct IconButton<Msg> {
    icon: Element<Msg>,
    variant: ButtonVariant,
    size: ControlSize,
    disabled: bool,
    label: Option<String>,
    on_click: Option<Msg>,
    key: Option<String>,
}

/// A square icon button (Ghost variant by default). Give it a `.label` —
/// icon-only buttons have no accessible name otherwise.
pub fn icon_button<Msg>(icon: impl Into<Element<Msg>>) -> IconButton<Msg> {
    IconButton {
        icon: icon.into(),
        variant: ButtonVariant::Ghost,
        size: ControlSize::default(),
        disabled: false,
        label: None,
        on_click: None,
        key: None,
    }
}

impl<Msg> IconButton<Msg> {
    /// Sets the visual variant.
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets the control size.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Disables the button.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets the accessible name (icon-only buttons need one).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Emits this message on click.
    pub fn on_click(mut self, msg: Msg) -> Self {
        self.on_click = Some(msg);
        self
    }

    /// Stable identity key.
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

impl<Msg> From<IconButton<Msg>> for Element<Msg> {
    fn from(b: IconButton<Msg>) -> Self {
        let variant = b.variant;
        let disabled = b.disabled;
        let edge = b.size.icon();
        let icon = b
            .icon
            .w(edge)
            .h(edge)
            .themed(move |t: &Theme, s| s.color(icon_color(t, variant, disabled)));
        let side = b.size.height();
        let mut el = row()
            .items_center()
            .justify_center()
            .w(side)
            .h(side)
            .themed(|t: &Theme, s| s.rounded(t.radius.md))
            .shrink0()
            .children([icon])
            .transition(Transition::colors())
            .press_scale()
            .focusable(true)
            .cursor(Cursor::Pointer)
            .disabled(b.disabled)
            .semantics(Semantics::Button);
        if let Some(label) = b.label {
            el = el.label(label);
        }

        el = apply_variant(el, variant);
        if disabled && matches!(variant, ButtonVariant::Primary | ButtonVariant::Danger) {
            el = el.opacity(0.5);
        }
        if let Some(key) = &b.key {
            el = el.id(key);
        }
        if let Some(msg) = b.on_click {
            el = el.on_click(msg);
        }
        el
    }
}
