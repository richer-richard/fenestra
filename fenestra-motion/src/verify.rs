//! The verification layer — the point of the crate. Structural assertions
//! and temporal lints run over [`Composition::sample`] with no pixels
//! involved; sentinel frames pick the interesting instants for golden
//! coverage; the contact sheet tiles a whole timeline into one labeled
//! image an agent can review in a single look.

use fenestra_core::{Color, TextSize, image_rgba8, text};
use rayon::prelude::*;
use serde::Serialize;

use crate::clip::{Prop, ResolvedProps};
use crate::composition::Composition;
use crate::render::MotionError;
use crate::timeline::Frames;

/// One temporal-lint finding, pointed at a clip, prop, and frame.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LintProblem {
    /// The clip the problem lives on.
    pub clip: String,
    /// The offending prop, when the problem is prop-shaped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prop: Option<String>,
    /// The first frame exhibiting the problem.
    pub frame: Frames,
    /// What went wrong, in one sentence.
    pub message: String,
}

impl Frames {
    fn range_to(self, end: Frames) -> impl Iterator<Item = Frames> {
        (self.0..end.0).map(Frames)
    }
}

// Serialize Frames as its number for lint reports.
impl Serialize for Frames {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(self.0)
    }
}

/// The default per-prop jump threshold for [`discontinuities`]: generous
/// enough that fast (but eased) motion passes — a crisp entrance curve
/// (control y = 1) legitimately covers ~35% of its distance in the first
/// frame — tight enough that a teleport fails. Translate thresholds scale
/// with the canvas.
fn default_eps(prop: Prop, comp: &Composition) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "canvas sizes are far below f32's integer range"
    )]
    match prop {
        Prop::Opacity => 0.5,
        Prop::Scale | Prop::ScaleXY => 0.5,
        Prop::Rotate => 60.0,
        Prop::TranslateX => comp.width as f32 * 0.25,
        Prop::TranslateY => comp.height as f32 * 0.25,
        // Colors compare on max channel delta.
        Prop::FillColor | Prop::StrokeColor | Prop::TextColor => 0.5,
    }
}

/// The largest single-frame delta a prop shows between two resolved states.
fn prop_delta(prop: Prop, a: &ResolvedProps, b: &ResolvedProps) -> f32 {
    let color_delta = |x: Option<Color>, y: Option<Color>| match (x, y) {
        (Some(x), Some(y)) => x
            .components
            .iter()
            .zip(y.components)
            .map(|(cx, cy)| (cx - cy).abs())
            .fold(0.0f32, f32::max),
        _ => 0.0,
    };
    match prop {
        Prop::Opacity => (a.opacity - b.opacity).abs(),
        Prop::TranslateX => (a.translate.0 - b.translate.0).abs(),
        Prop::TranslateY => (a.translate.1 - b.translate.1).abs(),
        Prop::Scale => (a.scale - b.scale).abs(),
        Prop::ScaleXY => (a.scale_xy.0 - b.scale_xy.0)
            .abs()
            .max((a.scale_xy.1 - b.scale_xy.1).abs()),
        Prop::Rotate => (a.rotate - b.rotate).abs(),
        Prop::FillColor => color_delta(a.fill, b.fill),
        Prop::StrokeColor => color_delta(a.stroke, b.stroke),
        Prop::TextColor => color_delta(a.text_color, b.text_color),
    }
}

/// The scalar view of a prop, for [`monotone`] / [`settled`].
fn prop_scalar(prop: Prop, p: &ResolvedProps) -> Option<f32> {
    match prop {
        Prop::Opacity => Some(p.opacity),
        Prop::TranslateX => Some(p.translate.0),
        Prop::TranslateY => Some(p.translate.1),
        Prop::Scale => Some(p.scale),
        Prop::Rotate => Some(p.rotate),
        Prop::ScaleXY | Prop::FillColor | Prop::StrokeColor | Prop::TextColor => None,
    }
}

/// Finds undeclared jumps: any animated prop whose value moves more than
/// its threshold (`eps` overrides every per-prop default) between adjacent
/// in-span frames, except across a declared [`Composition::cut`]. Span
/// edges are entrances/exits, not glitches, and are always allowed.
#[must_use]
pub fn discontinuities(comp: &Composition, eps: Option<f32>) -> Vec<LintProblem> {
    let mut problems = Vec::new();
    let duration = comp.total_frames();
    for clip in &comp.clips {
        let last = clip.span.end.min(duration);
        for frame in clip.span.start.range_to(last) {
            let next = Frames(frame.0 + 1);
            if !clip.span.contains(next) || comp.cuts.contains(&next) {
                continue;
            }
            let a = clip.resolve_props(Frames(frame.0 - clip.span.start.0), comp.fps);
            let b = clip.resolve_props(Frames(next.0 - clip.span.start.0), comp.fps);
            for (prop, _) in &clip.tracks {
                let bound = eps.unwrap_or_else(|| default_eps(*prop, comp));
                let delta = prop_delta(*prop, &a, &b);
                if delta > bound {
                    problems.push(LintProblem {
                        clip: clip.id.clone(),
                        prop: Some(format!("{prop:?}")),
                        frame: next,
                        message: format!(
                            "jumps by {delta:.3} between frames {frame} and {next} \
                             (threshold {bound:.3}); declare a .cut({next}) if intended"
                        ),
                    });
                }
            }
        }
    }
    problems
}

/// The direction [`monotone`] verifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Values never decrease across the range.
    Increasing,
    /// Values never increase across the range.
    Decreasing,
}

/// Verifies a scalar prop moves in one direction over `range` (comp
/// frames). Color and pair props have no single order and report as
/// unsupported.
#[must_use]
pub fn monotone(
    comp: &Composition,
    clip_id: &str,
    prop: Prop,
    range: std::ops::Range<u64>,
    direction: Direction,
) -> Vec<LintProblem> {
    let Some(clip) = comp.clips.iter().find(|c| c.id == clip_id) else {
        return vec![LintProblem {
            clip: clip_id.to_string(),
            prop: Some(format!("{prop:?}")),
            frame: Frames(range.start),
            message: "no clip with this id".into(),
        }];
    };
    let mut problems = Vec::new();
    let mut prev: Option<f32> = None;
    for frame in range {
        // Clamp into the span like every other sampler (sample, settled):
        // the renderer freezes at the last in-span frame past span.end, so
        // reading raw track values beyond it would check motion nobody
        // ever sees.
        let local = local_of(clip, Frames(frame));
        let props = clip.resolve_props(local, comp.fps);
        let Some(value) = prop_scalar(prop, &props) else {
            return vec![LintProblem {
                clip: clip_id.to_string(),
                prop: Some(format!("{prop:?}")),
                frame: Frames(frame),
                message: "monotone only orders scalar props".into(),
            }];
        };
        if let Some(prev) = prev {
            let broken = match direction {
                Direction::Increasing => value < prev - 1e-6,
                Direction::Decreasing => value > prev + 1e-6,
            };
            if broken {
                problems.push(LintProblem {
                    clip: clip_id.to_string(),
                    prop: Some(format!("{prop:?}")),
                    frame: Frames(frame),
                    message: format!(
                        "moves from {prev:.4} to {value:.4} against the declared \
                         {direction:?} direction"
                    ),
                });
            }
        }
        prev = Some(value);
    }
    problems
}

/// Verifies nothing changes after `after`: every animated prop holds its
/// value, and no clip appears or disappears, from `after` to the end of the
/// timeline.
#[must_use]
pub fn settled(comp: &Composition, after: Frames) -> Vec<LintProblem> {
    let mut problems = Vec::new();
    let duration = comp.total_frames();
    for clip in &comp.clips {
        // A span edge inside (after, duration) is a visibility change.
        for edge in [clip.span.start, clip.span.end] {
            if edge > after && edge < duration {
                problems.push(LintProblem {
                    clip: clip.id.clone(),
                    prop: None,
                    frame: edge,
                    message: format!("appears or disappears at frame {edge}, after {after}"),
                });
            }
        }
        for frame in after.range_to(duration) {
            let next = Frames(frame.0 + 1);
            if next >= duration {
                break;
            }
            let a = clip.resolve_props(local_of(clip, frame), comp.fps);
            let b = clip.resolve_props(local_of(clip, next), comp.fps);
            for (prop, _) in &clip.tracks {
                if prop_delta(*prop, &a, &b) > 1e-6 {
                    problems.push(LintProblem {
                        clip: clip.id.clone(),
                        prop: Some(format!("{prop:?}")),
                        frame: next,
                        message: format!("still moving at frame {next}, after {after}"),
                    });
                    break;
                }
            }
        }
    }
    problems
}

fn local_of(clip: &crate::clip::Clip, frame: Frames) -> Frames {
    Frames(
        frame
            .0
            .saturating_sub(clip.span.start.0)
            .min(clip.span.len().0.saturating_sub(1)),
    )
}

impl Composition {
    /// Auto-selects the frames worth pinning as goldens: every clip's span
    /// edges, every keyframe (comp-absolute), and every segment midpoint —
    /// deduped, sorted, clamped inside the timeline.
    #[must_use]
    pub fn sentinel_frames(&self) -> Vec<Frames> {
        let duration = self.total_frames();
        let mut out: Vec<Frames> = Vec::new();
        for clip in &self.clips {
            out.push(clip.span.start);
            out.push(clip.span.end);
            for (_, track) in &clip.tracks {
                let keys: Vec<u64> = track_key_frames(track)
                    .iter()
                    .map(|k| clip.span.start.0.saturating_add(k.0))
                    .collect();
                for pair in keys.windows(2) {
                    out.push(Frames(u64::midpoint(pair[0], pair[1])));
                }
                out.extend(keys.into_iter().map(Frames));
            }
        }
        out.retain(|f| f.0 < duration.0);
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Tiles every `every`-th frame into one labeled grid image — frame
    /// numbers burned in under each thumbnail — sized so an agent reviews a
    /// whole timeline in a single look. Thumbnails render at
    /// `thumb_width / width` scale (no resampling pass).
    ///
    /// # Errors
    /// When the grid would exceed the GPU texture ceiling (raise `every` or
    /// shrink `thumb_width`), or on a render failure.
    pub fn contact_sheet(
        &self,
        every: u64,
        thumb_width: u32,
    ) -> Result<image::RgbaImage, MotionError> {
        const MAX_DIM: u64 = 8192;
        const GAP: u64 = 8;
        const LABEL_H: u64 = 20;

        let frames: Vec<u64> = (0..self.total_frames().0)
            .step_by(usize::try_from(every.max(1)).unwrap_or(1))
            .collect();
        let count = frames.len() as u64;
        if count == 0 {
            return Err(MotionError::Sheet("the timeline has no frames".into()));
        }
        // A zero/tiny thumb width would render full-resolution thumbnails
        // into a degenerate grid; 16px is the floor of legibility.
        let thumb_width = thumb_width.max(16);
        let scale = f64::from(thumb_width) / f64::from(self.width);
        let tw = u64::from(thumb_width);
        let th = (f64::from(self.height) * scale).round() as u64;

        let cols = (count as f64).sqrt().ceil() as u64;
        let rows = count.div_ceil(cols);
        let sheet_w = cols * tw + (cols + 1) * GAP;
        let sheet_h = rows * (th + LABEL_H) + (rows + 1) * GAP;
        if sheet_w > MAX_DIM || sheet_h > MAX_DIM {
            return Err(MotionError::Sheet(format!(
                "{count} thumbnails at {tw}px would make a {sheet_w}×{sheet_h} sheet \
                 (max {MAX_DIM}); raise --every or lower the thumb width"
            )));
        }

        // Thumbnails in parallel (the GPU serializes; layout doesn't).
        let mut thumbs: Vec<(u64, image::RgbaImage)> = frames
            .par_iter()
            .map(|&f| Ok((f, self.render_frame_at(Frames(f), scale)?)))
            .collect::<Result<_, MotionError>>()?;
        thumbs.sort_by_key(|(f, _)| *f);

        #[expect(
            clippy::cast_precision_loss,
            reason = "sheet dimensions are bounded by MAX_DIM"
        )]
        let mut grid = fenestra_core::col::<()>()
            .w(sheet_w as f32)
            .h(sheet_h as f32)
            .p(GAP as f32)
            .gap(GAP as f32);
        for row in thumbs.chunks(usize::try_from(cols).unwrap_or(1)) {
            #[expect(
                clippy::cast_precision_loss,
                reason = "sheet dimensions are bounded by MAX_DIM"
            )]
            let mut r = fenestra_core::row::<()>().gap(GAP as f32);
            for (frame, img) in row {
                let (w, h) = img.dimensions();
                // A hairline frame keeps transparent/empty thumbnails
                // visible against the sheet background.
                let framed = fenestra_core::div::<()>()
                    .child(image_rgba8(w, h, img.as_raw().clone()))
                    .themed(|t, s| s.border(1.0, t.border_subtle));
                r = r.child(
                    fenestra_core::col().gap(2.0).child(framed).child(
                        text(format!("f {frame:05}"))
                            .size(TextSize::Xs)
                            .mono()
                            .themed(|t, s| s.color(t.text_muted)),
                    ),
                );
            }
            grid = grid.child(r);
        }

        let theme = self.theme.clone();
        crate::render::render_sheet(
            grid,
            &theme,
            (
                u32::try_from(sheet_w).expect("bounded by MAX_DIM"),
                u32::try_from(sheet_h).expect("bounded by MAX_DIM"),
            ),
        )
    }
}

/// The key frames of a typed track (clip-relative).
fn track_key_frames(track: &crate::clip::AnyTrack) -> Vec<Frames> {
    match track {
        crate::clip::AnyTrack::Scalar(t) => t.keys.iter().map(|k| k.at).collect(),
        crate::clip::AnyTrack::Pair(t) => t.keys.iter().map(|k| k.at).collect(),
        crate::clip::AnyTrack::Color(t) => t.keys.iter().map(|k| k.at).collect(),
    }
}
