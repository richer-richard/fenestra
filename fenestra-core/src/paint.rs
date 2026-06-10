//! M0 placeholder painting. Replaced by the element-tree painter in M1; kept
//! minimal so the hello example and the headless smoke test render the exact
//! same scene.

use kurbo::{Affine, Rect, RoundedRect, Stroke, Vec2};
use peniko::{Color, Fill};
use vello::Scene;

use crate::Theme;

/// Paints the M0 hello scene in logical coordinates: the theme background
/// plus one card-like rounded rect with a two-layer Md shadow and a 1px
/// border, centered in `width` x `height`.
pub fn paint_hello(scene: &mut Scene, theme: &Theme, width: f64, height: f64) {
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        theme.bg,
        None,
        &Rect::new(0.0, 0.0, width, height),
    );

    let (cx, cy) = (width / 2.0, height / 2.0);
    let card = Rect::new(cx - 160.0, cy - 100.0, cx + 160.0, cy + 100.0);
    let radius = 14.0;

    // Md shadow token: dy/blur/alpha layers (2, 4, 0.05) + (4, 12, 0.08).
    // Provisional blur -> std_dev mapping of blur/2; calibrated in M1.
    for (dy, blur, alpha) in [(2.0, 4.0, 0.05_f32), (4.0, 12.0, 0.08)] {
        scene.draw_blurred_rounded_rect(
            Affine::IDENTITY,
            card + Vec2::new(0.0, dy),
            Color::new([0.0, 0.0, 0.0, alpha]),
            radius,
            blur / 2.0,
        );
    }

    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        theme.surface_raised,
        None,
        &RoundedRect::from_rect(card, radius),
    );

    // 1px border stroked on the half-pixel so it stays crisp.
    scene.stroke(
        &Stroke::new(1.0),
        Affine::IDENTITY,
        theme.border,
        None,
        &RoundedRect::from_rect(card.inset(-0.5), radius - 0.5),
    );
}
