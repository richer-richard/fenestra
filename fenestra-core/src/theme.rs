//! Theme generation: OKLCH color ramps derived from one accent hue, plus the
//! semantic roles, status colors, and shadow scale. The L/C tables here are
//! the design spec; every generated value is locked by an insta snapshot.

use color::{AlphaColor, Oklch, Srgb};
use peniko::Color;

use crate::style::Shadow;
use crate::tokens::ShadowToken;

/// Light or dark color mode. Both are always generated from the same hue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Light backgrounds, dark text.
    Light,
    /// Dark backgrounds, light text.
    Dark,
}

/// The neutral-field character for [`Theme::derive`]: the hue the neutral ramp
/// is tinted with and how far it departs from gray. `chroma` multiplies the
/// neutral table's (very low) base chroma — `1.0` is the stock near-gray SaaS
/// tint, `4`–`10` an atmospheric duotone field (deep green, warm paper), `0`
/// pure gray.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BaseField {
    /// Neutral ramp hue in OKLCH degrees.
    pub hue: f32,
    /// Chroma multiplier on the neutral table's base chroma.
    pub chroma: f32,
}

/// Contrast level for [`Theme::derive`]: scales every neutral step's lightness
/// distance from the background, so text and UI separation widen or soften from
/// one knob. `Standard` reproduces the stock ramps exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Contrast {
    /// Gentler separation (0.92× the stock spread).
    Low,
    /// The stock ramps (1.0×).
    #[default]
    Standard,
    /// Crisper separation (1.10×).
    High,
}

impl Contrast {
    /// Lightness-distance multiplier from the background step.
    fn factor(self) -> f32 {
        match self {
            Self::Low => 0.92,
            Self::Standard => 1.0,
            Self::High => 1.10,
        }
    }
}

/// A 12-step color ramp.
#[derive(Debug, Clone)]
pub struct Ramp(pub [Color; 12]);

impl Ramp {
    /// Returns step `n` (1-based, like the design spec tables). Out-of-range
    /// values clamp to the nearest valid step.
    pub fn step(&self, n: usize) -> Color {
        self.0[n.clamp(1, 12) - 1]
    }
}

/// The resolved colors of one status hue: tinted background, border, solid
/// fill with its hover and pressed variants, and text.
#[derive(Debug, Clone, Copy)]
pub struct StatusColors {
    /// Tinted background (step 3).
    pub bg: Color,
    /// Border (step 7).
    pub border: Color,
    /// Solid fill (step 9).
    pub solid: Color,
    /// Hover state of the solid fill (step 10).
    pub solid_hover: Color,
    /// Pressed state of the solid fill (one OKLCH-lightness notch below
    /// `solid_hover`).
    pub solid_active: Color,
    /// Text on `bg` (step 11).
    pub text: Color,
}

/// A text/background pair that failed its APCA Lc floor during
/// [`Theme::validate_contrast`].
#[derive(Debug, Clone, PartialEq)]
pub struct ContrastViolation {
    /// The pair that fell short, e.g. `"text_muted on surface_raised"`.
    pub pair: String,
    /// The measured APCA Lc magnitude.
    pub measured_lc: f64,
    /// The floor it failed to reach.
    pub required_lc: f64,
}

impl std::fmt::Display for ContrastViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: APCA Lc {:.1} < required {:.1}",
            self.pair, self.measured_lc, self.required_lc
        )
    }
}

/// Design tokens resolved for one color mode.
#[derive(Debug, Clone)]
pub struct Theme {
    /// The color mode this theme was generated for.
    pub mode: Mode,
    /// The accent hue (OKLCH degrees) this theme was generated from.
    pub accent_hue: f32,
    /// The hue (OKLCH degrees) the neutral ramp was generated with: equal to
    /// `accent_hue` for `from_accent`, the duotone field hue otherwise.
    pub neutral_hue: f32,
    /// Chroma multiplier applied to the neutral ramp (1.0 for `from_accent`,
    /// the duotone chroma boost otherwise) — so surfaces derived from the
    /// ramp stay on the neutral field.
    pub neutral_chroma_mult: f32,
    /// The 12-step neutral ramp, tinted with the accent hue at low chroma.
    pub neutrals: Ramp,
    /// The 12-step accent ramp.
    pub accents: Ramp,
    /// Neutral alpha twins: each step as the smallest-alpha translucent color
    /// that, composited over `bg`, reproduces the solid neutral step. Use for
    /// overlays and state layers that must read correctly over any surface,
    /// not only over `bg`.
    pub neutral_alpha: Ramp,
    /// Accent alpha twins (translucent over `bg`); see [`Theme::neutral_alpha`].
    pub accent_alpha: Ramp,
    /// Danger status colors (hue 25).
    pub danger: StatusColors,
    /// Warning status colors (hue 80).
    pub warning: StatusColors,
    /// Success status colors (hue 150).
    pub success: StatusColors,

    /// Window background (N1).
    pub bg: Color,
    /// Default surface (N2).
    pub surface: Color,
    /// Raised surface: pure white in light mode, N3 in dark mode.
    pub surface_raised: Color,
    /// Interactive element fill (N3): unselected ghost/soft control backgrounds.
    pub element: Color,
    /// Element hover fill (N4).
    pub element_hover: Color,
    /// Element active/pressed fill (N5).
    pub element_active: Color,
    /// Subtle border (N5); pairs with the Sm shadow on cards.
    pub border_subtle: Color,
    /// Default border (N6).
    pub border: Color,
    /// Strong border (N7).
    pub border_strong: Color,
    /// Primary text (N12).
    pub text: Color,
    /// Muted text (N11).
    pub text_muted: Color,
    /// Subtle text (N9).
    pub text_subtle: Color,
    /// Disabled text (N8).
    pub text_disabled: Color,
    /// Accent solid (A9): primary buttons, focus, selection.
    pub accent: Color,
    /// Accent hover (A10).
    pub accent_hover: Color,
    /// Accent pressed (one OKLCH-lightness notch below `accent_hover`; in light
    /// mode this lands on A11's lightness at A10's chroma).
    pub accent_active: Color,
    /// Tinted accent background (A3).
    pub accent_bg: Color,
    /// Accent border (A7).
    pub accent_border: Color,
    /// Accent-colored text (A11).
    pub accent_text: Color,
    /// Text painted on top of `accent`.
    pub on_accent: Color,
}

/// `(L, C)` per ramp step 1..=12.
type RampTable = [(f32, f32); 12];

const NEUTRAL_LIGHT: RampTable = [
    (0.992, 0.002),
    (0.978, 0.003),
    (0.955, 0.004),
    (0.930, 0.005),
    (0.905, 0.006),
    (0.875, 0.007),
    (0.830, 0.008),
    (0.730, 0.010),
    (0.555, 0.012),
    (0.510, 0.012),
    (0.435, 0.010),
    (0.235, 0.008),
];

const NEUTRAL_DARK: RampTable = [
    (0.185, 0.004),
    (0.215, 0.005),
    (0.250, 0.006),
    (0.280, 0.007),
    (0.310, 0.008),
    (0.345, 0.009),
    (0.400, 0.010),
    (0.490, 0.012),
    (0.560, 0.012),
    (0.610, 0.012),
    (0.770, 0.008),
    (0.945, 0.004),
];

const ACCENT_LIGHT: RampTable = [
    (0.975, 0.020),
    (0.950, 0.040),
    (0.920, 0.060),
    (0.880, 0.080),
    (0.835, 0.100),
    (0.785, 0.120),
    (0.725, 0.140),
    (0.660, 0.150),
    (0.585, 0.160),
    (0.545, 0.155),
    (0.500, 0.135),
    (0.380, 0.100),
];

// Steps 9 and 10 match light mode: the brand color is constant across modes.
const ACCENT_DARK: RampTable = [
    (0.250, 0.040),
    (0.290, 0.055),
    (0.330, 0.070),
    (0.370, 0.085),
    (0.415, 0.100),
    (0.465, 0.120),
    (0.530, 0.140),
    (0.600, 0.150),
    (0.585, 0.160),
    (0.545, 0.155),
    (0.720, 0.140),
    (0.880, 0.110),
];

/// Status hues in OKLCH degrees.
const DANGER_HUE: f32 = 25.0;
const WARNING_HUE: f32 = 80.0;
const SUCCESS_HUE: f32 = 150.0;

/// Shadow tokens as `(dy, blur, alpha)` layers; dx and spread are 0. Multi-layer
/// tokens stack a tight contact shadow under a softer ambient one (and a third,
/// far layer for deep overlays) — the calibrated key+ambient ramp web systems use.
const fn shadow_layers(token: ShadowToken) -> &'static [(f32, f32, f32)] {
    match token {
        ShadowToken::Xs => &[(1.0, 2.0, 0.05)],
        ShadowToken::Sm => &[(1.0, 2.0, 0.05), (1.0, 3.0, 0.06)],
        ShadowToken::Md => &[(2.0, 4.0, 0.05), (4.0, 12.0, 0.08)],
        ShadowToken::Lg => &[(4.0, 10.0, 0.06), (16.0, 32.0, 0.12)],
        ShadowToken::Xl => &[(2.0, 6.0, 0.04), (8.0, 16.0, 0.08), (24.0, 48.0, 0.16)],
    }
}

/// Dark mode multiplies shadow alphas by this factor: soft black shadows
/// read poorly on dark backgrounds.
const DARK_SHADOW_ALPHA_FACTOR: f32 = 1.6;

/// Per-level lightness boost for raised surfaces in dark mode.
const DARK_ELEVATION_TINT: f32 = 0.025;

/// Pressed states drop this much OKLCH lightness below the step-10 hover. In
/// light mode the accent lands exactly on A11's lightness (0.545 → 0.500).
const ACTIVE_DL: f32 = 0.045;

/// APCA Lc floors enforced by [`Theme::validate_contrast`], as magnitudes.
/// Primary body text targets Lc 90 and the stock themes reach it; the floor
/// sits at the canonical body minimum (75) so it trips on a regression, not a
/// design choice. Secondary, control-label, and colored-component text get
/// progressively lower floors matching their role, size, and weight. Borders
/// and other non-text delineation are intentionally not checked — APCA models
/// text legibility, not non-text contrast.
const PRIMARY_TEXT_MIN: f64 = 75.0;
const SECONDARY_TEXT_MIN: f64 = 55.0;
const CONTROL_LABEL_MIN: f64 = 60.0;
const COMPONENT_TEXT_MIN: f64 = 40.0;

impl Theme {
    /// Generates every token from one accent hue (OKLCH degrees).
    /// The default fenestra accent is hue 262 (violet-blue).
    pub fn from_accent(hue_deg: f32, mode: Mode) -> Self {
        let hue = hue_deg.rem_euclid(360.0);
        let (neutral_table, accent_table) = match mode {
            Mode::Light => (&NEUTRAL_LIGHT, &ACCENT_LIGHT),
            Mode::Dark => (&NEUTRAL_DARK, &ACCENT_DARK),
        };
        let neutrals = make_ramp(neutral_table, hue);
        let accents = make_ramp(accent_table, hue);
        let status = |status_hue: f32| StatusColors {
            bg: ramp_color(accent_table, 3, status_hue),
            border: ramp_color(accent_table, 7, status_hue),
            solid: ramp_color(accent_table, 9, status_hue),
            solid_hover: ramp_color(accent_table, 10, status_hue),
            solid_active: active_color(accent_table, status_hue),
            text: ramp_color(accent_table, 11, status_hue),
        };

        let surface_raised = match mode {
            Mode::Light => Color::new([1.0, 1.0, 1.0, 1.0]),
            Mode::Dark => neutrals.step(3),
        };
        // The table L of A9 decides the on-accent text; gamut mapping never
        // changes lightness, so read it straight from the spec table.
        let on_accent = if accent_table[8].0 < 0.65 {
            Color::new([1.0, 1.0, 1.0, 1.0])
        } else {
            neutrals.step(12)
        };

        Self {
            mode,
            accent_hue: hue,
            neutral_hue: hue,
            neutral_chroma_mult: 1.0,
            danger: status(DANGER_HUE),
            warning: status(WARNING_HUE),
            success: status(SUCCESS_HUE),
            bg: neutrals.step(1),
            surface: neutrals.step(2),
            surface_raised,
            element: neutrals.step(3),
            element_hover: neutrals.step(4),
            element_active: neutrals.step(5),
            border_subtle: neutrals.step(5),
            border: neutrals.step(6),
            border_strong: neutrals.step(7),
            text: neutrals.step(12),
            text_muted: neutrals.step(11),
            text_subtle: neutrals.step(9),
            text_disabled: neutrals.step(8),
            accent: accents.step(9),
            accent_hover: accents.step(10),
            accent_active: active_color(accent_table, hue),
            accent_bg: accents.step(3),
            accent_border: accents.step(7),
            accent_text: accents.step(11),
            on_accent,
            neutral_alpha: alpha_ramp(&neutrals, neutrals.step(1)),
            accent_alpha: alpha_ramp(&accents, neutrals.step(1)),
            neutrals,
            accents,
        }
    }

    /// A duotone theme: the neutral field takes its own hue with a chroma
    /// multiplier (1.0 matches the standard near-gray neutrals; 4-10 gives
    /// an atmospheric, editorial field like deep green or warm paper),
    /// while the accent keeps `from_accent` semantics. Out-of-gamut chroma
    /// is gamut-mapped per color, never clipped.
    pub fn duotone(neutral_hue: f32, neutral_chroma: f32, accent_hue: f32, mode: Mode) -> Self {
        let mut theme = Self::from_accent(accent_hue, mode);
        let neutral_table = match mode {
            Mode::Light => &NEUTRAL_LIGHT,
            Mode::Dark => &NEUTRAL_DARK,
        };
        let hue = neutral_hue.rem_euclid(360.0);
        let boost = neutral_chroma.clamp(0.0, 40.0);
        let neutrals = Ramp(std::array::from_fn(|i| {
            let (l, c) = neutral_table[i];
            oklch(l, c * boost, hue)
        }));
        theme.apply_neutral_field(neutrals, hue, boost);
        theme
    }

    /// Web-grade by default: the whole palette from three inputs (Linear's
    /// model collapsed onto fenestra's OKLCH scales). `base` is the neutral
    /// field (hue + how far from gray), `accent_hue` the brand hue, and
    /// `contrast` the separation level. [`from_accent`](Self::from_accent) and
    /// [`duotone`](Self::duotone) are special cases — `duotone(hue, c, accent,
    /// mode)` equals `derive(BaseField{hue, chroma: c}, accent, Standard, mode)`.
    /// A matching radius family comes from [`RadiusScale::from_base`].
    ///
    /// [`RadiusScale::from_base`]: crate::RadiusScale::from_base
    pub fn derive(base: BaseField, accent_hue: f32, contrast: Contrast, mode: Mode) -> Self {
        let mut theme = Self::from_accent(accent_hue, mode);
        let neutral_table = match mode {
            Mode::Light => &NEUTRAL_LIGHT,
            Mode::Dark => &NEUTRAL_DARK,
        };
        let hue = base.hue.rem_euclid(360.0);
        let chroma_mult = base.chroma.clamp(0.0, 40.0);
        let k = contrast.factor();
        let l_bg = neutral_table[0].0;
        let neutrals = Ramp(std::array::from_fn(|i| {
            let (l, c) = neutral_table[i];
            // Scale each step's lightness distance from the page background, so
            // contrast widens or softens against a fixed bg rather than drifting.
            let l = (l_bg + (l - l_bg) * k).clamp(0.0, 1.0);
            oklch(l, c * chroma_mult, hue)
        }));
        theme.apply_neutral_field(neutrals, hue, chroma_mult);
        theme
    }

    /// Re-points every neutral-derived role at `neutrals` (already built for
    /// `hue` at `chroma_mult` × the base table chroma). Shared by `duotone`
    /// and `derive`; the accent ramp, status colors, and shadows are untouched.
    fn apply_neutral_field(&mut self, neutrals: Ramp, hue: f32, chroma_mult: f32) {
        let accent_table = match self.mode {
            Mode::Light => &ACCENT_LIGHT,
            Mode::Dark => &ACCENT_DARK,
        };
        let bg = neutrals.step(1);
        self.bg = bg;
        self.surface = neutrals.step(2);
        if matches!(self.mode, Mode::Dark) {
            self.surface_raised = neutrals.step(3);
        }
        self.element = neutrals.step(3);
        self.element_hover = neutrals.step(4);
        self.element_active = neutrals.step(5);
        self.border_subtle = neutrals.step(5);
        self.border = neutrals.step(6);
        self.border_strong = neutrals.step(7);
        self.text = neutrals.step(12);
        self.text_muted = neutrals.step(11);
        self.text_subtle = neutrals.step(9);
        self.text_disabled = neutrals.step(8);
        // A light accent (table L >= 0.65) takes dark on-accent text from the
        // new field, matching `from_accent`'s rule against the field neutrals.
        if accent_table[8].0 >= 0.65 {
            self.on_accent = neutrals.step(12);
        }
        self.neutral_alpha = alpha_ramp(&neutrals, bg);
        self.accent_alpha = alpha_ramp(&self.accents, bg);
        self.neutral_hue = hue;
        self.neutral_chroma_mult = chroma_mult;
        self.neutrals = neutrals;
    }

    /// The default light theme (accent hue 262).
    pub fn light() -> Self {
        Self::from_accent(262.0, Mode::Light)
    }

    /// The default dark theme (accent hue 262).
    pub fn dark() -> Self {
        Self::from_accent(262.0, Mode::Dark)
    }

    /// Resolves a shadow token to concrete layers for this mode. Dark mode
    /// multiplies alphas by 1.6, and every layer is tinted with the surface
    /// hue (see [`Theme::shadow_tint`]) rather than flat black.
    pub fn shadow(&self, token: ShadowToken) -> Vec<Shadow> {
        let factor = match self.mode {
            Mode::Light => 1.0,
            Mode::Dark => DARK_SHADOW_ALPHA_FACTOR,
        };
        let tint = self.shadow_tint();
        shadow_layers(token)
            .iter()
            .map(|&(dy, blur, alpha)| Shadow {
                dx: 0.0,
                dy,
                blur,
                spread: 0.0,
                color: tint.with_alpha((alpha * factor).min(1.0)),
            })
            .collect()
    }

    /// The base shadow color: a near-black carrying the surface hue at low
    /// chroma. Shadows on a cool theme read cool, on a warm theme warm — the
    /// web-craft alternative to flat `#000`. Alpha is applied per layer.
    pub fn shadow_tint(&self) -> Color {
        let [r, g, b, _] = self.bg.components;
        // A pure-gray surface carries no hue, so its shadow is a neutral
        // near-black — not an arbitrary one. (The color crate reports a fixed
        // hue, never NaN, for achromatic colors, so checking the sRGB channels
        // is the reliable test.)
        if (r - g).abs() < 1e-4 && (g - b).abs() < 1e-4 {
            return oklch(0.13, 0.0, 0.0);
        }
        let hue = self.bg.convert::<Oklch>().components[2];
        let hue = if hue.is_nan() { 0.0 } else { hue };
        oklch(0.13, 0.03, hue)
    }

    /// The surface color at an elevation level: 0 is `surface`, 1 is
    /// `surface_raised`, and each level above adds +0.025 L in dark mode
    /// (light mode raised surfaces are always pure white), because shadows
    /// alone read poorly on dark backgrounds.
    pub fn elevated_surface(&self, level: u8) -> Color {
        match (self.mode, level) {
            (_, 0) => self.surface,
            (Mode::Light, _) => self.surface_raised,
            (Mode::Dark, n) => {
                // Lighten from N3 in the theme's own neutral field (duotone
                // hue + boosted chroma included), not the stock accent hue.
                let (l, c) = NEUTRAL_DARK[2];
                oklch(
                    l + DARK_ELEVATION_TINT * f32::from(n - 1),
                    c * self.neutral_chroma_mult,
                    self.neutral_hue,
                )
            }
        }
    }

    /// A stable, human-readable dump of every generated color, used to lock
    /// the theme with a text snapshot.
    pub fn dump(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();
        let mode = match self.mode {
            Mode::Light => "light",
            Mode::Dark => "dark",
        };
        writeln!(out, "theme: accent_hue {} mode {}", self.accent_hue, mode).unwrap();
        writeln!(out, "\nneutrals:").unwrap();
        for n in 1..=12 {
            writeln!(out, "  N{n}: {}", hex(self.neutrals.step(n))).unwrap();
        }
        writeln!(out, "\naccents:").unwrap();
        for n in 1..=12 {
            writeln!(out, "  A{n}: {}", hex(self.accents.step(n))).unwrap();
        }
        writeln!(out, "\nneutral alpha twins (over bg):").unwrap();
        for n in 1..=12 {
            writeln!(out, "  NA{n}: {}", hex(self.neutral_alpha.step(n))).unwrap();
        }
        writeln!(out, "\naccent alpha twins (over bg):").unwrap();
        for n in 1..=12 {
            writeln!(out, "  AA{n}: {}", hex(self.accent_alpha.step(n))).unwrap();
        }
        for (name, s) in [
            ("danger", &self.danger),
            ("warning", &self.warning),
            ("success", &self.success),
        ] {
            writeln!(out, "\n{name}:").unwrap();
            writeln!(out, "  bg: {}", hex(s.bg)).unwrap();
            writeln!(out, "  border: {}", hex(s.border)).unwrap();
            writeln!(out, "  solid: {}", hex(s.solid)).unwrap();
            writeln!(out, "  solid_hover: {}", hex(s.solid_hover)).unwrap();
            writeln!(out, "  solid_active: {}", hex(s.solid_active)).unwrap();
            writeln!(out, "  text: {}", hex(s.text)).unwrap();
        }
        writeln!(out, "\nroles:").unwrap();
        for (name, c) in [
            ("bg", self.bg),
            ("surface", self.surface),
            ("surface_raised", self.surface_raised),
            ("element", self.element),
            ("element_hover", self.element_hover),
            ("element_active", self.element_active),
            ("border_subtle", self.border_subtle),
            ("border", self.border),
            ("border_strong", self.border_strong),
            ("text", self.text),
            ("text_muted", self.text_muted),
            ("text_subtle", self.text_subtle),
            ("text_disabled", self.text_disabled),
            ("accent", self.accent),
            ("accent_hover", self.accent_hover),
            ("accent_active", self.accent_active),
            ("accent_bg", self.accent_bg),
            ("accent_border", self.accent_border),
            ("accent_text", self.accent_text),
            ("on_accent", self.on_accent),
        ] {
            writeln!(out, "  {name}: {}", hex(c)).unwrap();
        }
        writeln!(out, "\nelevation:").unwrap();
        for level in 0..=2 {
            writeln!(
                out,
                "  level {level}: {}",
                hex(self.elevated_surface(level))
            )
            .unwrap();
        }
        writeln!(out, "\nshadows (dx dy blur spread color):").unwrap();
        for (name, token) in [
            ("xs", ShadowToken::Xs),
            ("sm", ShadowToken::Sm),
            ("md", ShadowToken::Md),
            ("lg", ShadowToken::Lg),
            ("xl", ShadowToken::Xl),
        ] {
            let layers: Vec<String> = self
                .shadow(token)
                .iter()
                .map(|s| {
                    format!(
                        "({} {} {} {} {})",
                        s.dx,
                        s.dy,
                        s.blur,
                        s.spread,
                        hex(s.color)
                    )
                })
                .collect();
            writeln!(out, "  {name}: {}", layers.join(" + ")).unwrap();
        }
        out
    }

    /// Measures every text/background role pair against its APCA Lc floor and
    /// returns the pairs that fall short (empty means the theme is legible
    /// everywhere). Floors by role: primary text [`PRIMARY_TEXT_MIN`],
    /// secondary/muted text [`SECONDARY_TEXT_MIN`], labels on filled controls
    /// [`CONTROL_LABEL_MIN`], and colored accent/status text
    /// [`COMPONENT_TEXT_MIN`]. Borders and other non-text contrast are not
    /// checked — APCA scores text legibility, not delineation.
    pub fn contrast_report(&self) -> Vec<ContrastViolation> {
        let mut out = Vec::new();
        // Primary body text on every surface it sits on.
        check_pair(&mut out, "text on bg", self.text, self.bg, PRIMARY_TEXT_MIN);
        check_pair(
            &mut out,
            "text on surface",
            self.text,
            self.surface,
            PRIMARY_TEXT_MIN,
        );
        check_pair(
            &mut out,
            "text on surface_raised",
            self.text,
            self.surface_raised,
            PRIMARY_TEXT_MIN,
        );
        // Secondary (muted) text.
        check_pair(
            &mut out,
            "text_muted on bg",
            self.text_muted,
            self.bg,
            SECONDARY_TEXT_MIN,
        );
        check_pair(
            &mut out,
            "text_muted on surface",
            self.text_muted,
            self.surface,
            SECONDARY_TEXT_MIN,
        );
        check_pair(
            &mut out,
            "text_muted on surface_raised",
            self.text_muted,
            self.surface_raised,
            SECONDARY_TEXT_MIN,
        );
        // Labels painted on filled controls (primary button, pressed states).
        check_pair(
            &mut out,
            "on_accent on accent",
            self.on_accent,
            self.accent,
            CONTROL_LABEL_MIN,
        );
        check_pair(
            &mut out,
            "on_accent on accent_hover",
            self.on_accent,
            self.accent_hover,
            CONTROL_LABEL_MIN,
        );
        check_pair(
            &mut out,
            "on_accent on accent_active",
            self.on_accent,
            self.accent_active,
            CONTROL_LABEL_MIN,
        );
        // Accent-colored text (links, selected option, avatar initials).
        check_pair(
            &mut out,
            "accent_text on bg",
            self.accent_text,
            self.bg,
            COMPONENT_TEXT_MIN,
        );
        check_pair(
            &mut out,
            "accent_text on accent_bg",
            self.accent_text,
            self.accent_bg,
            COMPONENT_TEXT_MIN,
        );
        // Status colors: tinted text on its tint and on the page, plus the
        // label painted on the solid status fill (e.g. a danger button).
        for (name, s) in [
            ("danger", &self.danger),
            ("warning", &self.warning),
            ("success", &self.success),
        ] {
            check_pair(
                &mut out,
                format!("{name}.text on {name}.bg"),
                s.text,
                s.bg,
                COMPONENT_TEXT_MIN,
            );
            check_pair(
                &mut out,
                format!("{name}.text on bg"),
                s.text,
                self.bg,
                COMPONENT_TEXT_MIN,
            );
            check_pair(
                &mut out,
                format!("on_accent on {name}.solid"),
                self.on_accent,
                s.solid,
                CONTROL_LABEL_MIN,
            );
        }
        out
    }

    /// `Ok(())` when every text/background pair clears its APCA floor, else the
    /// list of violations. This is the contract behind fenestra's
    /// "provably-legible themes": the stock themes and every shipped Look are
    /// asserted to pass in headless tests, and any custom [`Theme`] can be
    /// validated the same way.
    ///
    /// # Errors
    /// Returns the pairs that fall below their floor; see [`ContrastViolation`].
    pub fn validate_contrast(&self) -> Result<(), Vec<ContrastViolation>> {
        let report = self.contrast_report();
        if report.is_empty() {
            Ok(())
        } else {
            Err(report)
        }
    }
}

fn make_ramp(table: &RampTable, hue: f32) -> Ramp {
    Ramp(std::array::from_fn(|i| {
        let (l, c) = table[i];
        oklch(l, c, hue)
    }))
}

fn ramp_color(table: &RampTable, step: usize, hue: f32) -> Color {
    let (l, c) = table[step - 1];
    oklch(l, c, hue)
}

/// Pressed state: one OKLCH-lightness notch (`ACTIVE_DL`) below the step-10
/// hover, at `hue`. Read from the table so it is mode-invariant wherever the
/// table's step 10 is — and both the brand accent and the status hues are.
fn active_color(table: &RampTable, hue: f32) -> Color {
    let (l, c) = table[9];
    oklch((l - ACTIVE_DL).max(0.0), c, hue)
}

/// The alpha-twin ramp: each solid step rendered as the smallest-alpha
/// translucent color that composites over `bg` back to that step.
fn alpha_ramp(solid: &Ramp, bg: Color) -> Ramp {
    Ramp(std::array::from_fn(|i| alpha_twin(solid.0[i], bg)))
}

/// The smallest-alpha translucent color that, painted over `bg`, reproduces
/// `target`. For each channel the minimal alpha that keeps the back-solved
/// foreground inside `[0, 1]` is required; the max across channels wins
/// (mixed-direction channels — a tint both bluer and darker than a near-white
/// bg — force alpha toward 1). Reconstruction is exact at f32 precision, so
/// compositing the twin over `bg` round-trips to `target`.
fn alpha_twin(target: Color, bg: Color) -> Color {
    let t = target.components;
    let b = bg.components;
    let mut a = 0.0_f32;
    for ch in 0..3 {
        let (tc, bc) = (t[ch], b[ch]);
        let bound = if tc < bc {
            if bc > 0.0 { 1.0 - tc / bc } else { 0.0 }
        } else if tc > bc {
            if bc < 1.0 {
                (tc - bc) / (1.0 - bc)
            } else {
                1.0
            }
        } else {
            0.0
        };
        a = a.max(bound);
    }
    let a = a.clamp(0.0, 1.0);
    if a <= f32::EPSILON {
        // Fully transparent: the color is immaterial; keep bg for a stable dump.
        return Color::new([b[0], b[1], b[2], 0.0]);
    }
    let solve = |tc: f32, bc: f32| ((tc - bc * (1.0 - a)) / a).clamp(0.0, 1.0);
    Color::new([solve(t[0], b[0]), solve(t[1], b[1]), solve(t[2], b[2]), a])
}

/// Records a [`ContrastViolation`] when `text` on `bg` falls below `floor`
/// (by APCA Lc magnitude).
fn check_pair(
    out: &mut Vec<ContrastViolation>,
    pair: impl Into<String>,
    text: Color,
    bg: Color,
    floor: f64,
) {
    let measured_lc = crate::apca::lc_abs(text, bg);
    if measured_lc < floor {
        out.push(ContrastViolation {
            pair: pair.into(),
            measured_lc,
            required_lc: floor,
        });
    }
}

/// Converts OKLCH to sRGB, gamut-mapping by reducing chroma — never
/// lightness — when the color is out of gamut.
fn oklch(l: f32, c: f32, h: f32) -> Color {
    let convert = |chroma: f32| AlphaColor::<Oklch>::new([l, chroma, h, 1.0]).convert::<Srgb>();
    let mut srgb = convert(c);
    if !in_gamut(srgb) {
        let (mut lo, mut hi) = (0.0_f32, c);
        for _ in 0..24 {
            let mid = 0.5 * (lo + hi);
            if in_gamut(convert(mid)) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        srgb = convert(lo);
    }
    let [r, g, b, a] = srgb.components;
    Color::new([r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0), a])
}

fn in_gamut(c: AlphaColor<Srgb>) -> bool {
    c.components[..3]
        .iter()
        .all(|&v| (-1e-4..=1.0 + 1e-4).contains(&v))
}

/// `#rrggbb` (or `#rrggbbaa` when translucent) for snapshots and debugging.
fn hex(c: Color) -> String {
    let rgba = c.to_rgba8();
    if rgba.a == 255 {
        format!("#{:02x}{:02x}{:02x}", rgba.r, rgba.g, rgba.b)
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", rgba.r, rgba.g, rgba.b, rgba.a)
    }
}

/// A serializable theme *recipe*: the few numbers a theme generates
/// from, not hundreds of resolved colors — so files stay tiny, stable
/// across fenestra versions, and hand-editable.
///
/// ```json
/// {"mode": "dark", "duotone": {"neutral_hue": 152.0, "chroma": 6.0, "accent_hue": 72.0}}
/// ```
///
/// Precedence: `derive` wins over `duotone` wins over `accent_hue`; none
/// means the stock palette for `mode`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeSpec {
    /// Light or dark.
    pub mode: Mode,
    /// Accent hue in OKLCH degrees (`Theme::from_accent`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent_hue: Option<f32>,
    /// Duotone field (`Theme::duotone`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duotone: Option<DuotoneSpec>,
    /// Three-input derivation (`Theme::derive`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derive: Option<DeriveSpec>,
}

/// The duotone parameters of a [`ThemeSpec`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DuotoneSpec {
    /// Neutral ramp hue (OKLCH degrees).
    pub neutral_hue: f32,
    /// Chroma multiplier for the neutral ramp.
    pub chroma: f32,
    /// Accent hue (OKLCH degrees).
    pub accent_hue: f32,
}

/// The three-input parameters of a [`ThemeSpec`] (`Theme::derive`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeriveSpec {
    /// Neutral-field hue (OKLCH degrees).
    pub base_hue: f32,
    /// Neutral-field chroma multiplier.
    pub base_chroma: f32,
    /// Accent hue (OKLCH degrees).
    pub accent_hue: f32,
    /// Contrast level (defaults to `standard`).
    #[serde(default)]
    pub contrast: Contrast,
}

impl ThemeSpec {
    /// Resolves the recipe into a full [`Theme`].
    pub fn theme(&self) -> Theme {
        if let Some(d) = &self.derive {
            return Theme::derive(
                BaseField {
                    hue: d.base_hue,
                    chroma: d.base_chroma,
                },
                d.accent_hue,
                d.contrast,
                self.mode,
            );
        }
        if let Some(d) = &self.duotone {
            return Theme::duotone(d.neutral_hue, d.chroma, d.accent_hue, self.mode);
        }
        if let Some(hue) = self.accent_hue {
            return Theme::from_accent(hue, self.mode);
        }
        match self.mode {
            Mode::Light => Theme::light(),
            Mode::Dark => Theme::dark(),
        }
    }

    /// Parses a theme file's JSON. Unknown fields are errors — a typo'd
    /// recipe should fail loudly, not silently fall back.
    ///
    /// # Errors
    /// On malformed JSON or unknown fields.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// The recipe as pretty JSON, for writing theme files.
    ///
    /// # Panics
    /// Never (the type always serializes).
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("ThemeSpec serializes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// OKLCH lightness of a resolved color (for ordering assertions).
    fn lightness(c: Color) -> f32 {
        c.convert::<Oklch>().components[0]
    }

    /// OKLCH hue (degrees) of a resolved color.
    fn lch_hue(c: Color) -> f32 {
        c.convert::<Oklch>().components[2]
    }

    /// Circular distance between two hues in degrees.
    fn hue_delta(a: f32, b: f32) -> f32 {
        let d = (a - b).rem_euclid(360.0);
        d.min(360.0 - d)
    }

    /// Composite a translucent color over an opaque background (straight
    /// source-over), returning RGB in 0..1.
    fn composite_over(fg: Color, bg: Color) -> [f32; 3] {
        let f = fg.components;
        let b = bg.components;
        let a = f[3];
        std::array::from_fn(|i| f[i] * a + b[i] * (1.0 - a))
    }

    #[test]
    fn alpha_twins_composite_back_to_solid_steps() {
        for theme in [Theme::light(), Theme::dark()] {
            let bg = theme.bg;
            for n in 1..=12 {
                for (solid, twin) in [
                    (theme.neutrals.step(n), theme.neutral_alpha.step(n)),
                    (theme.accents.step(n), theme.accent_alpha.step(n)),
                ] {
                    let got = composite_over(twin, bg);
                    let want = solid.components;
                    for ch in 0..3 {
                        assert!(
                            (got[ch] - want[ch]).abs() < 1e-4,
                            "step {n} channel {ch}: twin over bg = {got:?}, want {want:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn element_roles_are_neutral_steps_3_4_5() {
        for theme in [Theme::light(), Theme::dark()] {
            assert_eq!(theme.element.to_rgba8(), theme.neutrals.step(3).to_rgba8());
            assert_eq!(
                theme.element_hover.to_rgba8(),
                theme.neutrals.step(4).to_rgba8()
            );
            assert_eq!(
                theme.element_active.to_rgba8(),
                theme.neutrals.step(5).to_rgba8()
            );
        }
    }

    #[test]
    fn pressed_states_are_darker_than_hover() {
        for theme in [Theme::light(), Theme::dark()] {
            assert!(
                lightness(theme.accent_active) < lightness(theme.accent_hover),
                "accent_active must be darker than accent_hover"
            );
            for status in [theme.danger, theme.warning, theme.success] {
                assert!(
                    lightness(status.solid_active) < lightness(status.solid_hover),
                    "solid_active must be darker than solid_hover"
                );
            }
        }
    }

    #[test]
    fn light_accent_active_lands_on_a11_lightness() {
        // ACTIVE_DL = 0.045 drops A10 (L 0.545) onto A11's lightness (0.500).
        let active_l = lightness(Theme::light().accent_active);
        assert!(
            (active_l - 0.500).abs() < 0.01,
            "light accent_active L = {active_l}, expected ~0.500"
        );
    }

    #[test]
    fn pressed_states_are_mode_invariant() {
        let (l, d) = (Theme::light(), Theme::dark());
        assert_eq!(l.accent_active.to_rgba8(), d.accent_active.to_rgba8());
        assert_eq!(
            l.danger.solid_active.to_rgba8(),
            d.danger.solid_active.to_rgba8()
        );
    }

    #[test]
    fn elevated_surface_level_1_equals_surface_raised() {
        for theme in [Theme::dark(), Theme::duotone(152.0, 6.0, 72.0, Mode::Dark)] {
            assert_eq!(
                theme.elevated_surface(1).to_rgba8(),
                theme.surface_raised.to_rgba8()
            );
        }
    }

    #[test]
    fn duotone_dark_elevation_tracks_the_field_not_the_accent() {
        // Regression: dark elevated surfaces must follow the duotone field hue
        // (152), not the accent hue (72) — the editorial-Look overlay bug.
        let t = Theme::duotone(152.0, 6.0, 72.0, Mode::Dark);
        let field = lch_hue(t.surface_raised);
        let lifted = lch_hue(t.elevated_surface(2));
        assert!(
            hue_delta(lifted, field) < 25.0,
            "elev(2) hue {lifted} vs field {field}"
        );
        assert!(
            hue_delta(lifted, 72.0) > 40.0,
            "elev(2) must not be the accent hue 72"
        );
    }

    #[test]
    fn shadow_tint_is_neutral_for_gray_themes_and_hued_otherwise() {
        // A grayscale (chroma 0) theme gets a neutral near-black shadow.
        let [r, g, b, _] = Theme::duotone(152.0, 0.0, 72.0, Mode::Dark)
            .shadow_tint()
            .components;
        assert!(
            (r - g).abs() < 1e-3 && (g - b).abs() < 1e-3,
            "gray theme shadow must be neutral, got {:?}",
            [r, g, b]
        );
        // The stock theme keeps a subtle hue.
        let [r2, _, b2, _] = Theme::dark().shadow_tint().components;
        assert!(
            (r2 - b2).abs() > 1e-3,
            "stock shadow tint should carry a hue"
        );
    }

    #[test]
    fn derive_reproduces_from_accent_and_duotone() {
        // derive is the general path: at chroma 1.0 / Standard contrast it is
        // from_accent, and at a boosted chroma / Standard it is duotone.
        for mode in [Mode::Light, Mode::Dark] {
            let as_accent = Theme::derive(
                BaseField {
                    hue: 262.0,
                    chroma: 1.0,
                },
                262.0,
                Contrast::Standard,
                mode,
            );
            assert_eq!(as_accent.dump(), Theme::from_accent(262.0, mode).dump());

            let as_duotone = Theme::derive(
                BaseField {
                    hue: 152.0,
                    chroma: 6.0,
                },
                72.0,
                Contrast::Standard,
                mode,
            );
            assert_eq!(
                as_duotone.dump(),
                Theme::duotone(152.0, 6.0, 72.0, mode).dump()
            );
        }
    }

    #[test]
    fn derive_contrast_orders_legibility_and_stays_legible() {
        let base = BaseField {
            hue: 262.0,
            chroma: 1.0,
        };
        let low = Theme::derive(base, 262.0, Contrast::Low, Mode::Light);
        let std = Theme::derive(base, 262.0, Contrast::Standard, Mode::Light);
        let high = Theme::derive(base, 262.0, Contrast::High, Mode::Light);
        let lc = |t: &Theme| crate::apca::lc_abs(t.text, t.bg);
        assert!(lc(&high) > lc(&std), "High must be crisper than Standard");
        assert!(lc(&std) > lc(&low), "Standard must be crisper than Low");
        // Every level still clears the APCA floors — derivation never ships an
        // illegible theme.
        for t in [&low, &std, &high] {
            assert!(
                t.validate_contrast().is_ok(),
                "derived theme failed contrast: {:?}",
                t.validate_contrast()
            );
        }
    }

    #[test]
    fn derive_recipe_round_trips() {
        let spec = ThemeSpec {
            mode: Mode::Dark,
            accent_hue: None,
            duotone: None,
            derive: Some(DeriveSpec {
                base_hue: 90.0,
                base_chroma: 2.0,
                accent_hue: 40.0,
                contrast: Contrast::High,
            }),
        };
        let json = spec.to_json();
        let back = ThemeSpec::from_json(&json).expect("round-trip");
        assert_eq!(spec, back);
        // The recipe resolves to the same theme as the direct call.
        let direct = Theme::derive(
            BaseField {
                hue: 90.0,
                chroma: 2.0,
            },
            40.0,
            Contrast::High,
            Mode::Dark,
        );
        assert_eq!(spec.theme().dump(), direct.dump());
    }
}
