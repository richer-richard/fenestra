//! Theme generation: OKLCH color ramps derived from one accent hue, plus the
//! semantic roles, status colors, and shadow scale. The L/C tables here are
//! the design spec; every generated value is locked by an insta snapshot.

use color::{AlphaColor, Oklch, Srgb};
use peniko::Color;

use crate::style::Shadow;
use crate::tokens::ShadowToken;

/// Light or dark color mode. Both are always generated from the same hue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Light backgrounds, dark text.
    Light,
    /// Dark backgrounds, light text.
    Dark,
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

/// The four resolved colors of one status hue.
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
    /// Text on `bg` (step 11).
    pub text: Color,
}

/// Design tokens resolved for one color mode.
#[derive(Debug, Clone)]
pub struct Theme {
    /// The color mode this theme was generated for.
    pub mode: Mode,
    /// The accent hue (OKLCH degrees) this theme was generated from.
    pub accent_hue: f32,
    /// The 12-step neutral ramp, tinted with the accent hue at low chroma.
    pub neutrals: Ramp,
    /// The 12-step accent ramp.
    pub accents: Ramp,
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

/// Shadow tokens as `(dy, blur, alpha)` layers; dx and spread are 0.
const fn shadow_layers(token: ShadowToken) -> &'static [(f32, f32, f32)] {
    match token {
        ShadowToken::Xs => &[(1.0, 2.0, 0.05)],
        ShadowToken::Sm => &[(1.0, 2.0, 0.05), (1.0, 3.0, 0.06)],
        ShadowToken::Md => &[(2.0, 4.0, 0.05), (4.0, 12.0, 0.08)],
        ShadowToken::Lg => &[(4.0, 10.0, 0.06), (16.0, 32.0, 0.12)],
    }
}

/// Dark mode multiplies shadow alphas by this factor: soft black shadows
/// read poorly on dark backgrounds.
const DARK_SHADOW_ALPHA_FACTOR: f32 = 1.6;

/// Per-level lightness boost for raised surfaces in dark mode.
const DARK_ELEVATION_TINT: f32 = 0.025;

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
            danger: status(DANGER_HUE),
            warning: status(WARNING_HUE),
            success: status(SUCCESS_HUE),
            bg: neutrals.step(1),
            surface: neutrals.step(2),
            surface_raised,
            border_subtle: neutrals.step(5),
            border: neutrals.step(6),
            border_strong: neutrals.step(7),
            text: neutrals.step(12),
            text_muted: neutrals.step(11),
            text_subtle: neutrals.step(9),
            text_disabled: neutrals.step(8),
            accent: accents.step(9),
            accent_hover: accents.step(10),
            accent_bg: accents.step(3),
            accent_border: accents.step(7),
            accent_text: accents.step(11),
            on_accent,
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
        let (neutral_table, accent_table) = match mode {
            Mode::Light => (&NEUTRAL_LIGHT, &ACCENT_LIGHT),
            Mode::Dark => (&NEUTRAL_DARK, &ACCENT_DARK),
        };
        let hue = neutral_hue.rem_euclid(360.0);
        let boost = neutral_chroma.clamp(0.0, 40.0);
        let neutrals = Ramp(std::array::from_fn(|i| {
            let (l, c) = neutral_table[i];
            oklch(l, c * boost, hue)
        }));
        theme.bg = neutrals.step(1);
        theme.surface = neutrals.step(2);
        if matches!(mode, Mode::Dark) {
            theme.surface_raised = neutrals.step(3);
        }
        theme.border_subtle = neutrals.step(5);
        theme.border = neutrals.step(6);
        theme.border_strong = neutrals.step(7);
        theme.text = neutrals.step(12);
        theme.text_muted = neutrals.step(11);
        theme.text_subtle = neutrals.step(9);
        theme.text_disabled = neutrals.step(8);
        if accent_table[8].0 >= 0.65 {
            theme.on_accent = neutrals.step(12);
        }
        theme.neutrals = neutrals;
        theme
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
    /// multiplies alphas by 1.6.
    pub fn shadow(&self, token: ShadowToken) -> Vec<Shadow> {
        let factor = match self.mode {
            Mode::Light => 1.0,
            Mode::Dark => DARK_SHADOW_ALPHA_FACTOR,
        };
        shadow_layers(token)
            .iter()
            .map(|&(dy, blur, alpha)| Shadow {
                dx: 0.0,
                dy,
                blur,
                spread: 0.0,
                color: Color::new([0.0, 0.0, 0.0, (alpha * factor).min(1.0)]),
            })
            .collect()
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
                let (l, c) = NEUTRAL_DARK[2];
                oklch(
                    l + DARK_ELEVATION_TINT * f32::from(n - 1),
                    c,
                    self.accent_hue,
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
            writeln!(out, "  text: {}", hex(s.text)).unwrap();
        }
        writeln!(out, "\nroles:").unwrap();
        for (name, c) in [
            ("bg", self.bg),
            ("surface", self.surface),
            ("surface_raised", self.surface_raised),
            ("border_subtle", self.border_subtle),
            ("border", self.border),
            ("border_strong", self.border_strong),
            ("text", self.text),
            ("text_muted", self.text_muted),
            ("text_subtle", self.text_subtle),
            ("text_disabled", self.text_disabled),
            ("accent", self.accent),
            ("accent_hover", self.accent_hover),
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
        writeln!(out, "\nshadows (dx dy blur spread alpha):").unwrap();
        for (name, token) in [
            ("xs", ShadowToken::Xs),
            ("sm", ShadowToken::Sm),
            ("md", ShadowToken::Md),
            ("lg", ShadowToken::Lg),
        ] {
            let layers: Vec<String> = self
                .shadow(token)
                .iter()
                .map(|s| {
                    format!(
                        "({} {} {} {} {:.3})",
                        s.dx, s.dy, s.blur, s.spread, s.color.components[3]
                    )
                })
                .collect();
            writeln!(out, "  {name}: {}", layers.join(" + ")).unwrap();
        }
        out
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
