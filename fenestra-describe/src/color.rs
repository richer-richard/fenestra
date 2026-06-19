//! Resolve a [`ColorSpec`] against a theme. Role names map to the theme's
//! semantic color fields; the `oklch` escape hatch builds a color directly.
//! Raw hex never enters the boundary — colors come from the theme or OKLCH.

use fenestra_core::{Color, Theme, oklch};

use crate::error::DescribeError;
use crate::format::ColorSpec;

/// Every theme role name a [`ColorSpec::Role`] may reference. Kept in lockstep
/// with [`role_color`] (a test asserts each one resolves), and surfaced by
/// `describe_vocabulary` so authors see the whole palette.
pub const COLOR_ROLES: &[&str] = &[
    "bg",
    "surface",
    "surface_raised",
    "element",
    "element_hover",
    "element_active",
    "border_subtle",
    "border",
    "border_strong",
    "text",
    "text_muted",
    "text_subtle",
    "text_disabled",
    "accent",
    "accent_hover",
    "accent_active",
    "accent_bg",
    "accent_border",
    "accent_text",
    "on_accent",
    "danger",
    "warning",
    "success",
];

/// Resolves `spec` to a concrete color against `theme`.
///
/// # Errors
/// A [`DescribeError`] when a role name is not one of [`COLOR_ROLES`], or when an
/// `oklch` triple is out of range (lightness outside `0..=1`, negative chroma, or
/// any non-finite component) — which would otherwise yield a degenerate or `NaN`
/// color the renderer cannot reason about.
pub fn resolve_color(spec: &ColorSpec, theme: &Theme) -> Result<Color, DescribeError> {
    match spec {
        ColorSpec::Oklch(o) => {
            let [l, c, h] = o.oklch;
            if !(l.is_finite()
                && (0.0..=1.0).contains(&l)
                && c.is_finite()
                && c >= 0.0
                && h.is_finite())
            {
                return Err(DescribeError::new(
                    "color",
                    format!(
                        "oklch out of range: lightness {l} must be in 0..=1, chroma {c} must be \
                         finite and >= 0, hue {h} must be finite"
                    ),
                ));
            }
            Ok(oklch(l, c, h))
        }
        ColorSpec::Role(name) => role_color(name, theme).ok_or_else(|| {
            DescribeError::new(
                "color",
                format!(
                    "unknown color role {name:?}; valid roles: {}",
                    COLOR_ROLES.join(", ")
                ),
            )
        }),
    }
}

/// Maps a role name to the theme field, or `None` if unknown. Status roles
/// resolve to their solid fill.
fn role_color(name: &str, theme: &Theme) -> Option<Color> {
    Some(match name {
        "bg" => theme.bg,
        "surface" => theme.surface,
        "surface_raised" => theme.surface_raised,
        "element" => theme.element,
        "element_hover" => theme.element_hover,
        "element_active" => theme.element_active,
        "border_subtle" => theme.border_subtle,
        "border" => theme.border,
        "border_strong" => theme.border_strong,
        "text" => theme.text,
        "text_muted" => theme.text_muted,
        "text_subtle" => theme.text_subtle,
        "text_disabled" => theme.text_disabled,
        "accent" => theme.accent,
        "accent_hover" => theme.accent_hover,
        "accent_active" => theme.accent_active,
        "accent_bg" => theme.accent_bg,
        "accent_border" => theme.accent_border,
        "accent_text" => theme.accent_text,
        "on_accent" => theme.on_accent,
        "danger" => theme.danger.solid,
        "warning" => theme.warning.solid,
        "success" => theme.success.solid,
        _ => return None,
    })
}
