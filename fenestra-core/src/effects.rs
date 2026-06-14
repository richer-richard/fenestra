//! Generated effect fields: deterministic RGBA8 textures for the "bespoke" end
//! of the design system. The inputs (size, color points, seed, intensity)
//! tokenize; the output is a pixel buffer you hand to
//! [`image_rgba8`](crate::image_rgba8). Pure and deterministic, so these
//! golden-lock like everything else in fenestra.
//!
//! - [`mesh`] — a multi-point mesh gradient (the Stripe "liquid light" field),
//!   blended in OKLab so it stays vivid with no gray dead-zone.
//! - [`grain`] — fine film grain from a seeded PRNG, to break up banding and
//!   add a tactile paper texture.
//!
//! The third common effect-family member, a scroll-edge fade, needs no new
//! primitive: a `linear_gradient` from the chrome's surface color to its
//! transparent twin is the fade where content scrolls under a floating toolbar.

use peniko::Color;

/// One control point of a [`mesh`] gradient: a position in unit coordinates
/// (`0.0..=1.0`, origin top-left) and the color radiating from it.
#[derive(Debug, Clone, Copy)]
pub struct MeshPoint {
    /// X in unit coordinates (0 = left edge, 1 = right edge).
    pub x: f32,
    /// Y in unit coordinates (0 = top edge, 1 = bottom edge).
    pub y: f32,
    /// The color at this point. Source it from a theme token, never a raw hex.
    pub color: Color,
}

/// Rounds a `0.0..=1.0` channel to a `u8` (clamped, so out-of-range is safe).
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is rounded and clamped to 0.0..=255.0 before the cast"
)]
fn channel(v: f32) -> u8 {
    (v * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Normalizes pixel index `i` to a unit coordinate at the pixel center.
#[expect(clippy::cast_precision_loss, reason = "image dimensions fit in f32")]
fn unit(i: usize, n: usize) -> f32 {
    (i as f32 + 0.5) / n as f32
}

/// A `width`×`height` RGBA8 mesh-gradient field: every pixel is an
/// inverse-distance-weighted blend of `points`, blended in OKLab (the
/// perceptual Cartesian space — no hue-wraparound, no gray dead-zone through
/// the middle), then gamut-mapped to sRGB. The Stripe "liquid light" look as a
/// static, deterministic, token-colored texture. Opaque; an empty `points`
/// yields a transparent field. Hand the buffer to
/// [`image_rgba8`](crate::image_rgba8).
#[must_use]
pub fn mesh(width: u32, height: u32, points: &[MeshPoint]) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let mut out = vec![0u8; w * h * 4];
    if points.is_empty() {
        return out;
    }
    // Each point as (x, y, L, a, b) — OKLab is OKLCH's Cartesian form.
    let pts: Vec<(f32, f32, f32, f32, f32)> = points
        .iter()
        .map(|p| {
            let [l, c, hue] = crate::theme::oklch_of(p.color);
            let (sin, cos) = hue.to_radians().sin_cos();
            (p.x, p.y, l, c * cos, c * sin)
        })
        .collect();
    for py in 0..h {
        for px in 0..w {
            let (u, v) = (unit(px, w), unit(py, h));
            let (mut sl, mut sa, mut sb, mut sw) = (0.0_f32, 0.0, 0.0, 0.0);
            for &(x, y, l, a, b) in &pts {
                let (dx, dy) = (u - x, v - y);
                let weight = 1.0 / (dx * dx + dy * dy + 1e-4); // inverse-distance²
                sl += l * weight;
                sa += a * weight;
                sb += b * weight;
                sw += weight;
            }
            let (l, a, b) = (sl / sw, sa / sw, sb / sw);
            let color = crate::theme::oklch(l, (a * a + b * b).sqrt(), b.atan2(a).to_degrees());
            let [r, g, bl, _] = color.components;
            let i = (py * w + px) * 4;
            out[i] = channel(r);
            out[i + 1] = channel(g);
            out[i + 2] = channel(bl);
            out[i + 3] = 255;
        }
    }
    out
}

/// A `width`×`height` RGBA8 film-grain overlay: fine monochrome value noise from
/// a seeded PRNG — deterministic, so the same `seed` always yields the same
/// texture — at `intensity` (`0.0..=1.0`) alpha. Overlay it over a flat fill or
/// a gradient to break up banding and add a tactile paper grain. Each pixel is
/// gray at the noise value with `intensity` alpha; `intensity` `0.0` is fully
/// transparent.
#[must_use]
pub fn grain(width: u32, height: u32, seed: u64, intensity: f32) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let mut out = vec![0u8; w * h * 4];
    let alpha = channel(intensity.clamp(0.0, 1.0));
    // xorshift64* — a tiny deterministic PRNG (no `rand` dep, no clock/random).
    let mut state = seed ^ 0x9E37_79B9_7F4A_7C15;
    for px in out.chunks_exact_mut(4) {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        // `>> 56` leaves only the top 8 bits, so the cast to u8 is exact.
        let v = (state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 56) as u8;
        px[0] = v;
        px[1] = v;
        px[2] = v;
        px[3] = alpha;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(buf: &[u8], w: usize, x: usize, y: usize) -> [u8; 4] {
        let i = (y * w + x) * 4;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    }

    #[test]
    fn mesh_is_opaque_and_correctly_sized() {
        let p = [MeshPoint {
            x: 0.2,
            y: 0.2,
            color: crate::theme::oklch(0.6, 0.15, 30.0),
        }];
        let buf = mesh(8, 8, &p);
        assert_eq!(buf.len(), 8 * 8 * 4);
        assert!(buf.chunks_exact(4).all(|c| c[3] == 255), "opaque");
    }

    #[test]
    fn mesh_with_one_point_is_that_color() {
        // One point ⇒ uniform field of (approximately) that color everywhere.
        let c = crate::theme::oklch(0.55, 0.16, 262.0);
        let buf = mesh(
            6,
            6,
            &[MeshPoint {
                x: 0.5,
                y: 0.5,
                color: c,
            }],
        );
        let got = px(&buf, 6, 3, 3);
        let want = c.to_rgba8();
        for ch in 0..3 {
            let d = i32::from(got[ch]) - i32::from([want.r, want.g, want.b][ch]);
            assert!(d.abs() <= 2, "channel {ch}: {got:?} vs {want:?}");
        }
    }

    #[test]
    fn mesh_empty_is_transparent() {
        assert!(mesh(4, 4, &[]).iter().all(|&b| b == 0));
    }

    #[test]
    fn grain_is_deterministic_and_seed_sensitive() {
        let a = grain(16, 16, 42, 0.5);
        assert_eq!(a.len(), 16 * 16 * 4);
        assert_eq!(a, grain(16, 16, 42, 0.5), "same seed ⇒ same texture");
        assert_ne!(a, grain(16, 16, 43, 0.5), "different seed ⇒ different");
        // Alpha is the intensity; the noise lives in the gray channels.
        assert!(
            a.chunks_exact(4)
                .all(|c| c[3] == 128 && c[0] == c[1] && c[1] == c[2])
        );
    }

    #[test]
    fn grain_zero_intensity_is_transparent() {
        assert!(grain(8, 8, 1, 0.0).chunks_exact(4).all(|c| c[3] == 0));
    }
}
