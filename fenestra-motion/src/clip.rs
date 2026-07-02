//! Clips: an id, an active span, content (a rebuilt-per-frame element
//! factory or a frame-driven closure), an anchor, and typed animation
//! tracks.

use fenestra_core::{Color, Element};

use crate::timeline::{FrameRange, Frames, Track};

/// An animatable property of a clip. Transform props apply about the clip's
/// [`Anchor`]; color props style the clip's *root* element (fenestra styles
/// don't cascade, so deeper nodes are [`Clip::dynamic`]'s business).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Prop {
    /// Subtree opacity, `0.0..=1.0`.
    Opacity,
    /// Paint-time x translation in logical px.
    TranslateX,
    /// Paint-time y translation in logical px.
    TranslateY,
    /// Uniform paint-time scale about the anchor.
    Scale,
    /// Non-uniform paint-time scale `(x, y)` about the anchor.
    ScaleXY,
    /// Paint-time rotation in degrees about the anchor.
    Rotate,
    /// The root element's fill (background) color.
    FillColor,
    /// The root element's border color (recolors an existing border; fenestra
    /// paints path strokes with the text color, so paths take
    /// [`Prop::TextColor`]).
    StrokeColor,
    /// The root element's text color.
    TextColor,
}

/// The value shape a prop's track must carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PropKind {
    Scalar,
    Pair,
    Color,
}

impl Prop {
    fn kind(self) -> PropKind {
        match self {
            Self::Opacity | Self::TranslateX | Self::TranslateY | Self::Scale | Self::Rotate => {
                PropKind::Scalar
            }
            Self::ScaleXY => PropKind::Pair,
            Self::FillColor | Self::StrokeColor | Self::TextColor => PropKind::Color,
        }
    }
}

/// A typed track behind one [`Prop`]; built from `Track<f32>`,
/// `Track<(f32, f32)>`, or `Track<Color>` via `From`.
#[derive(Debug, Clone)]
pub enum AnyTrack {
    /// An `f32` track (opacity, translate axes, uniform scale, rotate).
    Scalar(Track<f32>),
    /// An `(f32, f32)` track ([`Prop::ScaleXY`]).
    Pair(Track<(f32, f32)>),
    /// A color track (fill / stroke / text).
    Color(Track<Color>),
}

impl AnyTrack {
    fn kind(&self) -> PropKind {
        match self {
            Self::Scalar(_) => PropKind::Scalar,
            Self::Pair(_) => PropKind::Pair,
            Self::Color(_) => PropKind::Color,
        }
    }
}

impl From<Track<f32>> for AnyTrack {
    fn from(t: Track<f32>) -> Self {
        Self::Scalar(t)
    }
}

impl From<Track<(f32, f32)>> for AnyTrack {
    fn from(t: Track<(f32, f32)>) -> Self {
        Self::Pair(t)
    }
}

impl From<Track<Color>> for AnyTrack {
    fn from(t: Track<Color>) -> Self {
        Self::Color(t)
    }
}

/// Where a clip sits on the canvas and the pivot its transforms rotate and
/// scale about, as a bbox-relative point. Placement snaps to the nine zones
/// (a [`Custom`](Self::Custom) fraction rounds to the nearest zone edge for
/// placement while pivoting transforms at the exact point); fine positioning
/// belongs to translate tracks or the element's own layout.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Anchor {
    /// Canvas / bbox top-left.
    TopLeft,
    /// Top edge, horizontally centered.
    Top,
    /// Canvas / bbox top-right.
    TopRight,
    /// Left edge, vertically centered.
    Left,
    /// Dead center (the default).
    #[default]
    Center,
    /// Right edge, vertically centered.
    Right,
    /// Canvas / bbox bottom-left.
    BottomLeft,
    /// Bottom edge, horizontally centered.
    Bottom,
    /// Canvas / bbox bottom-right.
    BottomRight,
    /// An exact bbox-relative pivot `(fx, fy)` in `0.0..=1.0`.
    Custom(f32, f32),
}

impl Anchor {
    /// The anchor as bbox-relative fractions `(fx, fy)`.
    pub(crate) fn fractions(self) -> (f32, f32) {
        match self {
            Self::TopLeft => (0.0, 0.0),
            Self::Top => (0.5, 0.0),
            Self::TopRight => (1.0, 0.0),
            Self::Left => (0.0, 0.5),
            Self::Center => (0.5, 0.5),
            Self::Right => (1.0, 0.5),
            Self::BottomLeft => (0.0, 1.0),
            Self::Bottom => (0.5, 1.0),
            Self::BottomRight => (1.0, 1.0),
            Self::Custom(fx, fy) => (fx, fy),
        }
    }
}

/// The per-frame content builder: fenestra elements are single-use trees
/// rebuilt every frame (the framework's own rendering model), so content is
/// always a factory. `Send + Sync` keeps compositions shareable across a
/// parallel render.
pub(crate) type ContentFn = Box<dyn Fn(Frames) -> Element<()> + Send + Sync>;

/// One clip on the timeline. Content must be a pure function of the
/// clip-relative frame — the wall-clock animation surface (`.transition`,
/// `.keyframes`, `.spin`, `.enter`/`.exit`, `.animate_layout`) is FORBIDDEN
/// inside clip content: headless rendering pins it (reduced motion) and it
/// breaks frame purity. Animate through tracks or the frame argument.
pub struct Clip {
    pub(crate) id: String,
    pub(crate) span: FrameRange,
    pub(crate) z: i32,
    pub(crate) anchor: Anchor,
    pub(crate) content: ContentFn,
    pub(crate) tracks: Vec<(Prop, AnyTrack)>,
}

impl Clip {
    /// A clip with static content (set via [`element`](Self::element)),
    /// active over `span` (comp frames, half-open). Z-order is insertion
    /// order unless [`z`](Self::z) overrides it.
    pub fn new(id: impl Into<String>, span: impl Into<FrameRange>) -> Self {
        Self {
            id: id.into(),
            span: span.into(),
            z: 0,
            anchor: Anchor::default(),
            content: Box::new(|_| fenestra_core::div()),
            tracks: Vec::new(),
        }
    }

    /// The escape hatch: build any element tree from the clip-relative frame
    /// number (frame 0 = the clip's first active frame). Non-serializable —
    /// code-only; the data form can't carry closures.
    pub fn dynamic(
        id: impl Into<String>,
        span: impl Into<FrameRange>,
        f: impl Fn(Frames) -> Element<()> + Send + Sync + 'static,
    ) -> Self {
        let mut clip = Self::new(id, span);
        clip.content = Box::new(f);
        clip
    }

    /// The clip's content, rebuilt per frame (fenestra element trees are
    /// single-use by design). For frame-driven content use
    /// [`Clip::dynamic`].
    pub fn element(mut self, f: impl Fn() -> Element<()> + Send + Sync + 'static) -> Self {
        self.content = Box::new(move |_| f());
        self
    }

    /// Animates `prop` with a typed keyframe track (frames are
    /// clip-relative).
    ///
    /// # Panics
    /// If the track's value shape doesn't match the prop, or the prop is
    /// already animated on this clip.
    pub fn animate(mut self, prop: Prop, track: impl Into<AnyTrack>) -> Self {
        let track = track.into();
        let (want, got) = (prop.kind(), track.kind());
        assert!(
            want == got,
            "{prop:?} expects a {want:?} track, got a {got:?} track"
        );
        assert!(
            !self.tracks.iter().any(|(p, _)| *p == prop),
            "{prop:?} is already animated on clip {:?}",
            self.id
        );
        self.tracks.push((prop, track));
        self
    }

    /// Canvas placement and transform pivot (center by default).
    pub fn anchor(mut self, anchor: Anchor) -> Self {
        self.anchor = anchor;
        self
    }

    /// Z-order override: higher paints later (on top); ties keep insertion
    /// order.
    pub fn z(mut self, z: i32) -> Self {
        self.z = z;
        self
    }
}

/// A clip's props at one frame, defaults = identity (opacity 1, no
/// transform, inherit colors).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedProps {
    /// Subtree opacity.
    pub opacity: f32,
    /// Paint-time translation `(x, y)` in logical px.
    pub translate: (f32, f32),
    /// Uniform paint-time scale.
    pub scale: f32,
    /// Non-uniform paint-time scale `(x, y)`.
    pub scale_xy: (f32, f32),
    /// Paint-time rotation in degrees.
    pub rotate: f32,
    /// Root fill color, when animated.
    pub fill: Option<Color>,
    /// Root border color, when animated.
    pub stroke: Option<Color>,
    /// Root text color, when animated.
    pub text_color: Option<Color>,
}

impl Default for ResolvedProps {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            translate: (0.0, 0.0),
            scale: 1.0,
            scale_xy: (1.0, 1.0),
            rotate: 0.0,
            fill: None,
            stroke: None,
            text_color: None,
        }
    }
}

impl Clip {
    /// Samples every track at the clip-relative frame.
    pub(crate) fn resolve_props(&self, local: Frames, fps: u32) -> ResolvedProps {
        let mut out = ResolvedProps::default();
        for (prop, track) in &self.tracks {
            match (prop, track) {
                (Prop::Opacity, AnyTrack::Scalar(t)) => out.opacity = t.sample(local, fps),
                (Prop::TranslateX, AnyTrack::Scalar(t)) => {
                    out.translate.0 = t.sample(local, fps);
                }
                (Prop::TranslateY, AnyTrack::Scalar(t)) => {
                    out.translate.1 = t.sample(local, fps);
                }
                (Prop::Scale, AnyTrack::Scalar(t)) => out.scale = t.sample(local, fps),
                (Prop::Rotate, AnyTrack::Scalar(t)) => out.rotate = t.sample(local, fps),
                (Prop::ScaleXY, AnyTrack::Pair(t)) => out.scale_xy = t.sample(local, fps),
                (Prop::FillColor, AnyTrack::Color(t)) => out.fill = Some(t.sample(local, fps)),
                (Prop::StrokeColor, AnyTrack::Color(t)) => out.stroke = Some(t.sample(local, fps)),
                (Prop::TextColor, AnyTrack::Color(t)) => {
                    out.text_color = Some(t.sample(local, fps));
                }
                // `animate` enforces kind agreement at build time.
                _ => unreachable!("prop/track kind mismatch survived construction"),
            }
        }
        out
    }
}
