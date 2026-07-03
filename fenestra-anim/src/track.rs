//! Typed keyframe tracks and sampling.

use crate::easing::Ease;
use crate::interpolate::Interpolate;
use crate::timeline::Frames;

/// One typed keyframe: a value at a track-relative frame, easing into the
/// segment that follows it.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Key<T> {
    pub(crate) at: Frames,
    pub(crate) value: T,
    pub(crate) ease: Ease,
}

impl<T> Key<T> {
    /// The track-relative frame this key sits at.
    pub fn at(&self) -> Frames {
        self.at
    }

    /// Sets how this key eases into the next segment (linear by default).
    /// Accepts an [`Ease`], a bare [`CubicBezier`](crate::CubicBezier), or a
    /// [`Spring`](crate::Spring).
    pub fn ease(mut self, ease: impl Into<Ease>) -> Self {
        self.ease = ease.into();
        self
    }
}

/// A keyframe at track-relative frame `at` (frame 0 = the track's first
/// active frame), easing linearly into the next segment until overridden
/// with [`Key::ease`].
pub fn key<T>(at: impl Into<Frames>, value: T) -> Key<T> {
    Key {
        at: at.into(),
        value,
        ease: Ease::Linear,
    }
}

/// Validates and frame-sorts a raw key list: the bookkeeping behind
/// [`Track::new`], exposed so a consumer whose value type doesn't implement
/// [`Interpolate`] (e.g. a perceptual color type that needs its own mixing
/// space) can build a track-like container on the same invariants without
/// re-deriving them.
///
/// # Panics
/// On an empty key list or two keys sharing a frame — a track is a function
/// of frame.
pub fn sorted_keys<T>(keys: impl IntoIterator<Item = Key<T>>) -> Vec<Key<T>> {
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
    keys
}

/// Where a frame falls relative to a sorted, validated key list (see
/// [`sorted_keys`]).
#[derive(Debug, Clone, Copy)]
pub enum Located<T> {
    /// At or before the first key, or at/after the last: hold that value.
    Boundary(T),
    /// Strictly between two keys: interpolate `from -> to` at `eased`
    /// progress, which may leave `0..=1` under overshoot easing.
    Interior {
        /// The leading key's value.
        from: T,
        /// The trailing key's value.
        to: T,
        /// The eased progress between them.
        eased: f32,
    },
}

/// Locates `frame` within `keys` (as produced by [`sorted_keys`]) and
/// evaluates the leading key's easing for an interior frame. `fps` grounds
/// spring segments in seconds (derived per frame, never accumulated).
/// Exposed for the same reason as [`sorted_keys`]: reuse without
/// re-deriving the easing math.
///
/// # Panics
/// If `keys` is empty (violates the [`sorted_keys`] invariant).
pub fn locate<T: Copy>(keys: &[Key<T>], frame: Frames, fps: u32) -> Located<T> {
    assert!(!keys.is_empty(), "locate: empty key list");
    let last = keys.len() - 1;
    if frame <= keys[0].at {
        return Located::Boundary(keys[0].value);
    }
    if frame >= keys[last].at {
        return Located::Boundary(keys[last].value);
    }
    // keys[i].at <= frame < keys[i + 1].at; the boundary returns above
    // guarantee both neighbors exist.
    let i = keys.partition_point(|k| k.at <= frame) - 1;
    let (k0, k1) = (&keys[i], &keys[i + 1]);
    if k0.at == frame {
        return Located::Boundary(k0.value);
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
    Located::Interior {
        from: k0.value,
        to: k1.value,
        eased,
    }
}

/// A typed keyframe track: holds before the first key and after the last,
/// returns key values exactly on their frames, and eases each segment by its
/// leading key's [`Ease`]. A single-key track is a constant.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Track<T> {
    pub(crate) keys: Vec<Key<T>>,
}

impl<T: Interpolate> Track<T> {
    /// Builds a track from keys (any authoring order; sorted by frame).
    ///
    /// # Panics
    /// On an empty key list or two keys sharing a frame — a track must be a
    /// function of frame.
    pub fn new(keys: impl IntoIterator<Item = Key<T>>) -> Self {
        Self {
            keys: sorted_keys(keys),
        }
    }

    /// Samples the track at a track-relative frame. `fps` grounds spring
    /// segments in seconds (derived per frame, never accumulated).
    pub fn sample(&self, frame: Frames, fps: u32) -> T {
        match locate(&self.keys, frame, fps) {
            Located::Boundary(v) => v,
            Located::Interior { from, to, eased } => T::interpolate(from, to, eased),
        }
    }

    /// The frames this track has keys at, in ascending order — e.g. for a
    /// consumer that auto-selects sentinel/golden frames from keyframe
    /// boundaries.
    pub fn key_frames(&self) -> impl Iterator<Item = Frames> + '_ {
        self.keys.iter().map(Key::at)
    }
}
