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
}
