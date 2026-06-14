//! Surface / Material bundle: one typed primitive per elevation role.
//!
//! A [`Surface`] is a semantic *material* (Geist/Apple "materials"): it bundles
//! a corner radius, a fill role, a border role, a shadow token, and an optional
//! top highlight into a single value that resolves against a [`Theme`] into a
//! [`Style`] overlay. The kit's elevated surfaces — cards, popovers, menus,
//! modals, the slider thumb, tooltips — all derive their look from this one
//! table, so "every floating thing matches" is a structural property of the
//! bundle, not a convention re-typed at each call site.

use crate::style::{Border, CornerRadius, Paint, Style};
use crate::theme::Theme;
use crate::tokens::{R_FULL, R_LG, R_SM, R_XL, ShadowToken};

/// One of the kit's elevation *materials*: a semantic role that bundles a
/// corner radius, a fill role, a border role, a shadow token, and an optional
/// top highlight into one typed primitive, resolved against a [`Theme`] into a
/// [`Style`] overlay. Floating roles (`Popover`/`Menu`/`Modal`) carry radii
/// `>=` and shadow depth `>=` the resting roles (`Card`/`Raised`), so "every
/// floating thing matches" is structural, not a convention re-typed at each
/// call site. `Thumb` (a control handle) and `Tooltip` (an inverted chip) are
/// deliberately exempt from that ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Surface {
    /// A resting card: raised surface, subtle border, large radius, a small
    /// shadow.
    Card,
    /// A resting panel with the card's surface and radius but no shadow — a
    /// flat grouping that still reads as a distinct surface.
    Raised,
    /// A floating popover anchored to the page: elevated fill, deep shadow.
    Popover,
    /// A floating menu/listbox: same recipe as [`Surface::Popover`].
    Menu,
    /// A floating modal dialog: the deepest, roundest surface.
    Modal,
    /// A control handle (slider thumb): a pill on a raised surface with a firm
    /// border and a small shadow. Exempt from the elevation ordering.
    Thumb,
    /// An inverted high-contrast chip (tooltip): the neutral ramp's darkest
    /// step, a tight radius, no border. Exempt from the elevation ordering.
    Tooltip,
}

/// The corner-radius shape of a surface bundle: one `Uniform` radius on every
/// corner. A nested rounded child derives its concentric radius from this via
/// [`SurfaceRadius::inner`] (passing the padding between them). The enum is
/// `#[non_exhaustive]` to leave room for future shapes (e.g. per-corner radii).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum SurfaceRadius {
    /// The same radius on every corner.
    Uniform(f32),
}

impl SurfaceRadius {
    /// The outer corner radius in logical px.
    pub const fn outer(self) -> f32 {
        match self {
            SurfaceRadius::Uniform(r) => r,
        }
    }

    /// The concentric inner radius for a child inset by `inset` logical px on
    /// every side: `max(0, outer - inset)`. Nested rounded rectangles must
    /// share a center, so the child's radius is the parent's outer radius
    /// minus the padding between them — otherwise the inner corner visibly
    /// bulges away from the outer one. Use this for any rounded child of a
    /// rounded surface (menu items inside a menu panel, an inset thumbnail in
    /// a card) instead of hand-typing a second radius that can silently desync
    /// when the surface radius changes.
    pub fn inner(self, inset: f32) -> f32 {
        (self.outer() - inset).max(0.0)
    }
}

/// Which theme role a surface fill resolves to — a bundle is defined in
/// semantic roles, never literal colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SurfaceFill {
    /// [`Theme::surface_raised`] — resting cards and control surfaces.
    SurfaceRaised,
    /// [`Theme::elevated_surface`] at the given level — floating panels (lifts
    /// in dark mode).
    Elevated(u8),
    /// `Theme::neutrals.step(12)` — the inverted high-contrast chip (tooltips).
    Inverted,
}

impl SurfaceFill {
    /// Resolves this fill role to a concrete color for `theme`.
    pub fn color(self, theme: &Theme) -> crate::Color {
        match self {
            SurfaceFill::SurfaceRaised => theme.surface_raised,
            SurfaceFill::Elevated(level) => theme.elevated_surface(level),
            SurfaceFill::Inverted => theme.neutrals.step(12),
        }
    }
}

/// The border role of a surface bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceBorder {
    /// No border.
    None,
    /// 1px [`Theme::border_subtle`] — the hairline that pairs with elevation.
    Subtle,
    /// 1px [`Theme::border`] — a firmer edge for resting control surfaces
    /// (thumbs).
    Default,
}

/// The role-resolved description of a [`Surface`]: radius, fill/border *roles*,
/// shadow token, and optional top-highlight alpha. [`Surface::bundle`] returns
/// it (pure, no theme — so radius/shadow/role ordering is unit-testable);
/// [`SurfaceBundle::apply`] turns it into a concrete [`Style`]. Public so apps
/// and Looks can compose custom materials from the fill/border roles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceBundle {
    /// Outer corner radius.
    pub radius: SurfaceRadius,
    /// Fill role.
    pub fill: SurfaceFill,
    /// Border role.
    pub border: SurfaceBorder,
    /// Shadow elevation token (`None` for a flat resting surface).
    pub shadow: Option<ShadowToken>,
    /// 1px top-highlight alpha (white sheen), or `None` for no highlight.
    pub highlight: Option<f32>,
}

impl Surface {
    /// The role's bundle: the radius, fill/border roles, shadow token, and
    /// highlight that define this material. Pure and `const`.
    pub const fn bundle(self) -> SurfaceBundle {
        match self {
            // Resting roles: the card's surface and 14px radius.
            Surface::Card => SurfaceBundle {
                radius: SurfaceRadius::Uniform(R_LG),
                fill: SurfaceFill::SurfaceRaised,
                border: SurfaceBorder::Subtle,
                shadow: Some(ShadowToken::Sm),
                highlight: None,
            },
            Surface::Raised => SurfaceBundle {
                radius: SurfaceRadius::Uniform(R_LG),
                fill: SurfaceFill::SurfaceRaised,
                border: SurfaceBorder::Subtle,
                shadow: None,
                highlight: None,
            },
            // Floating roles: elevated fill, deep shadow, radius >= the card's.
            Surface::Popover | Surface::Menu => SurfaceBundle {
                radius: SurfaceRadius::Uniform(R_LG),
                fill: SurfaceFill::Elevated(2),
                border: SurfaceBorder::Subtle,
                shadow: Some(ShadowToken::Lg),
                highlight: None,
            },
            Surface::Modal => SurfaceBundle {
                radius: SurfaceRadius::Uniform(R_XL),
                fill: SurfaceFill::Elevated(2),
                border: SurfaceBorder::Subtle,
                shadow: Some(ShadowToken::Xl),
                highlight: None,
            },
            // Exempt materials: a pill control handle and an inverted chip.
            Surface::Thumb => SurfaceBundle {
                radius: SurfaceRadius::Uniform(R_FULL),
                fill: SurfaceFill::SurfaceRaised,
                border: SurfaceBorder::Default,
                shadow: Some(ShadowToken::Sm),
                highlight: None,
            },
            Surface::Tooltip => SurfaceBundle {
                radius: SurfaceRadius::Uniform(R_SM),
                fill: SurfaceFill::Inverted,
                border: SurfaceBorder::None,
                shadow: Some(ShadowToken::Md),
                highlight: None,
            },
        }
    }

    /// Whether this role floats above the page (`Popover`/`Menu`/`Modal`).
    /// `Card` and `Raised` rest in flow; `Thumb` and `Tooltip` are exempt
    /// control/chip materials (both report `false`).
    pub const fn is_floating(self) -> bool {
        matches!(self, Surface::Popover | Surface::Menu | Surface::Modal)
    }
}

impl SurfaceBundle {
    /// Overlays this bundle onto `base`, resolving fill/border against `theme`:
    /// sets corner radius, solid fill, border, shadow token, and top highlight,
    /// leaving every other field of `base` (layout, text) untouched. The
    /// highlight, when set, is a low-alpha pure white from `oklch(1, 0, 0)` — no
    /// raw literal. The shadow token expands to layers later, during frame
    /// resolution, exactly as a hand-set `.shadow(..)` would.
    #[must_use]
    pub fn apply(self, theme: &Theme, base: Style) -> Style {
        let mut style = base;
        style.corner_radius = CornerRadius::all(self.radius.outer());
        style.fill = Some(Paint::Solid(self.fill.color(theme)));
        style.border = match self.border {
            SurfaceBorder::None => None,
            SurfaceBorder::Subtle => Some(Border {
                width: 1.0,
                color: theme.border_subtle,
            }),
            SurfaceBorder::Default => Some(Border {
                width: 1.0,
                color: theme.border,
            }),
        };
        style.shadow_token = self.shadow;
        style.highlight_top = self
            .highlight
            .map(|a| crate::theme::oklch(1.0, 0.0, 0.0).with_alpha(a));
        style
    }
}

impl Theme {
    /// A fresh [`Style`] carrying `role`'s material (radius + fill + border +
    /// shadow + highlight) resolved for this theme. The theme-in-scope entry
    /// point; [`Element::surface`](crate::Element::surface) is the deferred one
    /// for `view()`.
    pub fn surface_style(&self, role: Surface) -> Style {
        role.bundle().apply(self, Style::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Border, CornerRadius, Edges, Paint};
    use crate::tokens::{R_FULL, R_LG, R_MD, R_XL, SP1, ShadowToken};

    #[test]
    fn card_bundle_is_resting_raised() {
        // Asserts role identity, not colors.
        let b = Surface::Card.bundle();
        assert_eq!(b.radius, SurfaceRadius::Uniform(R_LG));
        assert_eq!(b.fill, SurfaceFill::SurfaceRaised);
        assert_eq!(b.border, SurfaceBorder::Subtle);
        assert_eq!(b.shadow, Some(ShadowToken::Sm));
        assert_eq!(b.highlight, None);
    }

    #[test]
    fn floating_roles_use_elevated_fill_and_deep_shadows() {
        // Locks the floating recipe.
        for role in [Surface::Popover, Surface::Menu] {
            assert_eq!(role.bundle().fill, SurfaceFill::Elevated(2));
            assert_eq!(role.bundle().shadow, Some(ShadowToken::Lg));
        }
        let m = Surface::Modal.bundle();
        assert_eq!(m.fill, SurfaceFill::Elevated(2));
        assert_eq!(m.shadow, Some(ShadowToken::Xl));
        assert_eq!(m.radius.outer(), R_XL);
    }

    #[test]
    fn ordering_invariant_floating_ge_resting() {
        // The acceptance invariant: every floating role is at least as round
        // and at least as deep as every resting role. Thumb and Tooltip are
        // excluded — they are exempt control/chip materials, not on the ladder.
        for f in [Surface::Popover, Surface::Menu, Surface::Modal] {
            for r in [Surface::Card, Surface::Raised] {
                assert!(
                    f.bundle().radius.outer() >= r.bundle().radius.outer(),
                    "{f:?} radius must be >= {r:?}"
                );
                assert!(
                    f.bundle().shadow >= r.bundle().shadow,
                    "{f:?} shadow must be >= {r:?}"
                );
            }
        }
    }

    #[test]
    fn resolve_uses_theme_roles_not_literals() {
        // Proves role-derivation across both modes.
        for t in [Theme::light(), Theme::dark()] {
            let card = Surface::Card.bundle().apply(&t, Style::default());
            assert_eq!(card.fill, Some(Paint::Solid(t.surface_raised)));
            assert_eq!(
                card.border,
                Some(Border {
                    width: 1.0,
                    color: t.border_subtle,
                })
            );
            assert_eq!(card.shadow_token, Some(ShadowToken::Sm));
            assert_eq!(card.corner_radius, CornerRadius::all(R_LG));

            let menu = Surface::Menu.bundle().apply(&t, Style::default());
            assert_eq!(menu.fill, Some(Paint::Solid(t.elevated_surface(2))));

            let tip = Surface::Tooltip.bundle().apply(&t, Style::default());
            assert_eq!(tip.fill, Some(Paint::Solid(t.neutrals.step(12))));
            assert_eq!(tip.border, None);
        }
    }

    #[test]
    fn highlight_resolves_to_low_alpha_white() {
        // Exercises the highlight path even though no shipped role sets it.
        let t = Theme::light();
        let bundle = SurfaceBundle {
            highlight: Some(0.14),
            ..Surface::Card.bundle()
        };
        let s = bundle.apply(&t, Style::default());
        let c = s.highlight_top.expect("highlight set");
        let [r, g, b, a] = c.components;
        assert!((a - 0.14).abs() < 1e-4, "alpha {a}");
        assert!(
            (r - g).abs() < 1e-3 && (g - b).abs() < 1e-3,
            "achromatic: {r} {g} {b}"
        );
        assert!(r > 0.9, "near-white: {r}");
    }

    #[test]
    fn thumb_is_a_pill_with_a_firm_border() {
        let b = Surface::Thumb.bundle();
        assert_eq!(b.radius.outer(), R_FULL);
        assert_eq!(b.border, SurfaceBorder::Default);
        assert_eq!(b.shadow, Some(ShadowToken::Sm));
    }

    #[test]
    fn surface_radius_is_concentric_ready() {
        // Documents the forward-compat slot.
        assert!(matches!(
            Surface::Card.bundle().radius,
            SurfaceRadius::Uniform(_)
        ));
    }

    #[test]
    fn concentric_inner_is_outer_minus_inset() {
        // The concentric accessor: a child inset by `inset` derives its radius
        // as `max(0, outer - inset)`.
        assert_eq!(SurfaceRadius::Uniform(14.0).inner(4.0), 10.0);
        // Clamp floor: a child padded more than the outer radius is square.
        assert_eq!(SurfaceRadius::Uniform(2.0).inner(8.0), 0.0);
    }

    #[test]
    fn menu_item_radius_is_concentric_with_panel() {
        // Acceptance: a menu item inset by SP1 derives the panel-concentric
        // radius, which equals the old hand-typed `R_LG - 4.0` == R_MD.
        let r = Surface::Menu.bundle().radius;
        assert_eq!(r.inner(SP1), r.outer() - SP1);
        assert_eq!(r.inner(SP1), R_MD);
    }

    #[test]
    fn theme_surface_style_equals_bundle_apply() {
        let t = Theme::light();
        assert_eq!(
            t.surface_style(Surface::Modal),
            Surface::Modal.bundle().apply(&t, Style::default())
        );
    }

    #[test]
    fn surface_apply_preserves_layout() {
        let t = Theme::light();
        // apply overlays the material (fill/radius/etc.) while leaving layout
        // and text fields of the base style untouched.
        let s = Surface::Card.bundle().apply(&t, Style::default().p(8.0));
        assert_eq!(s.padding, Edges::all(8.0));
        assert!(s.fill.is_some());
    }
}
