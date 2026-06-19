//! APCA (Accessible Perceptual Contrast Algorithm) lightness contrast, `Lc`.
//!
//! APCA models text legibility far better than the WCAG 2.x contrast ratio:
//! it is polarity-aware (dark-on-light reads differently from light-on-dark)
//! and tuned to perceived contrast rather than a luminance quotient. fenestra
//! uses it to *prove* a theme's text/background pairs are legible — a check no
//! CSS framework can run, since fenestra resolves every color at construction.
//!
//! This is the `0.0.98G-4g` constants set (the `apca-w3` v0.1.9 reference).
//! `Lc` is signed: positive for dark text on a lighter background (BoW),
//! negative for light text on a darker background (WoB). The magnitude, 0
//! to ~108, is the perceptual contrast; targets are stated as `Lc` floors
//! (e.g. body text wants `Lc` 75+, ideally 90).

use peniko::Color;

// Constants set "0.0.98G-4g" — verbatim from apca-w3 v0.1.9 src/apca-w3.js.
// https://github.com/Myndex/apca-w3/blob/master/src/apca-w3.js
const MAIN_TRC: f64 = 2.4; // straight gamma, NOT the piecewise sRGB EOTF
const S_RCO: f64 = 0.212_672_9; // R luminance coefficient
const S_GCO: f64 = 0.715_152_2; // G luminance coefficient
const S_BCO: f64 = 0.072_175_0; // B luminance coefficient
const NORM_BG: f64 = 0.56; // normal-polarity (BoW) bg exponent
const NORM_TXT: f64 = 0.57; // normal-polarity (BoW) text exponent
const REV_BG: f64 = 0.65; // reverse-polarity (WoB) bg exponent
const REV_TXT: f64 = 0.62; // reverse-polarity (WoB) text exponent
const BLK_THRS: f64 = 0.022; // black soft-clamp threshold
const BLK_CLMP: f64 = 1.414; // black soft-clamp exponent
const SCALE_BOW: f64 = 1.14; // black-on-white output scale
const SCALE_WOB: f64 = 1.14; // white-on-black output scale
const LO_BOW_OFFSET: f64 = 0.027; // BoW low-contrast offset (subtracted)
const LO_WOB_OFFSET: f64 = 0.027; // WoB low-contrast offset (added)
const DELTA_Y_MIN: f64 = 0.000_5; // min luminance delta before Lc collapses to 0
const LO_CLIP: f64 = 0.1; // low-contrast clip on signed SAPC
const Y_MAX: f64 = 1.1; // input domain upper bound

/// Screen luminance `Y` of an opaque sRGB color, using APCA's straight-2.4
/// estimate (not the piecewise sRGB curve). Alpha is ignored — composite a
/// translucent color over its background before measuring it.
fn srgb_to_y(c: Color) -> f64 {
    let [r, g, b, _a] = c.components;
    let lin = |ch: f32| f64::from(ch).clamp(0.0, 1.0).powf(MAIN_TRC);
    S_RCO * lin(r) + S_GCO * lin(g) + S_BCO * lin(b)
}

/// APCA `Lc` for `text` painted over `bg`. Positive for dark-on-light, negative
/// for light-on-dark; the magnitude (0..~108) is the perceptual contrast.
///
/// Returns `0.0` for out-of-range inputs or pairs below the algorithm's
/// minimum-difference / low-contrast clips, exactly as the reference does.
#[must_use]
pub fn lc(text: Color, bg: Color) -> f64 {
    contrast(srgb_to_y(text), srgb_to_y(bg))
}

/// The magnitude of [`lc`] — the form used by "meets a target" checks, which
/// care about contrast strength, not polarity.
#[must_use]
pub fn lc_abs(text: Color, bg: Color) -> f64 {
    lc(text, bg).abs()
}

/// Whether `text` on `bg` reaches an `Lc` target (by magnitude).
#[must_use]
pub fn meets(text: Color, bg: Color, target_lc: f64) -> bool {
    lc_abs(text, bg) >= target_lc
}

/// WCAG 2 relative luminance of an opaque sRGB color — the piecewise-linear
/// sRGB EOTF (with the `0.03928` toe), distinct from APCA's straight-2.4
/// estimate in [`srgb_to_y`]. Alpha is ignored.
fn wcag_luminance(c: Color) -> f64 {
    let [r, g, b, _a] = c.components;
    let lin = |ch: f32| {
        let u = f64::from(ch).clamp(0.0, 1.0);
        if u <= 0.039_28 {
            u / 12.92
        } else {
            ((u + 0.055) / 1.055).powf(2.4)
        }
    };
    S_RCO * lin(r) + S_GCO * lin(g) + S_BCO * lin(b)
}

/// The WCAG 2 contrast ratio between two opaque colors: `(L1 + 0.05) / (L2 +
/// 0.05)` with the lighter luminance on top. Ranges from `1.0` (identical) to
/// `21.0` (black on white). APCA ([`lc`]) models perception better, but WCAG 2
/// is the shipping legal standard, so the verification surface reports both.
#[must_use]
pub fn wcag2_ratio(a: Color, b: Color) -> f64 {
    let la = wcag_luminance(a);
    let lb = wcag_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Whether `text` on `bg` clears the WCAG 2 AA threshold: `4.5:1` for normal
/// text, `3.0:1` for large text (`large = true`: >= 18pt, or >= 14pt bold).
#[must_use]
pub fn wcag2_passes(text: Color, bg: Color, large: bool) -> bool {
    let threshold = if large { 3.0 } else { 4.5 };
    wcag2_ratio(text, bg) >= threshold
}

/// APCA readability anchors as `(effective px @ weight 400, required Lc)`,
/// strictly decreasing in `Lc` and strictly increasing in px. The weight-400
/// baseline of [`required_lc`]; heavier weights map to a larger effective px.
const APCA_REQ: [(f64, f64); 8] = [
    (12.0, 100.0),
    (14.0, 90.0),
    (16.0, 75.0),
    (18.0, 70.0),
    (24.0, 60.0),
    (36.0, 45.0),
    (72.0, 30.0),
    (300.0, 15.0),
];

/// APCA's maximum `Lc` magnitude (black on white tops out near here); the upper
/// output clamp of [`required_lc`].
const LC_MAX: f64 = 108.0;
/// The lower output clamp of [`required_lc`]: the spot/decorative floor for very
/// large or heavy text.
const LC_SPOT: f64 = 15.0;
/// The weight that [`APCA_REQ`] is tabulated at; the reference of the
/// effective-size weight model.
const WEIGHT_REF: f64 = 400.0;
/// Exponent of the effective-size weight model: `eff = px·(weight/400)^0.5`, so
/// going from 400 to 800 weight scales the effective size by `√2`.
const WEIGHT_EXP: f64 = 0.5;

/// The minimum APCA `Lc` magnitude that text of `size_px` logical pixels at
/// font `weight` (OpenType 100..900) needs to be read fluently — APCA's
/// readability criterion as a function instead of a fixed floor. Smaller and
/// thinner text needs more contrast; larger and heavier text needs less.
///
/// Calibrated to the APCA "in a nutshell" / Readability Criterion (Bronze)
/// anchors: ~Lc 90 for 14px/400, ~75 for 16px/400, ~60 for
/// larger or heavier body text, ~45 for headlines, down to a ~Lc 15
/// spot/decorative floor. Monotonic in both axes; faithful at the anchors,
/// not the full per-weight matrix. APCA is the draft WCAG-3 contrast method
/// (Myndex SAPC-APCA, <https://github.com/Myndex/SAPC-APCA>), consistent with
/// the `apca-w3` reference [`lc`] uses.
///
/// Pair with [`lc_abs`] to check a concrete text/bg pair (see
/// [`Theme::contrast_ok`](crate::Theme::contrast_ok)). `size_px` is clamped to
/// `>= 1.0` and `weight` to `1.0..=1000.0`, so out-of-range inputs are safe.
#[must_use]
pub fn required_lc(size_px: f32, weight: f32) -> f64 {
    let px = f64::from(size_px).max(1.0);
    let w = f64::from(weight).clamp(1.0, 1000.0);
    // Heavier weight ⇒ larger effective px ⇒ lower required Lc.
    let eff = px * (w / WEIGHT_REF).powf(WEIGHT_EXP);
    // Clamp into the tabulated domain so the interpolation always brackets.
    let eff = eff.clamp(APCA_REQ[0].0, APCA_REQ[APCA_REQ.len() - 1].0);
    interp_decreasing(&APCA_REQ, eff).clamp(LC_SPOT, LC_MAX)
}

/// Piecewise-linear interpolation of `x` over a table sorted ascending by its
/// first column. `x` must already lie within `[table[0].0, table[last].0]`
/// (the caller clamps it), so the bracketing segment always exists.
fn interp_decreasing(table: &[(f64, f64)], x: f64) -> f64 {
    for win in table.windows(2) {
        let (x0, y0) = win[0];
        let (x1, y1) = win[1];
        if x <= x1 {
            let t = (x - x0) / (x1 - x0);
            return y0 + t * (y1 - y0);
        }
    }
    table[table.len() - 1].1
}

/// The APCA contrast of two pre-computed luminances. Split out so the math is
/// testable independently of color conversion.
fn contrast(mut txt_y: f64, mut bg_y: f64) -> f64 {
    // Input domain clamp: NaN or outside [0.0, 1.1] yields no contrast.
    if txt_y.is_nan() || bg_y.is_nan() {
        return 0.0;
    }
    if txt_y.min(bg_y) < 0.0 || txt_y.max(bg_y) > Y_MAX {
        return 0.0;
    }
    // Black soft-clamp, applied to both luminances first.
    if txt_y <= BLK_THRS {
        txt_y += (BLK_THRS - txt_y).powf(BLK_CLMP);
    }
    if bg_y <= BLK_THRS {
        bg_y += (BLK_THRS - bg_y).powf(BLK_CLMP);
    }
    // Negligible luminance difference: no contrast.
    if (bg_y - txt_y).abs() < DELTA_Y_MIN {
        return 0.0;
    }
    let output = if bg_y > txt_y {
        // Normal polarity: dark text on a lighter background.
        let sapc = (bg_y.powf(NORM_BG) - txt_y.powf(NORM_TXT)) * SCALE_BOW;
        if sapc < LO_CLIP {
            0.0
        } else {
            sapc - LO_BOW_OFFSET
        }
    } else {
        // Reverse polarity: light text on a darker background.
        let sapc = (bg_y.powf(REV_BG) - txt_y.powf(REV_TXT)) * SCALE_WOB;
        if sapc > -LO_CLIP {
            0.0
        } else {
            sapc + LO_WOB_OFFSET
        }
    };
    output * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(hex: &str) -> Color {
        let h = hex.trim_start_matches('#');
        let n = u32::from_str_radix(h, 16).expect("hex");
        Color::from_rgba8((n >> 16) as u8, (n >> 8) as u8, n as u8, 255)
    }

    /// The published `0.98G-4g` reference values. f32 color input quantizes
    /// each 8-bit channel to within ~1e-7, so Lc matches to well under 0.01.
    #[test]
    fn matches_reference_vectors() {
        let cases = [
            ("#000000", "#ffffff", 106.040_672_479_003),
            ("#ffffff", "#000000", -107.884_733_110_653),
            ("#888888", "#ffffff", 63.056_469_930_209),
            ("#ffffff", "#888888", -68.541_464_366_450),
            ("#000000", "#aaaaaa", 58.146_262_578_561),
            ("#aaaaaa", "#000000", -56.241_133_368_397),
            ("#000000", "#888888", 41.017_552_855_556),
            ("#888888", "#000000", -38.622_974_760_542),
            ("#aaaaaa", "#ffffff", 45.834_574_796_143),
            ("#ffffff", "#aaaaaa", -50.737_685_085_089),
            ("#112233", "#ddeeff", 91.668_307_772_408),
            ("#444444", "#ffffff", 92.609_110_548_456),
        ];
        for (text, bg, want) in cases {
            let got = lc(rgb(text), rgb(bg));
            assert!(
                (got - want).abs() < 0.01,
                "Lc({text} on {bg}) = {got}, want {want}"
            );
        }
    }

    #[test]
    fn polarity_is_signed() {
        assert!(lc(rgb("#000000"), rgb("#ffffff")) > 0.0, "BoW positive");
        assert!(lc(rgb("#ffffff"), rgb("#000000")) < 0.0, "WoB negative");
    }

    #[test]
    fn identical_colors_have_no_contrast() {
        assert_eq!(lc(rgb("#777777"), rgb("#777777")), 0.0);
    }

    #[test]
    fn lc_abs_and_meets_use_magnitude() {
        // White-on-black is strong contrast despite the negative sign.
        assert!(lc_abs(rgb("#ffffff"), rgb("#000000")) > 100.0);
        assert!(meets(rgb("#ffffff"), rgb("#000000"), 90.0));
        assert!(!meets(rgb("#888888"), rgb("#777777"), 30.0));
    }

    #[test]
    fn required_lc_is_monotonic_in_size_and_weight() {
        // Smaller text needs more contrast.
        assert!(required_lc(12.0, 400.0) > required_lc(16.0, 400.0));
        assert!(required_lc(16.0, 400.0) > required_lc(24.0, 400.0));
        assert!(required_lc(24.0, 400.0) > required_lc(36.0, 400.0));
        // Lighter text needs more contrast (heavier ⇒ larger effective px).
        assert!(required_lc(16.0, 300.0) > required_lc(16.0, 400.0));
        assert!(required_lc(16.0, 400.0) > required_lc(16.0, 500.0));
        assert!(required_lc(16.0, 500.0) > required_lc(16.0, 600.0));
    }

    #[test]
    fn required_lc_matches_apca_anchors() {
        // 16px/400 is APCA's body minimum — the load-bearing anchor.
        assert!((required_lc(16.0, 400.0) - 75.0).abs() <= 5.0);
        // Small body text demands near-maximal contrast.
        assert!((90.0..=108.0).contains(&required_lc(12.0, 400.0)));
        // Large + bold text relaxes into the headline band.
        assert!((45.0..=60.0).contains(&required_lc(24.0, 700.0)));
        // Degenerate inputs clamp to a finite value in range, never panic.
        for lc in [required_lc(1.0, 900.0), required_lc(1000.0, 100.0)] {
            assert!(lc.is_finite() && (15.0..=108.0).contains(&lc), "lc = {lc}");
        }
    }
}
