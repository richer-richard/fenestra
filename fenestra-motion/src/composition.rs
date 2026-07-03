//! The composition: canvas size, frame rate, duration, background, theme,
//! and the clip list. A composition plus a frame number is everything a
//! sample needs — no other state exists.

use fenestra_anim::Frames;
use fenestra_core::{Color, Theme};

use crate::clip::Clip;
use crate::sample::SampledScene;

/// The largest frame count [`Composition::total_frames`] will report,
/// regardless of a declared `duration` or a clip's span end — both are
/// untrusted (document/CLI-reachable) and otherwise unbounded `u64`s that
/// would make sinks collect a multi-exabyte `Vec<u64>` or loop effectively
/// forever. ~46 hours at 60fps; no real composition approaches this.
const MAX_FRAMES: u64 = 10_000_000;

/// A motion composition: `width × height` logical px at `fps`, holding
/// clips. Sampling is pure — `sample(frame)` depends only on
/// `(composition, frame)`.
pub struct Composition {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) fps: u32,
    pub(crate) duration: Option<Frames>,
    pub(crate) background: Color,
    pub(crate) theme: Theme,
    pub(crate) clips: Vec<Clip>,
    pub(crate) cuts: Vec<Frames>,
    /// The source document when loaded from the data form; code-built
    /// compositions carry `None` and refuse to serialize.
    pub(crate) source: Option<Box<crate::data::MotionDoc>>,
}

impl std::fmt::Debug for Composition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Composition")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("fps", &self.fps)
            .field("duration", &self.total_frames())
            .field(
                "clips",
                &self.clips.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl Composition {
    /// A composition of `width × height` logical pixels at `fps` frames per
    /// second (each clamped to at least 1). The background defaults to
    /// transparent and the theme to light.
    pub fn new(width: u32, height: u32, fps: u32) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
            fps: fps.max(1),
            duration: None,
            background: Color::TRANSPARENT,
            theme: Theme::light(),
            clips: Vec::new(),
            cuts: Vec::new(),
            source: None,
        }
    }

    /// Total length in frames. Defaults to the last clip's span end.
    pub fn duration(mut self, frames: impl Into<Frames>) -> Self {
        self.duration = Some(frames.into());
        self.source = None;
        self
    }

    /// The canvas base color (transparent by default: alpha renders
    /// straight into the PNG sequence).
    pub fn background(mut self, color: Color) -> Self {
        self.background = color;
        self.source = None;
        self
    }

    /// The theme clip content resolves its tokens against (light by
    /// default).
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self.source = None;
        self
    }

    /// Adds a clip. Paint order is insertion order, overridden per clip by
    /// [`Clip::z`].
    ///
    /// # Panics
    /// On a duplicate clip id — ids key assertions and the CLI.
    pub fn clip(mut self, clip: Clip) -> Self {
        assert!(
            !self.clips.iter().any(|c| c.id == clip.id),
            "duplicate clip id {:?}",
            clip.id
        );
        self.clips.push(clip);
        self.source = None;
        self
    }

    /// Declares an intentional discontinuity at `frame`: temporal lints
    /// allow value jumps across declared cuts (and clip span edges) only.
    pub fn cut(mut self, frame: impl Into<Frames>) -> Self {
        self.cuts.push(frame.into());
        self.source = None;
        self
    }

    /// Canvas width in logical px.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Canvas height in logical px.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Frames per second — the only clock there is.
    pub fn fps(&self) -> u32 {
        self.fps
    }

    /// Total frame count: the declared duration, or the furthest clip end
    /// — clamped to `MAX_FRAMES` regardless of source, since both are
    /// untrusted `u64`s reachable from a document or CLI flag.
    #[must_use]
    pub fn total_frames(&self) -> Frames {
        let raw = self
            .duration
            .unwrap_or_else(|| Frames(self.clips.iter().map(|c| c.span.end.0).max().unwrap_or(0)));
        Frames(raw.0.min(MAX_FRAMES))
    }

    /// The canvas base color.
    pub fn background_color(&self) -> Color {
        self.background
    }

    /// Every clip id, in insertion order.
    #[must_use]
    pub fn clip_ids(&self) -> Vec<&str> {
        self.clips.iter().map(|c| c.id.as_str()).collect()
    }

    /// Samples the composition at `frame`: every clip's props resolve, and
    /// the scene can build its element tree, measure bboxes, and report
    /// paint order — all windowless.
    #[must_use]
    pub fn sample(&self, frame: Frames) -> SampledScene<'_> {
        SampledScene::new(self, frame)
    }
}
