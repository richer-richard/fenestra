//! Integer-frame ground truth: [`Frames`], typed keyframes, and [`Track`]
//! sampling. Seconds are derived per frame as `frame / fps` — never
//! accumulated across frames.

use fenestra_core::Color;

use crate::easing::Ease;

/// A frame count or frame index. Integer frames are the timeline's ground
/// truth; wall-clock seconds are derived per frame (`frame / fps`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Frames(pub u64);

impl Frames {
    /// The instant this frame represents at `fps`, in seconds — derived, not
    /// accumulated.
    pub fn seconds(self, fps: u32) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "frame counts sit far below f64's 2^53 integer range"
        )]
        let frame = self.0 as f64;
        frame / f64::from(fps.max(1))
    }
}

impl From<u64> for Frames {
    fn from(n: u64) -> Self {
        Self(n)
    }
}

impl std::fmt::Display for Frames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A half-open span of frames `start..end`: a clip is visible on `start`,
/// gone on `end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRange {
    /// First active frame.
    pub start: Frames,
    /// First frame past the span.
    pub end: Frames,
}

impl FrameRange {
    /// Whether `frame` lies inside the half-open span.
    pub fn contains(self, frame: Frames) -> bool {
        frame >= self.start && frame < self.end
    }

    /// The span's length in frames.
    pub fn len(self) -> Frames {
        Frames(self.end.0.saturating_sub(self.start.0))
    }

    /// Whether the span covers no frames.
    pub fn is_empty(self) -> bool {
        self.end <= self.start
    }
}

impl From<std::ops::Range<u64>> for FrameRange {
    fn from(r: std::ops::Range<u64>) -> Self {
        Self {
            start: Frames(r.start),
            end: Frames(r.end),
        }
    }
}

impl From<std::ops::Range<Frames>> for FrameRange {
    fn from(r: std::ops::Range<Frames>) -> Self {
        Self {
            start: r.start,
            end: r.end,
        }
    }
}

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

/// One typed keyframe: a value at a clip-relative frame, easing into the
/// segment that follows it.
#[derive(Debug, Clone, Copy)]
pub struct Key<T> {
    pub(crate) at: Frames,
    pub(crate) value: T,
    pub(crate) ease: Ease,
}

impl<T> Key<T> {
    /// Sets how this key eases into the next segment (linear by default).
    /// Accepts an [`Ease`], a bare
    /// [`CubicBezier`](fenestra_core::CubicBezier) (e.g. a core Material
    /// token), or a [`Spring`](crate::Spring).
    pub fn ease(mut self, ease: impl Into<Ease>) -> Self {
        self.ease = ease.into();
        self
    }
}

/// A keyframe at clip-relative frame `at` (frame 0 = the clip's first active
/// frame), easing linearly into the next segment until overridden with
/// [`Key::ease`].
pub fn key<T>(at: impl Into<Frames>, value: T) -> Key<T> {
    Key {
        at: at.into(),
        value,
        ease: Ease::Linear,
    }
}

/// A value a track can interpolate. `t` is the eased progress and may leave
/// `0..=1` under overshoot easing: geometry extrapolates, colors clamp —
/// the same rule the interactive transition engine applies.
pub trait TrackValue: Copy {
    /// Interpolates `a → b` at eased progress `t` (see the trait docs for
    /// the out-of-range policy). `space` matters only to colors.
    fn interpolate(a: Self, b: Self, t: f32, space: ColorSpace) -> Self;
}

impl TrackValue for f32 {
    fn interpolate(a: Self, b: Self, t: f32, _space: ColorSpace) -> Self {
        a + (b - a) * t
    }
}

impl TrackValue for (f32, f32) {
    fn interpolate(a: Self, b: Self, t: f32, _space: ColorSpace) -> Self {
        (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
    }
}

impl TrackValue for Color {
    /// Colors clamp the eased progress into `0..=1` (an extrapolated color
    /// isn't a color) and lerp in Oklab by default — componentwise in the
    /// perceptual rectangular space — or raw sRGB on per-track opt-in.
    fn interpolate(a: Self, b: Self, t: f32, space: ColorSpace) -> Self {
        let t = t.clamp(0.0, 1.0);
        if t <= 0.0 {
            return a;
        }
        if t >= 1.0 {
            return b;
        }
        let mix = |x: [f32; 4], y: [f32; 4]| {
            [
                x[0] + (y[0] - x[0]) * t,
                x[1] + (y[1] - x[1]) * t,
                x[2] + (y[2] - x[2]) * t,
                x[3] + (y[3] - x[3]) * t,
            ]
        };
        match space {
            ColorSpace::Srgb => Color::new(mix(a.components, b.components)),
            ColorSpace::Oklab => {
                let ao: color::AlphaColor<color::Oklab> = a.convert();
                let bo: color::AlphaColor<color::Oklab> = b.convert();
                let mixed =
                    color::AlphaColor::<color::Oklab>::new(mix(ao.components, bo.components))
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
}

/// A typed keyframe track: holds before the first key and after the last,
/// returns key values exactly on their frames, and eases each segment by its
/// leading key's [`Ease`]. A single-key track is a constant.
#[derive(Debug, Clone)]
pub struct Track<T> {
    pub(crate) keys: Vec<Key<T>>,
    pub(crate) space: ColorSpace,
}

impl<T: TrackValue> Track<T> {
    /// Builds a track from keys (any authoring order; sorted by frame).
    ///
    /// # Panics
    /// On an empty key list or two keys sharing a frame — a track must be a
    /// function of frame.
    pub fn new(keys: impl IntoIterator<Item = Key<T>>) -> Self {
        let mut keys: Vec<Key<T>> = keys.into_iter().collect();
        assert!(!keys.is_empty(), "a track needs at least one key");
        keys.sort_by_key(|k| k.at);
        for pair in keys.windows(2) {
            assert!(
                pair[0].at != pair[1].at,
                "duplicate key frame {} — a track is a function of frame",
                pair[0].at
            );
        }
        Self {
            keys,
            space: ColorSpace::default(),
        }
    }

    /// Samples the track at a clip-relative frame. `fps` grounds spring
    /// segments in seconds (derived per frame, never accumulated).
    pub fn sample(&self, frame: Frames, fps: u32) -> T {
        let keys = &self.keys;
        let last = keys.len() - 1;
        if frame <= keys[0].at {
            return keys[0].value;
        }
        if frame >= keys[last].at {
            return keys[last].value;
        }
        // keys[i].at <= frame < keys[i + 1].at; the boundary returns above
        // guarantee both neighbors exist.
        let i = keys.partition_point(|k| k.at <= frame) - 1;
        let (k0, k1) = (&keys[i], &keys[i + 1]);
        if k0.at == frame {
            return k0.value;
        }
        let run = Frames(frame.0 - k0.at.0);
        let span = k1.at.0 - k0.at.0;
        #[expect(
            clippy::cast_precision_loss,
            reason = "segment lengths sit far below f64's integer range"
        )]
        let u = (run.0 as f64 / span as f64) as f32;
        let eased = match k0.ease {
            Ease::Linear => u,
            Ease::Hold => 0.0,
            Ease::Bezier(curve) => curve.eval(u),
            #[expect(
                clippy::cast_possible_truncation,
                reason = "segment spans are short time spans"
            )]
            Ease::Spring(s) => s.progress(run.seconds(fps) as f32),
        };
        T::interpolate(k0.value, k1.value, eased, self.space)
    }
}

impl Track<Color> {
    /// Switches this color track to raw sRGB component interpolation
    /// (perceptual Oklab is the default).
    pub fn srgb(mut self) -> Self {
        self.space = ColorSpace::Srgb;
        self
    }
}
