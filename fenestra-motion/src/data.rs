//! The data form: a versioned serde mirror of the composition IR
//! (`version: 1`). RON is the primary format — parsed and written with
//! `implicit_some` and unwrapped variant newtypes for humane authoring —
//! and JSON parses the identical shape. Clip content embeds the
//! fenestra-describe node vocabulary (`fenestra/1`), so a motion document
//! authors real fenestra UI, with colors as theme roles or `oklch` —
//! never raw hex. [`Clip::dynamic`](crate::Clip::dynamic) is code-only and
//! deliberately absent here: closures don't serialize.

use fenestra_core::{Color, Theme};
use fenestra_describe::color::resolve_color;
use fenestra_describe::format::{ColorSpec, Description, Node, SCHEMA_V1};
use fenestra_describe::parse::{to_element, to_element_lenient};
use serde::{Deserialize, Serialize};

use crate::clip::{Anchor, AnyTrack, Clip, Prop, PropKind};
use crate::composition::Composition;
use crate::easing::{
    EASE_CRISP, EASE_EDITORIAL, EASE_POP, Ease, Spring, ease_in, ease_in_out, ease_out,
};
use crate::timeline::{Key, Track, key};

/// The one schema version this build reads and writes.
const DOC_VERSION: u32 = 1;

/// A data-form failure: syntax, semantic problems (path-pointed), or an
/// attempt to serialize closures.
#[derive(Debug)]
pub enum DataError {
    /// The document didn't parse (RON or JSON syntax / shape).
    Parse(String),
    /// The document parsed but doesn't compile; each entry points at its
    /// path.
    Invalid(Vec<String>),
    /// Serialization was asked of a composition built in code:
    /// [`Clip::dynamic`](crate::Clip::dynamic) and element closures don't
    /// serialize. Author through the data form to serialize.
    NotSerializable,
}

impl std::fmt::Display for DataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(msg) => write!(f, "document did not parse: {msg}"),
            Self::Invalid(problems) => {
                writeln!(f, "document did not compile:")?;
                for p in problems {
                    writeln!(f, "  {p}")?;
                }
                Ok(())
            }
            Self::NotSerializable => write!(
                f,
                "this composition was built in code (element closures / Clip::dynamic don't \
                 serialize); only compositions loaded from the data form serialize back"
            ),
        }
    }
}

impl std::error::Error for DataError {}

/// A serialized composition: `version: 1`, canvas, clips, cuts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionDoc {
    /// Schema version; must be `1`.
    pub version: u32,
    /// Canvas width in logical px.
    pub width: u32,
    /// Canvas height in logical px.
    pub height: u32,
    /// Frames per second.
    pub fps: u32,
    /// Total frames; defaults to the furthest clip end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<u64>,
    /// Canvas base color: a theme role, `(oklch: (l, c, h))`, or the
    /// special `"transparent"` (also the default when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<ColorSpec>,
    /// Theme preset the content resolves its tokens against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemeDoc>,
    /// The clips, in paint (insertion) order.
    pub clips: Vec<ClipDoc>,
    /// Declared discontinuities for the temporal lints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cuts: Vec<u64>,
}

/// The theme presets a document may pick.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeDoc {
    /// [`Theme::light`].
    Light,
    /// [`Theme::dark`].
    Dark,
}

/// One serialized clip.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipDoc {
    /// Unique id (assertions and the CLI address clips by it).
    pub id: String,
    /// First active frame (comp time).
    pub start: u64,
    /// First frame past the span.
    pub end: u64,
    /// Z-order override; insertion order breaks ties.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub z: i32,
    /// Placement zone and transform pivot.
    #[serde(default, skip_serializing_if = "is_default_anchor")]
    pub anchor: AnchorDoc,
    /// The clip's content: a `fenestra/1` node.
    pub element: Node,
    /// Animation tracks (keyframe frames are clip-relative).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<TrackDoc>,
}

fn is_zero(z: &i32) -> bool {
    *z == 0
}

fn is_default_anchor(a: &AnchorDoc) -> bool {
    matches!(a, AnchorDoc::Center)
}

/// [`Anchor`] in document form.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AnchorDoc {
    /// Bbox top-left.
    TopLeft,
    /// Top edge, centered.
    Top,
    /// Bbox top-right.
    TopRight,
    /// Left edge, centered.
    Left,
    /// Dead center (the default).
    #[default]
    Center,
    /// Right edge, centered.
    Right,
    /// Bbox bottom-left.
    BottomLeft,
    /// Bottom edge, centered.
    Bottom,
    /// Bbox bottom-right.
    BottomRight,
    /// Exact fractions `(fx, fy)`.
    Custom(f32, f32),
}

impl From<AnchorDoc> for Anchor {
    fn from(a: AnchorDoc) -> Self {
        match a {
            AnchorDoc::TopLeft => Self::TopLeft,
            AnchorDoc::Top => Self::Top,
            AnchorDoc::TopRight => Self::TopRight,
            AnchorDoc::Left => Self::Left,
            AnchorDoc::Center => Self::Center,
            AnchorDoc::Right => Self::Right,
            AnchorDoc::BottomLeft => Self::BottomLeft,
            AnchorDoc::Bottom => Self::Bottom,
            AnchorDoc::BottomRight => Self::BottomRight,
            AnchorDoc::Custom(fx, fy) => Self::Custom(fx, fy),
        }
    }
}

/// One serialized track.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackDoc {
    /// The animated property.
    pub prop: PropDoc,
    /// Color interpolation space (color props only; Oklab default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space: Option<SpaceDoc>,
    /// The keyframes (clip-relative frames).
    pub keys: Vec<KeyDoc>,
}

/// [`Prop`] in document form.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropDoc {
    /// Subtree opacity.
    Opacity,
    /// Paint-time x translation, px.
    TranslateX,
    /// Paint-time y translation, px.
    TranslateY,
    /// Uniform scale about the anchor.
    Scale,
    /// Non-uniform scale `(x, y)` about the anchor.
    ScaleXy,
    /// Rotation in degrees about the anchor.
    Rotate,
    /// Root fill color.
    FillColor,
    /// Root border color.
    StrokeColor,
    /// Root text color.
    TextColor,
}

impl PropDoc {
    /// The name as the document grammar writes it (snake_case), for
    /// path-pointed errors that read like the author's own text.
    fn grammar_name(self) -> &'static str {
        match self {
            Self::Opacity => "opacity",
            Self::TranslateX => "translate_x",
            Self::TranslateY => "translate_y",
            Self::Scale => "scale",
            Self::ScaleXy => "scale_xy",
            Self::Rotate => "rotate",
            Self::FillColor => "fill_color",
            Self::StrokeColor => "stroke_color",
            Self::TextColor => "text_color",
        }
    }
}

impl From<PropDoc> for Prop {
    fn from(p: PropDoc) -> Self {
        match p {
            PropDoc::Opacity => Self::Opacity,
            PropDoc::TranslateX => Self::TranslateX,
            PropDoc::TranslateY => Self::TranslateY,
            PropDoc::Scale => Self::Scale,
            PropDoc::ScaleXy => Self::ScaleXY,
            PropDoc::Rotate => Self::Rotate,
            PropDoc::FillColor => Self::FillColor,
            PropDoc::StrokeColor => Self::StrokeColor,
            PropDoc::TextColor => Self::TextColor,
        }
    }
}

/// Color interpolation space in document form.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpaceDoc {
    /// Perceptual Oklab (the default).
    Oklab,
    /// Raw sRGB component lerp.
    Srgb,
}

/// One serialized keyframe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyDoc {
    /// Clip-relative frame.
    pub at: u64,
    /// The value (its shape must match the prop).
    pub value: ValueDoc,
    /// Easing into the next segment (linear when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ease: Option<EaseDoc>,
}

/// A keyframe value, explicitly tagged so documents stay self-describing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueDoc {
    /// An `f32` (opacity, translate axes, scale, rotate).
    Scalar(f32),
    /// An `(x, y)` pair (`scale_xy`).
    Pair(f32, f32),
    /// A color: theme role or `(oklch: (l, c, h))`.
    Color(ColorSpec),
}

impl ValueDoc {
    fn kind(&self) -> PropKind {
        match self {
            Self::Scalar(_) => PropKind::Scalar,
            Self::Pair(..) => PropKind::Pair,
            Self::Color(_) => PropKind::Color,
        }
    }
}

/// Easing in document form.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EaseDoc {
    /// Constant rate.
    Linear,
    /// Hold this key's value for the segment.
    Hold,
    /// CSS `ease-in`.
    EaseIn,
    /// CSS `ease-out`.
    EaseOut,
    /// CSS `ease-in-out`.
    EaseInOut,
    /// [`EASE_CRISP`].
    Crisp,
    /// [`EASE_EDITORIAL`].
    Editorial,
    /// [`EASE_POP`].
    Pop,
    /// A raw `cubic-bezier(x1, y1, x2, y2)`.
    Bezier(f32, f32, f32, f32),
    /// A closed-form damped spring.
    Spring {
        /// Stiffness (ω² scale).
        stiffness: f32,
        /// Damping; critical ≈ 2·√stiffness.
        damping: f32,
        /// Initial velocity, progress/second (0 when absent).
        #[serde(default)]
        velocity: f32,
    },
}

impl From<EaseDoc> for Ease {
    fn from(e: EaseDoc) -> Self {
        match e {
            EaseDoc::Linear => Self::Linear,
            EaseDoc::Hold => Self::Hold,
            EaseDoc::EaseIn => ease_in(),
            EaseDoc::EaseOut => ease_out(),
            EaseDoc::EaseInOut => ease_in_out(),
            EaseDoc::Crisp => EASE_CRISP,
            EaseDoc::Editorial => EASE_EDITORIAL,
            EaseDoc::Pop => EASE_POP,
            EaseDoc::Bezier(x1, y1, x2, y2) => {
                Self::Bezier(fenestra_core::CubicBezier { x1, y1, x2, y2 })
            }
            EaseDoc::Spring {
                stiffness,
                damping,
                velocity,
            } => Self::Spring(Spring {
                stiffness,
                damping,
                velocity,
            }),
        }
    }
}

/// The RON dialect motion documents speak: `implicit_some` (options read
/// as bare values) and unwrapped variant newtypes (`div(style: …)` instead
/// of `div((style: …))`).
fn ron_options() -> ron::Options {
    ron::Options::default().with_default_extension(
        ron::extensions::Extensions::IMPLICIT_SOME
            | ron::extensions::Extensions::UNWRAP_VARIANT_NEWTYPES,
    )
}

impl Composition {
    /// Parses and compiles a RON document (the primary data form).
    ///
    /// # Errors
    /// [`DataError::Parse`] on syntax, [`DataError::Invalid`] with
    /// path-pointed problems on a document that doesn't compile.
    pub fn from_ron(src: &str) -> Result<Self, DataError> {
        let doc: MotionDoc = ron_options()
            .from_str(src)
            .map_err(|e| DataError::Parse(e.to_string()))?;
        doc.compile()
    }

    /// Parses and compiles a JSON document (same shape as the RON form).
    ///
    /// # Errors
    /// As [`from_ron`](Self::from_ron).
    pub fn from_json(src: &str) -> Result<Self, DataError> {
        let doc: MotionDoc =
            serde_json::from_str(src).map_err(|e| DataError::Parse(e.to_string()))?;
        doc.compile()
    }

    /// Serializes the composition back to RON.
    ///
    /// # Errors
    /// [`DataError::NotSerializable`] for compositions built in code —
    /// closures don't serialize; only data-form documents round-trip.
    /// A serializer failure (practically unreachable for a compiled doc)
    /// reports as [`DataError::Parse`].
    pub fn to_ron(&self) -> Result<String, DataError> {
        let doc = self.source.as_ref().ok_or(DataError::NotSerializable)?;
        ron_options()
            .to_string_pretty(doc, ron::ser::PrettyConfig::default())
            .map_err(|e| DataError::Parse(e.to_string()))
    }

    /// Serializes the composition back to JSON.
    ///
    /// # Errors
    /// As [`to_ron`](Self::to_ron).
    pub fn to_json(&self) -> Result<String, DataError> {
        let doc = self.source.as_ref().ok_or(DataError::NotSerializable)?;
        serde_json::to_string_pretty(doc).map_err(|e| DataError::Parse(e.to_string()))
    }
}

impl MotionDoc {
    /// Compiles the document into a [`Composition`], validating everything
    /// the code-path builders assert: unique ids, prop/value agreement,
    /// key uniqueness, element vocabulary, and color resolution.
    ///
    /// # Errors
    /// [`DataError::Invalid`] with one path-pointed line per problem.
    pub fn compile(self) -> Result<Composition, DataError> {
        let mut problems = Vec::new();
        if self.version != DOC_VERSION {
            problems.push(format!(
                "version: this build reads version {DOC_VERSION}, got {}",
                self.version
            ));
        }
        // Zero dimensions are author errors here, not clamps: the code-path
        // builder forgives them, but a document saying `width: 0` means a
        // generator bug the author needs to hear about.
        for (name, value) in [
            ("width", self.width),
            ("height", self.height),
            ("fps", self.fps),
        ] {
            if value == 0 {
                problems.push(format!("{name}: must be at least 1"));
            }
        }
        let theme = match self.theme {
            Some(ThemeDoc::Dark) => Theme::dark(),
            Some(ThemeDoc::Light) | None => Theme::light(),
        };
        let background = match &self.background {
            None => Color::TRANSPARENT,
            Some(spec) => match resolve_motion_color(spec, &theme) {
                Ok(c) => c,
                Err(msg) => {
                    problems.push(format!("background: {msg}"));
                    Color::TRANSPARENT
                }
            },
        };

        let mut comp = Composition::new(self.width, self.height, self.fps)
            .background(background)
            .theme(theme.clone());
        if let Some(d) = self.duration {
            comp = comp.duration(d);
        }
        for cut in &self.cuts {
            comp = comp.cut(*cut);
        }

        let mut seen_ids: Vec<&str> = Vec::new();
        for (i, clip_doc) in self.clips.iter().enumerate() {
            let at = |suffix: &str| format!("clips[{i}] ({:?}){suffix}", clip_doc.id);
            if seen_ids.contains(&clip_doc.id.as_str()) {
                problems.push(at(": duplicate clip id"));
                continue;
            }
            seen_ids.push(&clip_doc.id);
            if clip_doc.end <= clip_doc.start {
                // Skip compiling the clip entirely: `start` is untrusted
                // u64, and building a span around it would overflow on
                // hostile values (start = u64::MAX panicked here once).
                problems.push(at(&format!(
                    ".span: empty ({}..{})",
                    clip_doc.start, clip_doc.end
                )));
                continue;
            }
            // Validate the element vocabulary once, strictly, then reuse the
            // SAME built Description for the per-frame factory below — it's
            // immutable across frames, so building it (and cloning the
            // content Node) here once, instead of every frame, roughly
            // halves the data-form path's per-frame tree cost.
            let desc = describe_doc(clip_doc.element.clone());
            if let Err(errs) = to_element(&desc, &theme) {
                for e in errs {
                    problems.push(at(&format!(".element: {e}")));
                }
            }
            match compile_clip(clip_doc, desc, &theme) {
                Ok(clip) => comp = comp.clip(clip),
                Err(mut errs) => {
                    problems.extend(errs.drain(..).map(|e| at(&e)));
                }
            }
        }

        if problems.is_empty() {
            comp.source = Some(Box::new(self));
            Ok(comp)
        } else {
            Err(DataError::Invalid(problems))
        }
    }
}

/// Wraps a content node in the one-node `fenestra/1` document the describe
/// parser speaks.
fn describe_doc(root: Node) -> Description {
    Description {
        schema: SCHEMA_V1.to_string(),
        root,
        theme: None,
        state: fenestra_describe::state::StateMap::default(),
    }
}

/// The motion color grammar: `"transparent"` (motion's own extension — a
/// document-authoring convenience, not part of describe's shared
/// [`ColorSpec`], so it works identically wherever a document names a
/// color: the background field AND any fill/stroke/text color track, so a
/// track can fade to nothing exactly like it fades between any two
/// theme-role colors), a theme role, or oklch.
fn resolve_motion_color(spec: &ColorSpec, theme: &Theme) -> Result<Color, String> {
    if let ColorSpec::Role(role) = spec
        && role == "transparent"
    {
        return Ok(Color::TRANSPARENT);
    }
    resolve_color(spec, theme).map_err(|e| e.to_string())
}

/// Compiles one clip: tracks validate against their props, keys against
/// track rules, and the content node becomes a rebuilt-per-frame factory.
fn compile_clip(doc: &ClipDoc, desc: Description, theme: &Theme) -> Result<Clip, Vec<String>> {
    let mut problems = Vec::new();
    // The caller has already rejected empty/inverted spans.
    let mut clip = Clip::new(&doc.id, doc.start..doc.end)
        .z(doc.z)
        .anchor(doc.anchor.into());

    // `desc` is immutable across frames (the caller already validated it
    // strictly); capture it once instead of re-cloning the content Node and
    // rebuilding a Description every frame.
    let content_theme = theme.clone();
    clip = clip.element(move || {
        // Validated strictly at compile; lenient here can no longer fail.
        let (el, _) = to_element_lenient(&desc, &content_theme);
        el.map(|_| ())
    });

    let mut seen: Vec<Prop> = Vec::new();
    for (t, track_doc) in doc.tracks.iter().enumerate() {
        let prop: Prop = track_doc.prop.into();
        let at = |msg: &str| format!(".tracks[{t}] ({}): {msg}", track_doc.prop.grammar_name());
        if seen.contains(&prop) {
            problems.push(at("prop is already animated on this clip"));
            continue;
        }
        seen.push(prop);
        match build_track(track_doc, prop, theme) {
            Ok(track) => clip = clip.animate(prop, track),
            Err(errs) => problems.extend(errs.into_iter().map(|e| at(&e))),
        }
    }

    if problems.is_empty() {
        Ok(clip)
    } else {
        Err(problems)
    }
}

/// Validates and builds one typed track from its document form.
fn build_track(doc: &TrackDoc, prop: Prop, theme: &Theme) -> Result<AnyTrack, Vec<String>> {
    let mut problems = Vec::new();
    if doc.keys.is_empty() {
        return Err(vec!["a track needs at least one key".into()]);
    }
    let mut frames: Vec<u64> = doc.keys.iter().map(|k| k.at).collect();
    frames.sort_unstable();
    for pair in frames.windows(2) {
        if pair[0] == pair[1] {
            problems.push(format!("duplicate key frame {}", pair[0]));
        }
    }

    let expects = prop.kind();
    for k in &doc.keys {
        if k.value.kind() != expects {
            problems.push(format!(
                "expects a {} value, got {} at frame {}",
                expects.name(),
                k.value.kind().name(),
                k.at
            ));
        }
        // A non-finite value would poison every sampled frame (NaN/inf
        // transforms), AND every temporal lint compares `delta > bound`,
        // which is false for NaN — so a NaN track would lint clean while
        // rendering garbage. Reject at the untrusted boundary.
        match &k.value {
            ValueDoc::Scalar(v) if !v.is_finite() => {
                problems.push(format!("frame {}: value must be finite", k.at));
            }
            ValueDoc::Pair(x, y) if !(x.is_finite() && y.is_finite()) => {
                problems.push(format!("frame {}: value must be finite", k.at));
            }
            _ => {}
        }
        // Non-finite easing parameters would poison every sampled value
        // (NaN transforms); reject them at the untrusted boundary.
        match k.ease {
            Some(EaseDoc::Spring {
                stiffness,
                damping,
                velocity,
            }) if !(stiffness.is_finite() && damping.is_finite() && velocity.is_finite()) => {
                problems.push(format!("frame {}: spring parameters must be finite", k.at));
            }
            Some(EaseDoc::Bezier(x1, y1, x2, y2))
                if !(x1.is_finite() && y1.is_finite() && x2.is_finite() && y2.is_finite()) =>
            {
                problems.push(format!(
                    "frame {}: bezier control points must be finite",
                    k.at
                ));
            }
            _ => {}
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }

    let ease_of = |k: &KeyDoc| k.ease.map_or(Ease::Linear, Into::into);
    let track = match expects {
        PropKind::Pair => {
            let keys = doc.keys.iter().map(|k| {
                let ValueDoc::Pair(x, y) = k.value else {
                    unreachable!("kind checked above")
                };
                key(k.at, (x, y)).ease(ease_of(k))
            });
            AnyTrack::Pair(Track::new(keys))
        }
        PropKind::Color => {
            let mut keys: Vec<Key<Color>> = Vec::new();
            for k in &doc.keys {
                let ValueDoc::Color(spec) = &k.value else {
                    unreachable!("kind checked above")
                };
                match resolve_motion_color(spec, theme) {
                    Ok(c) => keys.push(key(k.at, c).ease(ease_of(k))),
                    Err(e) => problems.push(format!("frame {}: {e}", k.at)),
                }
            }
            if !problems.is_empty() {
                return Err(problems);
            }
            let track = Track::new(keys);
            match doc.space {
                Some(SpaceDoc::Srgb) => AnyTrack::Color(track.srgb()),
                _ => AnyTrack::Color(track),
            }
        }
        PropKind::Scalar => {
            let keys = doc.keys.iter().map(|k| {
                let ValueDoc::Scalar(v) = k.value else {
                    unreachable!("kind checked above")
                };
                key(k.at, v).ease(ease_of(k))
            });
            AnyTrack::Scalar(Track::new(keys))
        }
    };
    Ok(track)
}
