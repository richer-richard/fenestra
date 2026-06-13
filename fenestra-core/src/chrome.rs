//! The editor-chrome token tier: the dense panel/inspector register that
//! desktop creative tools (Figma's anatomy) use — 11–14px text and a flat,
//! layered panel-shadow vocabulary — distinct from the product token scale.
//! Pairs with the [`canvas`](crate::canvas) substrate to make "build a
//! Figma-class tool in fenestra" a real story. Values follow Figma's published
//! plugin design system (the de-facto mirror of its editor chrome).

use peniko::Color;

use crate::style::Shadow;

/// Editor-chrome text sizes — the dense panel type, 11–14px, distinct from the
/// product [`TextSize`](crate::tokens::TextSize) scale (which starts at 12px
/// for reading text). 11px is the base for nearly all panel/inspector text.
/// Apply through the free-form size override:
/// `text(..).size_px(ChromeText::Sm.px()).tracking(ChromeText::Sm.tracking())`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ChromeText {
    /// 11px — the base panel/inspector text (labels, fields, layer names).
    #[default]
    Sm,
    /// 12px — secondary panel text.
    Md,
    /// 13px — section titles.
    Lg,
    /// 14px — panel emphasis / headings.
    Xl,
}

impl ChromeText {
    /// Font size in logical pixels.
    pub const fn px(self) -> f32 {
        match self {
            Self::Sm => 11.0,
            Self::Md => 12.0,
            Self::Lg => 13.0,
            Self::Xl => 14.0,
        }
    }

    /// Letter spacing in em — Figma's per-size tracking: a hair positive at
    /// 11px, tightening as size grows.
    pub const fn tracking(self) -> f32 {
        match self {
            Self::Sm => 0.005,
            Self::Md => 0.0,
            Self::Lg => -0.0025,
            Self::Xl => -0.001,
        }
    }

    /// Line height as a multiple of the size — Figma's fixed line boxes (16px
    /// for 11/12px text, 24px for 13/14px) expressed as a ratio.
    pub fn line_height(self) -> f32 {
        let lh = if matches!(self, Self::Sm | Self::Md) {
            16.0
        } else {
            24.0
        };
        lh / self.px()
    }
}

/// The editor-chrome elevation vocabulary — Figma's flat, layered panel
/// shadows: two soft black drops over a 0.5px hairline ring. Flat and
/// mode-independent (true black at low alpha), in deliberate contrast to the
/// hue-tinted, themed [`ShadowToken`](crate::tokens::ShadowToken) used for
/// product surfaces. The ring is a 0.5px shadow *spread* (no blur), which the
/// painter renders as a crisp sub-pixel edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChromeElevation {
    /// Floating menus, dropdowns, popovers.
    Popover,
    /// Modal dialogs.
    Modal,
    /// Slider thumbs and small floating handles.
    Thumb,
}

fn black(alpha: f32) -> Color {
    Color::new([0.0, 0.0, 0.0, alpha])
}

/// A 0.5px hairline ring at `alpha` (a zero-blur spread shadow).
fn ring(alpha: f32) -> Shadow {
    Shadow {
        dx: 0.0,
        dy: 0.0,
        blur: 0.0,
        spread: 0.5,
        color: black(alpha),
    }
}

/// A soft black drop shadow.
fn drop(dy: f32, blur: f32, alpha: f32) -> Shadow {
    Shadow {
        dx: 0.0,
        dy,
        blur,
        spread: 0.0,
        color: black(alpha),
    }
}

impl ChromeElevation {
    /// The concrete shadow layers, painted bottom-up (drops first, then the
    /// hairline ring closest to the surface). Drop these straight onto a
    /// [`Style`](crate::Style) via its `shadows` field.
    pub fn shadows(self) -> Vec<Shadow> {
        match self {
            Self::Popover => vec![drop(5.0, 17.0, 0.20), drop(2.0, 7.0, 0.15), ring(0.10)],
            Self::Modal => vec![drop(2.0, 14.0, 0.15), ring(0.20)],
            Self::Thumb => vec![drop(1.0, 3.0, 0.10), drop(3.0, 8.0, 0.10), ring(0.18)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_text_is_the_dense_11_to_14_register() {
        assert_eq!(ChromeText::Sm.px(), 11.0);
        assert_eq!(ChromeText::Xl.px(), 14.0);
        // Tracking tightens as size grows (positive at 11px, negative by 13px).
        assert!(ChromeText::Sm.tracking() > 0.0);
        assert!(ChromeText::Lg.tracking() < 0.0);
        // 11px sits on a 16px line box.
        assert!((ChromeText::Sm.line_height() * 11.0 - 16.0).abs() < 1e-4);
    }

    #[test]
    fn chrome_elevation_is_two_drops_plus_a_hairline_ring() {
        let pop = ChromeElevation::Popover.shadows();
        assert_eq!(pop.len(), 3);
        // The last layer is the 0.5px ring (zero blur, 0.5 spread).
        let r = pop[2];
        assert_eq!(r.blur, 0.0);
        assert!((r.spread - 0.5).abs() < 1e-6);
        // The drops are blurred and offset downward.
        assert!(pop[0].blur > 0.0 && pop[0].dy > 0.0);
        // All layers are flat black (no hue), unlike themed shadows.
        for s in pop {
            let [r, g, b, _] = s.color.components;
            assert!(r == 0.0 && g == 0.0 && b == 0.0);
        }
        assert_eq!(ChromeElevation::Modal.shadows().len(), 2);
    }
}
