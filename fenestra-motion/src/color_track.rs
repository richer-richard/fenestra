//! A color keyframe track: the one value type `fenestra-anim`'s generic
//! `Track` deliberately doesn't cover (see that crate's docs). Built
//! directly on `fenestra-anim`'s `sorted_keys`/`locate` — the same
//! segment-lookup and easing evaluation the generic `Track` uses — with its
//! own Oklab-default, sRGB-opt-in value combination in place of
//! `Interpolate`.

use fenestra_anim::{Frames, Key, Located, locate, sorted_keys};
use fenestra_core::Color;

/// The color space a color track interpolates in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorSpace {
    /// Perceptual Oklab (the default): even lightness ramps, no gray dead
    /// zones on hue-crossing lerps.
    #[default]
    Oklab,
    /// Raw sRGB component lerp — the per-track opt-out for deliberately
    /// digital ramps.
    Srgb,
}

fn mix(a: Color, b: Color, t: f32, space: ColorSpace) -> Color {
    let t = t.clamp(0.0, 1.0);
    if t <= 0.0 {
        return a;
    }
    if t >= 1.0 {
        return b;
    }
    let lerp = |x: [f32; 4], y: [f32; 4]| {
        [
            x[0] + (y[0] - x[0]) * t,
            x[1] + (y[1] - x[1]) * t,
            x[2] + (y[2] - x[2]) * t,
            x[3] + (y[3] - x[3]) * t,
        ]
    };
    match space {
        ColorSpace::Srgb => Color::new(lerp(a.components, b.components)),
        ColorSpace::Oklab => {
            let ao: color::AlphaColor<color::Oklab> = a.convert();
            let bo: color::AlphaColor<color::Oklab> = b.convert();
            let mixed = color::AlphaColor::<color::Oklab>::new(lerp(ao.components, bo.components))
                .convert::<color::Srgb>();
            let [r, g, b, alpha] = mixed.components;
            Color::new([
                r.clamp(0.0, 1.0),
                g.clamp(0.0, 1.0),
                b.clamp(0.0, 1.0),
                alpha.clamp(0.0, 1.0),
            ])
        }
    }
}

/// A typed color keyframe track: holds before the first key and after the
/// last, returns key values exactly on their frames, and eases each segment
/// by its leading key's `Ease` — the same contract as `fenestra_anim::Track`,
/// specialized for perceptual color mixing. A single-key track is a
/// constant.
#[derive(Debug, Clone)]
pub struct ColorTrack {
    pub(crate) keys: Vec<Key<Color>>,
    space: ColorSpace,
}

impl ColorTrack {
    /// Builds a color track from keys (any authoring order; sorted by
    /// frame). Oklab by default; see [`ColorTrack::srgb`].
    ///
    /// # Panics
    /// On an empty key list or two keys sharing a frame — a track must be a
    /// function of frame.
    pub fn new(keys: impl IntoIterator<Item = Key<Color>>) -> Self {
        Self {
            keys: sorted_keys(keys),
            space: ColorSpace::default(),
        }
    }

    /// Switches this color track to raw sRGB component interpolation
    /// (perceptual Oklab is the default).
    pub fn srgb(mut self) -> Self {
        self.space = ColorSpace::Srgb;
        self
    }

    /// Samples the track at a track-relative frame. `fps` grounds spring
    /// segments in seconds (derived per frame, never accumulated).
    pub fn sample(&self, frame: Frames, fps: u32) -> Color {
        match locate(&self.keys, frame, fps) {
            Located::Boundary(v) => v,
            Located::Interior { from, to, eased } => mix(from, to, eased, self.space),
        }
    }

    /// The frames this track has keys at, in ascending order.
    pub fn key_frames(&self) -> impl Iterator<Item = Frames> + '_ {
        self.keys.iter().map(Key::at)
    }
}
