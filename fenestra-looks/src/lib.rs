//! Looks: complete design languages for fenestra — theme, typefaces,
//! and character bundled into one value and applied in one call. The
//! same app, five voices:
//!
//! - [`product`] — the stock voice: Inter, neutral surfaces, blue
//!   accent. What the kit ships as.
//! - [`editorial`] — print energy: Playfair Display headlines over a
//!   deep duotone field (the poster's language, packaged).
//! - [`terminal`] — instrument panel: JetBrains Mono everywhere,
//!   phosphor-green accent, built for dense tools.
//! - [`warm_editorial`] — warm paper and ink: a cream-and-terracotta field
//!   ([`Theme::derive`]) with Playfair serif prose under sans chrome.
//! - [`playful`] — a soft pastel canvas with a saturated accent, for
//!   whiteboard-class, friendly tools.
//!
//! ```
//! use fenestra_core::Mode;
//! let look = fenestra_looks::editorial(Mode::Dark);
//! let theme = look.theme.clone();
//! let fonts = look.fonts(); // embedded base + the look's faces
//! ```
//!
//! Typefaces are vendored under their OFL licenses (see `assets/`).

use fenestra_core::{BaseField, Contrast, FamilyRole, Fonts, Mode, Theme};

/// A packaged design language: a resolved theme plus the typefaces
/// that give it its voice.
pub struct Look {
    /// Stable identifier ("product", "editorial", "terminal").
    pub name: &'static str,
    /// The resolved theme for the requested mode.
    pub theme: Theme,
    /// Faces to register, by family role.
    pub faces: Vec<(FamilyRole, &'static [u8])>,
}

impl Look {
    /// Fonts for headless rendering: the embedded base set plus this
    /// look's faces. (Windowed apps: register the same faces via
    /// `WindowOptions::with_font`, or start from `Fonts::with_system`.)
    pub fn fonts(&self) -> Fonts {
        let mut fonts = Fonts::embedded();
        for (role, bytes) in &self.faces {
            fonts.register(*role, bytes.to_vec());
        }
        fonts
    }
}

const PLAYFAIR: &[u8] = include_bytes!("../assets/PlayfairDisplay.ttf");
const PLAYFAIR_ITALIC: &[u8] = include_bytes!("../assets/PlayfairDisplay-Italic.ttf");
const JB_MONO: &[u8] = include_bytes!("../assets/JetBrainsMono-Regular.ttf");
const JB_MONO_MEDIUM: &[u8] = include_bytes!("../assets/JetBrainsMono-Medium.ttf");
const JB_MONO_SEMIBOLD: &[u8] = include_bytes!("../assets/JetBrainsMono-SemiBold.ttf");
const JB_MONO_BOLD: &[u8] = include_bytes!("../assets/JetBrainsMono-Bold.ttf");

/// The stock voice: Inter, neutral surfaces, the default accent.
pub fn product(mode: Mode) -> Look {
    Look {
        name: "product",
        theme: match mode {
            Mode::Light => Theme::light(),
            Mode::Dark => Theme::dark(),
        },
        faces: Vec::new(),
    }
}

/// Print energy: Playfair Display headlines (Display + Serif roles)
/// over the deep-green duotone field the poster proved.
pub fn editorial(mode: Mode) -> Look {
    Look {
        name: "editorial",
        theme: Theme::duotone(152.0, 6.0, 72.0, mode),
        faces: vec![
            (FamilyRole::Display, PLAYFAIR),
            (FamilyRole::Serif, PLAYFAIR_ITALIC),
        ],
    }
}

/// Instrument panel: JetBrains Mono as the body voice, phosphor-green
/// accent, tuned for dense tooling.
pub fn terminal(mode: Mode) -> Look {
    Look {
        name: "terminal",
        theme: Theme::from_accent(145.0, mode),
        // Weight coverage matters: requesting a weight a family lacks
        // (e.g. Semibold headlines) falls back out of the family, so
        // the look ships 400/500/600/700 across the roles.
        faces: vec![
            (FamilyRole::Sans, JB_MONO),
            (FamilyRole::Sans, JB_MONO_MEDIUM),
            (FamilyRole::Sans, JB_MONO_SEMIBOLD),
            (FamilyRole::Display, JB_MONO_BOLD),
            (FamilyRole::Display, JB_MONO_SEMIBOLD),
            (FamilyRole::Mono, JB_MONO),
        ],
    }
}

/// Warm paper and ink: a cream-and-terracotta field with Playfair serif prose
/// under sans chrome. The palette is *derived* — a warm neutral field (hue 80,
/// a touch off gray) and a terracotta accent (hue 40) at crisp contrast — not
/// hand-placed, so light and dark stay coherent. Apps set prose runs to
/// [`FamilyRole::Serif`] and keep controls on the sans default; elevation reads
/// as borders more than shadows on paper.
pub fn warm_editorial(mode: Mode) -> Look {
    Look {
        name: "warm-editorial",
        theme: Theme::derive(
            BaseField {
                hue: 80.0,
                chroma: 2.5,
            },
            40.0,
            Contrast::High,
            mode,
        ),
        faces: vec![
            (FamilyRole::Serif, PLAYFAIR),
            (FamilyRole::Display, PLAYFAIR),
        ],
    }
}

/// A soft pastel canvas with a saturated accent — the whiteboard/FigJam voice:
/// faintly tinted surfaces (derived from a cool field at low chroma) under a
/// vivid magenta accent, so accent strokes pop against pastel fills. (Ships
/// with the base sans; a hand-drawn display face is a future addition.)
pub fn playful(mode: Mode) -> Look {
    Look {
        name: "playful",
        theme: Theme::derive(
            BaseField {
                hue: 280.0,
                chroma: 2.0,
            },
            330.0,
            Contrast::Standard,
            mode,
        ),
        faces: Vec::new(),
    }
}

/// Every shipped look, for galleries and pickers.
pub fn all(mode: Mode) -> Vec<Look> {
    vec![
        product(mode),
        editorial(mode),
        terminal(mode),
        warm_editorial(mode),
        playful(mode),
    ]
}
