//! Buttons: four variants, three sizes, hover/active/focus states, and a
//! Fast color transition.
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

use fenestra_core::{Cursor, Element, R_MD, SP2, Semantics, Theme, Transition, Weight, row, text};

use super::ControlSize;

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
        let label_text = b.label.clone();
        let label = text(b.label)
            .size(b.size.text_size())
            .weight(Weight::Medium)
            .themed(move |t: &Theme, s| match variant {
                ButtonVariant::Primary | ButtonVariant::Danger => s.color(t.on_accent),
                ButtonVariant::Secondary | ButtonVariant::Ghost => s.color(t.text),
            });

        let mut el = row()
            .items_center()
            .justify_center()
            .h(b.size.height())
            .px(b.size.padding_x())
            .gap(SP2)
            .rounded(R_MD)
            .shrink0()
            .children([label])
            .transition(Transition::colors())
            .focusable(true)
            .cursor(Cursor::Pointer)
            .disabled(b.disabled)
            .semantics(Semantics::Button)
            .label(label_text);

        el = match variant {
            ButtonVariant::Primary => el
                .themed(|t: &Theme, s| s.bg(t.accent).highlight_top(t.on_accent.with_alpha(0.14)))
                .hover_themed(|t, s| s.bg(t.accent_hover))
                .active_themed(|t, s| s.bg(t.accent_active)),
            ButtonVariant::Secondary => el
                .themed(|t: &Theme, s| s.bg(t.surface_raised).border(1.0, t.border))
                .hover_themed(|t, s| s.bg(t.element))
                .active_themed(|t, s| s.bg(t.element_hover)),
            ButtonVariant::Ghost => el
                // Transparent base (alpha 0 of the hover color) so the
                // hover fade animates instead of snapping None -> Some.
                .themed(|t: &Theme, s| s.bg(t.element.with_alpha(0.0)))
                .hover_themed(|t, s| s.bg(t.element))
                .active_themed(|t, s| s.bg(t.element_hover)),
            ButtonVariant::Danger => el
                .themed(|t: &Theme, s| {
                    s.bg(t.danger.solid)
                        .highlight_top(t.on_accent.with_alpha(0.14))
                })
                .hover_themed(|t, s| s.bg(t.danger.solid_hover))
                .active_themed(|t, s| s.bg(t.danger.solid_active)),
        };
        if b.disabled {
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
        let icon = b.icon.themed(move |t: &Theme, s| match variant {
            ButtonVariant::Primary | ButtonVariant::Danger => s.color(t.on_accent),
            ButtonVariant::Secondary => s.color(t.text),
            ButtonVariant::Ghost => s.color(t.text_muted),
        });
        let side = b.size.height();
        let mut el = row()
            .items_center()
            .justify_center()
            .w(side)
            .h(side)
            .rounded(R_MD)
            .shrink0()
            .children([icon])
            .transition(Transition::colors())
            .focusable(true)
            .cursor(Cursor::Pointer)
            .disabled(b.disabled)
            .semantics(Semantics::Button);
        if let Some(label) = b.label {
            el = el.label(label);
        }

        el = match variant {
            ButtonVariant::Primary => el
                .themed(|t: &Theme, s| s.bg(t.accent).highlight_top(t.on_accent.with_alpha(0.14)))
                .hover_themed(|t, s| s.bg(t.accent_hover))
                .active_themed(|t, s| s.bg(t.accent_active)),
            ButtonVariant::Secondary => el
                .themed(|t: &Theme, s| s.bg(t.surface_raised).border(1.0, t.border))
                .hover_themed(|t, s| s.bg(t.element))
                .active_themed(|t, s| s.bg(t.element_hover)),
            ButtonVariant::Ghost => el
                .themed(|t: &Theme, s| s.bg(t.element.with_alpha(0.0)))
                .hover_themed(|t, s| s.bg(t.element))
                .active_themed(|t, s| s.bg(t.element_hover)),
            ButtonVariant::Danger => el
                .themed(|t: &Theme, s| {
                    s.bg(t.danger.solid)
                        .highlight_top(t.on_accent.with_alpha(0.14))
                })
                .hover_themed(|t, s| s.bg(t.danger.solid_hover))
                .active_themed(|t, s| s.bg(t.danger.solid_active)),
        };
        if b.disabled {
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
