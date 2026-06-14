//! Optical adjustments: the small geometric corrections that make shapes *look*
//! right even though they measure "wrong". The eye weighs the area near a
//! shape's boundary, so a circle must be slightly larger than a square to read
//! as the same size, and an asymmetric shape (a play triangle) must be centered
//! on its visual mass (centroid), not its bounding box. (bjango optical
//! adjustments; Rauno's interface guidelines.)

/// How much larger a circle — or any round / pointed shape — must be than a
/// square to read as the *same* visual size: ~12.84%. A circle inscribed in a
/// square looks smaller than the square because the eye compares boundary area;
/// scaling the circle's diameter by this ratio against an adjacent square
/// equalizes the perceived size. Apply to a circular icon sitting beside square
/// ones (e.g. a round avatar next to square thumbnails).
#[expect(
    clippy::approx_constant,
    reason = "the empirical bjango optical-overshoot ratio ~112.84%; coincidentally near 2/sqrt(pi), not derived from it"
)]
pub const CIRCLE_OVERSHOOT: f32 = 1.1284;

/// `size` scaled by [`CIRCLE_OVERSHOOT`] — the diameter a circle needs to match
/// the visual size of a square of edge `size`.
#[must_use]
pub fn overshoot(size: f32) -> f32 {
    size * CIRCLE_OVERSHOOT
}

/// The centroid (visual-mass center) of a polygon's `vertices` — the mean of
/// the vertex coordinates. Center an asymmetric shape on its centroid rather
/// than its bounding-box center: a right-pointing play triangle's mass sits
/// toward its flat edge, so bounding-box centering leaves it looking shifted
/// toward the point. Translating the shape so its centroid lands at the target
/// center is the optical correction (the classic "nudge the play triangle
/// right"). Returns `(0.0, 0.0)` for an empty slice.
#[must_use]
pub fn centroid(vertices: &[(f32, f32)]) -> (f32, f32) {
    if vertices.is_empty() {
        return (0.0, 0.0);
    }
    let (sx, sy) = vertices
        .iter()
        .fold((0.0_f32, 0.0_f32), |(ax, ay), &(x, y)| (ax + x, ay + y));
    #[expect(clippy::cast_precision_loss, reason = "vertex counts are tiny")]
    let n = vertices.len() as f32;
    (sx / n, sy / n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overshoot_is_the_circle_square_ratio() {
        // ~112.84% — the bjango circle-vs-square optical overshoot.
        assert!((overshoot(100.0) - 112.84).abs() < 0.05);
        // A circle the same nominal size as a square reads smaller, so the
        // corrected diameter is strictly larger.
        assert!(overshoot(24.0) > 24.0);
    }

    #[test]
    fn centroid_of_a_play_triangle_is_left_of_its_bbox_center() {
        // A right-pointing triangle in the unit box: centroid x = (0+0+1)/3 ≈
        // 0.333, left of the bbox center 0.5 — so centering on the centroid
        // shifts it RIGHT, the optical play-button correction.
        let tri = [(0.0, 0.0), (0.0, 1.0), (1.0, 0.5)];
        let (cx, cy) = centroid(&tri);
        assert!((cx - 1.0 / 3.0).abs() < 1e-6, "cx {cx}");
        assert!((cy - 0.5).abs() < 1e-6, "cy {cy}");
        assert!(cx < 0.5, "centroid left of bbox center");
    }

    #[test]
    fn centroid_empty_is_origin() {
        assert_eq!(centroid(&[]), (0.0, 0.0));
    }
}
