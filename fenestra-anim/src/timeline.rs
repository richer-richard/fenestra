//! Integer-frame ground truth. Seconds are derived per frame as
//! `frame / fps` — never accumulated across frames.

/// A frame count or frame index. Integer frames are the timeline's ground
/// truth; wall-clock seconds are derived per frame (`frame / fps`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
