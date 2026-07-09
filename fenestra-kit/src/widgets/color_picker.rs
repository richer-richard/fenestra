//! OKLCH color picker: a lightness×chroma field, a hue strip, an alpha
//! strip, a swatch, and a hex/`oklch()` text entry — Elm-pure, like every
//! kit widget. The app owns the color as a plain [`Color`] (fenestra's one
//! color primitive; see [`fenestra_core::oklch`]/[`fenestra_core::oklch_of`]
//! for the OKLCH round-trip) and every gesture — pad drag, hue/alpha drag,
//! arrow keys, or a parsed text edit — reports the new value through
//! [`ColorPicker::on_change`].
//!
//! OKLCH chroma can leave sRGB gamut; the pad is generated per-pixel through
//! [`fenestra_core::oklch`] (which gamut-maps by reducing chroma, never
//! lightness), so it only ever shows displayable colors, and the picker
//! flags the swatch when the current point is sitting at that gamut edge
//! rather than silently lying about it.
//!
//! ```
//! use fenestra_core::{Color, oklch};
//! use fenestra_kit::color_picker;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Changed(Color),
//!     Typed(String, Option<Color>),
//! }
//!
//! let el: fenestra_core::Element<Msg> = color_picker(oklch(0.7, 0.15, 250.0))
//!     .label("Accent color")
//!     .on_change(Msg::Changed)
//!     .on_text_change(Msg::Typed)
//!     .into();
//! ```

use std::rc::Rc;

use fenestra_core::{
    Color, Cursor, Element, GradientStop, Key, Paint, R_FULL, R_SM, SP2, SP3, SP4, Semantics,
    ShadowToken, Theme, Transition, col, div, image_rgba8, oklch, oklch_of, row, stack,
};

use super::{ControlSize, Density};
use crate::icons;
use crate::text_input;

/// Upper bound the pad/field ever asks for on the chroma axis — a fixed,
/// generous ceiling (matching common CSS OKLCH tooling) comfortably past the
/// most saturated color any hue/lightness combination can actually display,
/// so chroma has one resolution-independent domain instead of a per-hue one.
pub const MAX_CHROMA: f32 = 0.4;

const PAD_MIN: f32 = 80.0;
const PAD_MAX: f32 = 480.0;
const PAD_DEFAULT: f32 = 200.0;

/// Height of the hue/alpha strip's clickable row (thumb included).
const STRIP_ROW_H: f32 = 20.0;
/// Height of the hue/alpha strip's painted track within that row.
const STRIP_TRACK_H: f32 = 12.0;
/// Edge length of every draggable thumb/puck marker.
const THUMB: f32 = 16.0;
/// Checkerboard cell size behind translucent colors (swatch + alpha track).
const CHECKER_CELL: u32 = 6;

/// Keyboard step sizes (arrow-key nudge) for each channel.
const L_STEP: f32 = 0.01;
const C_STEP: f32 = MAX_CHROMA / 40.0;
const H_STEP: f32 = 2.0;
const A_STEP: f32 = 0.02;

/// Fixed, vivid lightness/chroma sample the hue strip renders its rainbow
/// at. The hue strip is a hue *reference*, independent of the current L/C —
/// showing it at the live lightness/chroma would go flat gray whenever the
/// current chroma is low, making hue impossible to pick visually.
const HUE_STRIP_L: f32 = 0.72;
const HUE_STRIP_C: f32 = 0.2;

/// A nudge in chroma used to test whether an `(l, c, h)` point already sits
/// at (or past) its own local sRGB gamut boundary: see [`is_gamut_mapped`].
const GAMUT_PROBE: f32 = 0.02;

// ─── hostile-input clamps (clamp-over-panic at the widget's API boundary) ──

/// Clamps lightness to `0.0..=1.0`; non-finite input (NaN/±inf) falls back to
/// a neutral mid-gray rather than propagating into [`oklch`], which is not
/// guaranteed sane on non-finite components.
fn sanitize_lightness(l: f32) -> f32 {
    if l.is_finite() {
        l.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

/// Clamps chroma to `0.0..=MAX_CHROMA`; non-finite input falls back to `0.0`
/// (achromatic).
fn sanitize_chroma(c: f32) -> f32 {
    if c.is_finite() {
        c.clamp(0.0, MAX_CHROMA)
    } else {
        0.0
    }
}

/// Wraps hue into `0.0..360.0` (hue is circular, so this wraps rather than
/// clamps); non-finite input falls back to `0.0`. A tiny negative input
/// (e.g. `-1e-7`) can round-trip through `rem_euclid` to exactly `360.0`
/// (360.0's ULP dwarfs the input), which would violate the half-open
/// range, so that boundary case wraps to `0.0` explicitly.
fn sanitize_hue(h: f32) -> f32 {
    if !h.is_finite() {
        return 0.0;
    }
    let wrapped = h.rem_euclid(360.0);
    if wrapped >= 360.0 { 0.0 } else { wrapped }
}

/// Clamps alpha to `0.0..=1.0`; non-finite input falls back to fully opaque.
fn sanitize_alpha(a: f32) -> f32 {
    if a.is_finite() {
        a.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

/// True when `(l, c, h)` is sitting at (or past) its own local sRGB gamut
/// boundary: nudging chroma up by [`GAMUT_PROBE`] and re-mapping through
/// [`oklch`] fails to recover most of that nudge, meaning there is no
/// headroom left at this lightness/hue. Used to flag the swatch honestly
/// instead of silently showing a flattened color with no indication.
fn is_gamut_mapped(l: f32, c: f32, h: f32) -> bool {
    if c <= 0.0 {
        return false;
    }
    let [_, probed_c, _] = oklch_of(oklch(l, c + GAMUT_PROBE, h));
    probed_c < c + GAMUT_PROBE * 0.5
}

// ─── text parsing (forgiving: never panics, `None` on anything invalid) ────

fn hex_digit(b: u8) -> Option<u8> {
    (b as char).to_digit(16).map(|d| d as u8)
}

/// Parses `#rgb`/`#rgba`/`#rrggbb`/`#rrggbbaa` hex (no leading `#` required),
/// case-insensitive. `None` for anything else — including the right length
/// but non-hex characters.
fn parse_hex(s: &str) -> Option<Color> {
    if s.is_empty() || !s.is_ascii() {
        return None;
    }
    let b = s.as_bytes();
    let nibble = |i: usize| hex_digit(b[i]);
    match b.len() {
        3 | 4 => {
            let r = nibble(0)?;
            let g = nibble(1)?;
            let bl = nibble(2)?;
            let a = if b.len() == 4 { nibble(3)? } else { 15 };
            Some(Color::from_rgba8(r * 17, g * 17, bl * 17, a * 17))
        }
        6 | 8 => {
            let byte = |i: usize| -> Option<u8> { Some(nibble(i)? * 16 + nibble(i + 1)?) };
            let r = byte(0)?;
            let g = byte(2)?;
            let bl = byte(4)?;
            let a = if b.len() == 8 { byte(6)? } else { 255 };
            Some(Color::from_rgba8(r, g, bl, a))
        }
        _ => None,
    }
}

/// Parses one numeric OKLCH channel: a bare float, a percentage of
/// `pct_ref` (CSS's OKLCH reference range), or the CSS `none` keyword
/// (treated as `0`). Rejects non-finite results (so a literal `nan`/`inf` in
/// the text — which Rust's own float parser otherwise accepts — is treated
/// as invalid, not as a value to silently clamp).
fn parse_channel(s: &str, pct_ref: f32) -> Option<f32> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("none") {
        return Some(0.0);
    }
    let raw = if let Some(p) = s.strip_suffix('%') {
        p.trim().parse::<f32>().ok()? / 100.0 * pct_ref
    } else {
        s.parse::<f32>().ok()?
    };
    raw.is_finite().then_some(raw)
}

/// Parses the hue component: a bare number, an optional trailing `deg`, or
/// `none` (treated as `0`). Same non-finite rejection as [`parse_channel`].
fn parse_hue_component(s: &str) -> Option<f32> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("none") {
        return Some(0.0);
    }
    let raw = s
        .strip_suffix("deg")
        .unwrap_or(s)
        .trim()
        .parse::<f32>()
        .ok()?;
    raw.is_finite().then_some(raw)
}

fn parse_oklch_fn(inner: &str) -> Option<Color> {
    let (main, alpha_part) = match inner.split_once('/') {
        Some((m, a)) => (m, Some(a)),
        None => (inner, None),
    };
    let parts: Vec<&str> = main
        .split([',', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() != 3 {
        return None;
    }
    let l = parse_channel(parts[0], 1.0)?;
    let c = parse_channel(parts[1], MAX_CHROMA)?;
    let h = parse_hue_component(parts[2])?;
    let a = match alpha_part {
        Some(raw) => parse_channel(raw, 1.0)?,
        None => 1.0,
    };
    Some(
        oklch(sanitize_lightness(l), sanitize_chroma(c), sanitize_hue(h))
            .with_alpha(sanitize_alpha(a)),
    )
}

/// Parses a `#rrggbb`/`#rrggbbaa`/`#rgb`/`#rgba` hex string (the leading `#`
/// is optional) or a CSS `oklch(L C H)` / `oklch(L C H / A)` string into a
/// [`Color`]. Whitespace-tolerant and case-insensitive; OKLCH components
/// accept percentages or the `none` keyword. Never panics: anything else —
/// garbage text, the wrong channel count, `nan`/`inf` literals — returns
/// `None`, so a caller can leave the current value uncommitted while the
/// user is still typing rather than destroying it on an invalid keystroke.
#[must_use]
pub fn parse_color_text(text: &str) -> Option<Color> {
    let trimmed = text.trim();
    let lower = trimmed.to_ascii_lowercase();
    if let Some(inner) = lower
        .strip_prefix("oklch(")
        .and_then(|s| s.strip_suffix(')'))
    {
        return parse_oklch_fn(inner);
    }
    parse_hex(trimmed.strip_prefix('#').unwrap_or(trimmed))
}

/// Formats `color` as `#rrggbb` (or `#rrggbbaa` when translucent) — the
/// default text shown in the entry field when the caller doesn't thread
/// their own buffer via [`ColorPicker::text`].
#[must_use]
pub fn format_color_text(color: Color) -> String {
    let c = color.to_rgba8();
    if c.a == 255 {
        format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
    }
}

// ─── generated textures (deterministic RGBA8 fields) ───────────────────────

/// A deterministic `side`×`side` RGBA8 field: chroma `0.0..MAX_CHROMA`
/// horizontally, lightness `1.0..0.0` (light at the top) vertically, at a
/// fixed `hue` — the pad's generated background. Every pixel is
/// gamut-mapped through [`oklch`] (reducing chroma, never lightness), so the
/// field only ever shows displayable colors; it flattens past the gamut
/// edge instead of showing something impossible.
fn lc_field(hue: f32, side: u32) -> Vec<u8> {
    let side = side.max(1);
    let mut out = vec![0u8; side as usize * side as usize * 4];
    #[expect(clippy::cast_precision_loss, reason = "side is clamped to <= PAD_MAX")]
    let side_f = side as f32;
    for py in 0..side {
        #[expect(clippy::cast_precision_loss, reason = "py < side, which fits in f32")]
        let v = (py as f32 + 0.5) / side_f;
        let l = 1.0 - v;
        for px in 0..side {
            #[expect(clippy::cast_precision_loss, reason = "px < side, which fits in f32")]
            let u = (px as f32 + 0.5) / side_f;
            let c = u * MAX_CHROMA;
            let rgba = oklch(l, c, hue).to_rgba8();
            let i = (py * side + px) as usize * 4;
            out[i] = rgba.r;
            out[i + 1] = rgba.g;
            out[i + 2] = rgba.b;
            out[i + 3] = 255;
        }
    }
    out
}

/// A deterministic `width`×`height` RGBA8 rainbow: hue sweeps `0.0..360.0`
/// across the width at a fixed `(l, c)`, uniform down the height. The hue
/// strip's generated background.
fn hue_field(width: u32, height: u32, l: f32, c: f32) -> Vec<u8> {
    let (w, h) = (width.max(1), height.max(1));
    let mut out = vec![0u8; w as usize * h as usize * 4];
    #[expect(
        clippy::cast_precision_loss,
        reason = "w is clamped to a small strip width"
    )]
    let w_f = w as f32;
    for px in 0..w {
        #[expect(clippy::cast_precision_loss, reason = "px < w, which fits in f32")]
        let hue = (px as f32 + 0.5) / w_f * 360.0;
        let rgba = oklch(l, c, hue).to_rgba8();
        for py in 0..h {
            let i = (py * w + px) as usize * 4;
            out[i] = rgba.r;
            out[i + 1] = rgba.g;
            out[i + 2] = rgba.b;
            out[i + 3] = 255;
        }
    }
    out
}

/// A deterministic light/mid-gray checkerboard — the "behind translucency"
/// backdrop under the alpha strip and the swatch, so a translucent color
/// reads honestly against a known field instead of an ambiguous flat fill.
/// Theme-independent by design (like every other design tool's alpha
/// checker): it has to read the same regardless of the picked color or the
/// app's theme.
fn checkerboard(width: u32, height: u32, cell: u32) -> Vec<u8> {
    let cell = cell.max(1);
    let (w, h) = (width.max(1), height.max(1));
    let mut out = vec![0u8; w as usize * h as usize * 4];
    for py in 0..h {
        for px in 0..w {
            let on = ((px / cell) + (py / cell)).is_multiple_of(2);
            let v: u8 = if on { 214 } else { 168 };
            let i = (py * w + px) as usize * 4;
            out[i] = v;
            out[i + 1] = v;
            out[i + 2] = v;
            out[i + 3] = 255;
        }
    }
    out
}

// ─── shared visuals ─────────────────────────────────────────────────────────

/// A small ring marker (white border + a hairline shadow for definition
/// against light backgrounds) sized `THUMB` — used for the pad puck and the
/// hue/alpha thumbs alike. The white ring is one of the widget's two
/// deliberately theme-independent colors (with the alpha checkerboard):
/// it has to contrast the arbitrary picked color underneath it, which a
/// theme token cannot guarantee.
fn ring_marker<Msg: 'static>(cx: f32, cy: f32) -> Element<Msg> {
    div()
        .absolute()
        .top(cy - THUMB / 2.0)
        .left(cx - THUMB / 2.0)
        .w(THUMB)
        .h(THUMB)
        .rounded_full()
        .border(2.0, Color::from_rgba8(255, 255, 255, 235))
        .shadow(ShadowToken::Xs)
        .transition(Transition::colors().offsets(true))
}

/// Everything resolved once per render: the sanitized channels, whether
/// interaction is allowed, and the shared `on_change` mapper. Passed by
/// value (cheap: four floats, a bool, an `Rc` clone) into each sub-builder
/// instead of threading half a dozen separate arguments through every fn.
#[derive(Clone)]
struct Resolved<Msg> {
    l: f32,
    c: f32,
    h: f32,
    a: f32,
    disabled: bool,
    on_change: Option<Rc<dyn Fn(Color) -> Msg>>,
}

impl<Msg> Resolved<Msg> {
    fn color(&self) -> Color {
        oklch(self.l, self.c, self.h).with_alpha(self.a)
    }
}

/// The 2D lightness (vertical) × chroma (horizontal) pad: a generated field
/// at the current hue, draggable with the pointer, with two independent
/// keyboard-accessible axes layered on top (an invisible-until-focused
/// full-pad "Chroma" slider, and the puck itself doubling as the "Lightness"
/// slider) — mirroring how [`RangeSlider`](super::slider::RangeSlider) nests
/// two independent focus targets inside one draggable track.
fn pad_el<Msg: Clone + 'static>(side: f32, r: Resolved<Msg>) -> Element<Msg> {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "side is clamped to PAD_MIN..=PAD_MAX"
    )]
    let side_px = side.round() as u32;
    let bg = image_rgba8::<Msg>(side_px, side_px, lc_field(r.h, side_px))
        .w(side)
        .h(side)
        .rounded(R_SM);

    let cx = (r.c / MAX_CHROMA).clamp(0.0, 1.0) * side;
    let cy = (1.0 - r.l.clamp(0.0, 1.0)) * side;

    let mut chroma_overlay = div()
        .absolute()
        .top(0.0)
        .left(0.0)
        .w(side)
        .h(side)
        .label("Chroma")
        .semantics(Semantics::Slider {
            value: r.c,
            min: 0.0,
            max: MAX_CHROMA,
        });

    let mut puck = ring_marker::<Msg>(cx, cy)
        .label("Lightness")
        .semantics(Semantics::Slider {
            value: r.l,
            min: 0.0,
            max: 1.0,
        });

    if !r.disabled
        && let Some(f) = r.on_change.clone()
    {
        let (l0, c0, h0, a0) = (r.l, r.c, r.h, r.a);
        let fc = f.clone();
        chroma_overlay = chroma_overlay
            .focusable(true)
            .cursor(Cursor::Default)
            .focus_themed(|t: &Theme, s| s.border(2.0, t.accent_border))
            .on_key(move |k| {
                let nc = match k.key {
                    Key::ArrowLeft => c0 - C_STEP,
                    Key::ArrowRight => c0 + C_STEP,
                    Key::Home => 0.0,
                    Key::End => MAX_CHROMA,
                    _ => return None,
                };
                Some(fc(oklch(l0, sanitize_chroma(nc), h0).with_alpha(a0)))
            });

        puck = puck
            .focusable(true)
            .cursor(Cursor::Pointer)
            .focus_themed(|t: &Theme, s| s.border(2.0, t.accent_border))
            .on_key(move |k| {
                let nl = match k.key {
                    Key::ArrowDown => l0 - L_STEP,
                    Key::ArrowUp => l0 + L_STEP,
                    Key::Home => 0.0,
                    Key::End => 1.0,
                    _ => return None,
                };
                Some(f(oklch(sanitize_lightness(nl), c0, h0).with_alpha(a0)))
            });
    }

    let mut root = div().w(side).h(side).children([bg, chroma_overlay, puck]);
    if r.disabled {
        // The pad has no other disabled styling of its own (unlike
        // `text_input`, which already dims itself), so dim it here.
        root = root.opacity(0.5);
    } else if let Some(f) = r.on_change.clone() {
        let (h2, a2) = (r.h, r.a);
        root = root.on_drag(move |fx, fy| {
            let l = sanitize_lightness(1.0 - fy.clamp(0.0, 1.0));
            let c = sanitize_chroma(fx.clamp(0.0, 1.0) * MAX_CHROMA);
            Some(f(oklch(l, c, h2).with_alpha(a2)))
        });
    }
    root
}

/// Maps a strip's normalized `0.0..=1.0` drag/keyboard fraction, plus the
/// other resolved channels it doesn't own, to the resulting color.
type ToColor<Msg> = Rc<dyn Fn(f32, &Resolved<Msg>) -> Color>;

/// One 1D strip (hue or alpha): a generated/composited track plus a single
/// draggable, keyboard-operable thumb — the same shape as
/// [`super::slider::slider`], hand-built here because the track paint
/// (a rainbow, or a checkerboard-backed translucency ramp) isn't something
/// the generic `Slider` exposes a hook for.
struct Strip<Msg> {
    track: Element<Msg>,
    frac: f32,
    label: &'static str,
    min: f32,
    max: f32,
    step: f32,
    to_color: ToColor<Msg>,
}

fn strip_row<Msg: Clone + 'static>(width: f32, r: Resolved<Msg>, s: Strip<Msg>) -> Element<Msg> {
    let cx = s.frac.clamp(0.0, 1.0) * width;
    let mut thumb = ring_marker::<Msg>(cx, STRIP_ROW_H / 2.0)
        .label(s.label)
        .semantics(Semantics::Slider {
            value: s.frac.clamp(0.0, 1.0) * (s.max - s.min) + s.min,
            min: s.min,
            max: s.max,
        });

    if !r.disabled
        && let Some(f) = r.on_change.clone()
    {
        let cur = s.frac.clamp(0.0, 1.0) * (s.max - s.min) + s.min;
        let (min, max, step) = (s.min, s.max, s.step);
        let to_color = s.to_color.clone();
        let rk = r.clone();
        thumb = thumb
            .focusable(true)
            .cursor(Cursor::Pointer)
            .focus_themed(|t: &Theme, s| s.border(2.0, t.accent_border))
            .on_key(move |k| {
                let next = match k.key {
                    Key::ArrowLeft | Key::ArrowDown => cur - step,
                    Key::ArrowRight | Key::ArrowUp => cur + step,
                    Key::Home => min,
                    Key::End => max,
                    _ => return None,
                }
                .clamp(min, max);
                Some(f(to_color((next - min) / (max - min), &rk)))
            });
    }

    let mut container = row()
        .items_center()
        .w(width)
        .h(STRIP_ROW_H)
        .children([s.track, thumb]);
    if r.disabled {
        // No other disabled styling of its own; dim it here (see `pad_el`).
        container = container.opacity(0.5);
    } else if let Some(f) = r.on_change.clone() {
        let to_color = s.to_color.clone();
        let rk = r.clone();
        container = container.on_drag(move |fx, _fy| Some(f(to_color(fx.clamp(0.0, 1.0), &rk))));
    }
    container
}

fn hue_row<Msg: Clone + 'static>(width: f32, r: Resolved<Msg>) -> Element<Msg> {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "width is clamped to PAD_MIN..=PAD_MAX"
    )]
    let width_px = width.round() as u32;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "STRIP_TRACK_H is a small fixed constant"
    )]
    let track_h_px = STRIP_TRACK_H.round() as u32;
    let track = image_rgba8::<Msg>(
        width_px,
        track_h_px,
        hue_field(width_px, track_h_px, HUE_STRIP_L, HUE_STRIP_C),
    )
    .w(width)
    .h(STRIP_TRACK_H)
    .rounded(R_FULL);

    let frac = r.h / 360.0;
    strip_row(
        width,
        r,
        Strip {
            track,
            frac,
            label: "Hue",
            min: 0.0,
            max: 360.0,
            step: H_STEP,
            to_color: Rc::new(|frac, r| {
                oklch(r.l, r.c, sanitize_hue(frac * 360.0)).with_alpha(r.a)
            }),
        },
    )
}

fn alpha_row<Msg: Clone + 'static>(width: f32, r: Resolved<Msg>) -> Element<Msg> {
    let base = oklch(r.l, r.c, r.h);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "width is clamped to PAD_MIN..=PAD_MAX"
    )]
    let width_px = width.round() as u32;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "STRIP_TRACK_H is a small fixed constant"
    )]
    let track_h_px = STRIP_TRACK_H.round() as u32;
    let checker = image_rgba8::<Msg>(
        width_px,
        track_h_px,
        checkerboard(width_px, track_h_px, CHECKER_CELL),
    )
    .w(width)
    .h(STRIP_TRACK_H);
    let gradient = div()
        .absolute()
        .top(0.0)
        .left(0.0)
        .w(width)
        .h(STRIP_TRACK_H)
        .bg(Paint::LinearGradient {
            angle_deg: 90.0,
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: base.with_alpha(0.0),
                },
                GradientStop {
                    offset: 1.0,
                    color: base,
                },
            ],
        });
    let track = stack()
        .w(width)
        .h(STRIP_TRACK_H)
        .rounded(R_FULL)
        .overflow_hidden()
        .children([checker, gradient]);

    let frac = r.a;
    strip_row(
        width,
        r,
        Strip {
            track,
            frac,
            label: "Alpha",
            min: 0.0,
            max: 1.0,
            step: A_STEP,
            to_color: Rc::new(|frac, r| oklch(r.l, r.c, r.h).with_alpha(sanitize_alpha(frac))),
        },
    )
}

fn gamut_badge<Msg: 'static>() -> Element<Msg> {
    icons::lucide::alert_triangle()
        .w(14.0)
        .h(14.0)
        .themed(|t: &Theme, s| s.color(t.warning.solid))
        .semantics(Semantics::Label)
        .label("Out of gamut — showing the nearest displayable color")
}

fn swatch_el<Msg: 'static>(size: f32, color: Color, disabled: bool) -> Element<Msg> {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "size comes from ControlMetrics::height, a small fixed grid value"
    )]
    let size_px = size.round().max(1.0) as u32;
    let checker = image_rgba8::<Msg>(
        size_px,
        size_px,
        checkerboard(size_px, size_px, CHECKER_CELL),
    )
    .w(size)
    .h(size);
    let fill = div()
        .absolute()
        .top(0.0)
        .left(0.0)
        .w(size)
        .h(size)
        .bg(color)
        .transition(Transition::colors());
    let mut el = stack()
        .w(size)
        .h(size)
        .shrink0()
        .rounded(R_SM)
        .overflow_hidden()
        .themed(|t: &Theme, s| s.border(1.0, t.border))
        .children([checker, fill])
        .semantics(Semantics::Image)
        .label(format!("Current color {}", format_color_text(color)));
    if disabled {
        el = el.opacity(0.5);
    }
    el
}

/// Maps a text entry edit — the raw typed string plus the parsed color when
/// the text currently parses — to a message.
type OnTextChange<Msg> = Rc<dyn Fn(String, Option<Color>) -> Msg>;

/// An OKLCH color picker under construction; converts into an [`Element`].
pub struct ColorPicker<Msg> {
    value: Color,
    text: Option<String>,
    label: String,
    disabled: bool,
    size: ControlSize,
    density: Density,
    pad_size: f32,
    on_change: Option<Rc<dyn Fn(Color) -> Msg>>,
    on_text_change: Option<OnTextChange<Msg>>,
    key: Option<String>,
}

/// An OKLCH color picker showing `value` (any [`Color`] — the framework's
/// one color primitive; build one with [`fenestra_core::oklch`] or convert
/// an existing one to OKLCH terms with [`fenestra_core::oklch_of`]). Wire
/// [`ColorPicker::on_change`] for the pad/hue/alpha gestures and
/// [`ColorPicker::on_text_change`] for the hex/`oklch()` entry field.
pub fn color_picker<Msg>(value: Color) -> ColorPicker<Msg> {
    ColorPicker {
        value,
        text: None,
        label: "Color".to_owned(),
        disabled: false,
        size: ControlSize::default(),
        density: Density::default(),
        pad_size: PAD_DEFAULT,
        on_change: None,
        on_text_change: None,
        key: None,
    }
}

impl<Msg> ColorPicker<Msg> {
    /// The text entry field's current content. Defaults to a live
    /// `#rrggbb`/`#rrggbbaa` formatting of `value`; set this to thread your
    /// own in-progress buffer (so a partially-typed, still-invalid string
    /// stays visible instead of being overwritten every render).
    #[must_use]
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// The accessible name for the whole widget (default `"Color"`).
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Disables every control (pad, strips, entry field) and dims the whole
    /// widget.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets the control size for the swatch and the text entry field (the
    /// pad and strips are their own fixed-scale visual fields; see
    /// [`ColorPicker::pad_size`]).
    #[must_use]
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Sets the packing density ([`Density`]) for the same grid-aligned
    /// parts as [`ColorPicker::size`].
    #[must_use]
    pub fn density(mut self, density: Density) -> Self {
        self.density = density;
        self
    }

    /// Sets the 2D pad's side length in logical px (200 by default), clamped
    /// to a sane `80.0..=480.0` range.
    #[must_use]
    pub fn pad_size(mut self, side: f32) -> Self {
        self.pad_size = side.clamp(PAD_MIN, PAD_MAX);
        self
    }

    /// Maps every pointer/keyboard gesture (pad drag or arrows, hue/alpha
    /// drag or arrows) to a message carrying the new color.
    pub fn on_change(mut self, f: impl Fn(Color) -> Msg + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    /// Maps every edit of the text entry field to a message carrying both
    /// the raw typed text (always — store this as your buffer) and the
    /// parsed color when the text is currently valid (`None` while it
    /// isn't — leave `value` unchanged in that case, so an in-progress edit
    /// never destroys the last good color).
    ///
    /// This deliberately departs from the kit's usual `on_input(Fn(String))`
    /// text-entry verb: the second `Option<Color>` argument is the parse
    /// result the caller needs to decide whether to commit, so a plain
    /// `on_input` shape could not carry it.
    pub fn on_text_change(mut self, f: impl Fn(String, Option<Color>) -> Msg + 'static) -> Self {
        self.on_text_change = Some(Rc::new(f));
        self
    }

    /// Stable identity key.
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

impl<Msg: Clone + 'static> From<ColorPicker<Msg>> for Element<Msg> {
    fn from(p: ColorPicker<Msg>) -> Self {
        let [raw_l, raw_c, raw_h] = oklch_of(p.value);
        let l = sanitize_lightness(raw_l);
        let c = sanitize_chroma(raw_c);
        let h = sanitize_hue(raw_h);
        let a = sanitize_alpha(p.value.components[3]);
        let gamut_mapped = is_gamut_mapped(l, c, h);

        let resolved = Resolved {
            l,
            c,
            h,
            a,
            disabled: p.disabled,
            on_change: p.on_change.clone(),
        };
        let resolved_color = resolved.color();
        let side = p.pad_size;
        let m = p.size.metrics_at(p.density);

        let mut header_children = vec![swatch_el(m.height, resolved_color, p.disabled)];
        if gamut_mapped {
            let mut badge = gamut_badge();
            if p.disabled {
                badge = badge.opacity(0.5);
            }
            header_children.push(badge);
        }
        let header = row().gap(SP2).items_center().children(header_children);

        let text_value = p
            .text
            .clone()
            .unwrap_or_else(|| format_color_text(resolved_color));
        let invalid = !text_value.trim().is_empty() && parse_color_text(&text_value).is_none();
        let mut entry = text_input(text_value.clone())
            .placeholder("#rrggbb or oklch(l c h)")
            .width(side)
            .size(p.size)
            .density(p.density)
            .disabled(p.disabled)
            .invalid(invalid);
        if let Some(f) = p.on_text_change.clone() {
            entry = entry.on_input(move |s: String| {
                let parsed = parse_color_text(&s);
                f(s, parsed)
            });
        }

        let controls = col().gap(SP3).w(side).children([
            header,
            hue_row(side, resolved.clone()),
            alpha_row(side, resolved.clone()),
            Element::from(entry),
        ]);

        let mut root = row()
            .gap(SP4)
            .items_start()
            .label(p.label.clone())
            .children([pad_el(side, resolved), controls]);
        // Each sub-element dims itself when disabled (`pad_el`, `strip_row`,
        // `swatch_el`, and the badge above; `text_input` already dims its own
        // `.disabled` state) — no additional opacity here, or the text entry
        // would end up double-dimmed (0.5 × 0.5).
        if let Some(key) = &p.key {
            root = root.id(key);
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_functions_clamp_hostile_values() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(sanitize_lightness(bad), 0.5);
            assert_eq!(sanitize_chroma(bad), 0.0);
            assert_eq!(sanitize_hue(bad), 0.0);
            assert_eq!(sanitize_alpha(bad), 1.0);
        }
        assert_eq!(sanitize_lightness(-5.0), 0.0);
        assert_eq!(sanitize_lightness(5.0), 1.0);
        assert_eq!(sanitize_chroma(-1.0), 0.0);
        assert_eq!(sanitize_chroma(10.0), MAX_CHROMA);
        assert_eq!(sanitize_alpha(-1.0), 0.0);
        assert_eq!(sanitize_alpha(2.0), 1.0);
        // Hue wraps (circular), rather than clamping to an endpoint.
        assert!((sanitize_hue(-10.0) - 350.0).abs() < 1e-3);
        assert!((sanitize_hue(370.0) - 10.0).abs() < 1e-3);
    }

    #[test]
    fn gamut_mapped_flags_extreme_points_only() {
        // Near-white with strong chroma has essentially no headroom.
        assert!(is_gamut_mapped(0.97, 0.3, 30.0));
        // A modest, well-inside-gamut color has plenty of headroom.
        assert!(!is_gamut_mapped(0.5, 0.05, 200.0));
        // Achromatic is trivially never gamut-mapped.
        assert!(!is_gamut_mapped(0.5, 0.0, 0.0));
    }

    #[test]
    fn hex_forms_parse() {
        let c6 = parse_color_text("#ff0000").expect("6-digit hex");
        assert_eq!(c6.to_rgba8().to_u8_array(), [255, 0, 0, 255]);
        let c3 = parse_color_text("f00").expect("3-digit hex, no leading #");
        assert_eq!(c3.to_rgba8().to_u8_array(), [255, 0, 0, 255]);
        let c8 = parse_color_text("#00ff0080").expect("8-digit hex with alpha");
        let got = c8.to_rgba8();
        assert_eq!((got.r, got.g, got.b), (0, 255, 0));
        assert!((i32::from(got.a) - 128).abs() <= 1);
        let c4 = parse_color_text("#0f08").expect("4-digit hex with alpha");
        assert_eq!(c4.to_rgba8().r, 0);
    }

    #[test]
    fn oklch_form_parses_percentages_degrees_and_none() {
        // Modest lightness/chroma, comfortably inside gamut at this hue (the
        // sRGB gamut at hue 180/l 0.5 tops out around c=0.09 — verified with
        // a throwaway probe), so the round-trip through `oklch`'s gamut
        // mapping is exact and this test is purely about the percent/deg
        // parsing math.
        let c = parse_color_text("oklch(50% 15% 180deg)").expect("percent + deg form");
        let [l, ch, hue] = oklch_of(c);
        assert!((l - 0.5).abs() < 1e-3);
        assert!((ch - MAX_CHROMA * 0.15).abs() < 1e-3);
        assert!((hue - 180.0).abs() < 1e-3);

        let with_alpha = parse_color_text("oklch(0.5 0.1 30 / 50%)").expect("alpha form");
        assert!((with_alpha.components[3] - 0.5).abs() < 1e-3);

        let none_chroma = parse_color_text("oklch(0.6 none 0)").expect("none channel");
        assert!(oklch_of(none_chroma)[1].abs() < 1e-6);
    }

    #[test]
    fn invalid_text_never_panics_and_returns_none() {
        for bad in [
            "",
            "not a color",
            "#12",
            "#gggggg",
            "oklch(",
            "oklch()",
            "oklch(0.5 0.1)",
            "oklch(nan 0.1 30)",
            "oklch(0.5 inf 30)",
            "#🎨🎨🎨",
        ] {
            assert!(parse_color_text(bad).is_none(), "expected None for {bad:?}");
        }
    }

    #[test]
    fn format_and_parse_hex_round_trip() {
        let c = Color::from_rgba8(18, 52, 86, 255);
        let text = format_color_text(c);
        assert_eq!(text, "#123456");
        assert_eq!(
            parse_color_text(&text).unwrap().to_rgba8().to_u8_array(),
            c.to_rgba8().to_u8_array()
        );
    }

    #[test]
    fn lc_field_and_hue_field_are_deterministic_and_correctly_sized() {
        let a = lc_field(30.0, 8);
        let b = lc_field(30.0, 8);
        assert_eq!(a, b, "same inputs ⇒ same texture");
        assert_eq!(a.len(), 8 * 8 * 4);
        assert!(a.chunks_exact(4).all(|px| px[3] == 255), "opaque");

        let hue_tex = hue_field(12, 4, 0.7, 0.15);
        assert_eq!(hue_tex.len(), 12 * 4 * 4);
        assert!(hue_tex.chunks_exact(4).all(|px| px[3] == 255));
    }

    #[test]
    fn checkerboard_alternates_and_is_opaque() {
        let tex = checkerboard(4, 4, 2);
        assert_eq!(tex.len(), 4 * 4 * 4);
        let px = |x: usize, y: usize| tex[(y * 4 + x) * 4];
        assert_ne!(px(0, 0), px(2, 0), "adjacent cells differ");
        assert_eq!(px(0, 0), px(1, 1), "same cell (2x2) matches");
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::{
        MAX_CHROMA, parse_color_text, sanitize_alpha, sanitize_chroma, sanitize_hue,
        sanitize_lightness,
    };

    proptest! {
        #[test]
        fn sanitize_lightness_total(l in any::<f32>()) {
            let v = sanitize_lightness(l);
            prop_assert!(v.is_finite());
            prop_assert!((0.0..=1.0).contains(&v));
        }

        #[test]
        fn sanitize_chroma_total(c in any::<f32>()) {
            let v = sanitize_chroma(c);
            prop_assert!(v.is_finite());
            prop_assert!((0.0..=MAX_CHROMA).contains(&v));
        }

        #[test]
        fn sanitize_hue_total(h in any::<f32>()) {
            let v = sanitize_hue(h);
            prop_assert!(v.is_finite());
            prop_assert!((0.0..360.0).contains(&v));
        }

        #[test]
        fn sanitize_alpha_total(a in any::<f32>()) {
            let v = sanitize_alpha(a);
            prop_assert!(v.is_finite());
            prop_assert!((0.0..=1.0).contains(&v));
        }

        #[test]
        fn parse_color_text_never_panics(s in ".{0,64}") {
            let _ = parse_color_text(&s);
        }
    }
}
