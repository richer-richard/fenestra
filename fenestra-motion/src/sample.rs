//! A sampled scene: one frame's resolved clips, the element tree they
//! assemble into, and windowless bbox measurement through the same layout
//! and transform math the renderer paints with.

use std::cell::OnceCell;

use fenestra_anim::Frames;
use fenestra_core::{Element, Fonts, Frame, FrameState, Style, build_frame, by, div, stack};

use crate::clip::ResolvedProps;
use crate::composition::Composition;

/// One clip's state at the sampled frame.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedClip {
    /// Track values at this frame (identity defaults where unanimated).
    /// Outside the span they sample at the clamped local frame.
    pub props: ResolvedProps,
    /// The post-transform axis-aligned bounding box of the clip's painted
    /// content, in canvas coordinates; `None` while invisible.
    pub bbox: Option<kurbo::Rect>,
    /// Whether the sampled frame lies inside the clip's span.
    pub visible: bool,
}

struct Entry {
    /// Index into `comp.clips`.
    index: usize,
    /// Clip-relative frame, clamped into the span.
    local: Frames,
    visible: bool,
    props: ResolvedProps,
}

/// A composition sampled at one frame — resolved props per clip, structural
/// probes, and the element tree the renderer rasterizes. Everything here is
/// windowless; only rasterization needs a GPU.
pub struct SampledScene<'a> {
    comp: &'a Composition,
    frame: Frames,
    /// All clips in paint order (z, then insertion).
    entries: Vec<Entry>,
    /// The laid-out frame, built on first bbox query (embedded fonts,
    /// reduced motion — the deterministic golden setup).
    layout: OnceCell<Frame>,
}

impl<'a> SampledScene<'a> {
    pub(crate) fn new(comp: &'a Composition, frame: Frames) -> Self {
        let mut order: Vec<usize> = (0..comp.clips.len()).collect();
        order.sort_by_key(|&i| (comp.clips[i].z, i));
        let entries = order
            .into_iter()
            .map(|index| {
                let clip = &comp.clips[index];
                let span = clip.span;
                let local = Frames(
                    frame
                        .0
                        .saturating_sub(span.start.0)
                        .min(span.len().0.saturating_sub(1)),
                );
                let props = clip.resolve_props(local, comp.fps);
                Entry {
                    index,
                    local,
                    visible: span.contains(frame),
                    props,
                }
            })
            .collect();
        Self {
            comp,
            frame,
            entries,
            layout: OnceCell::new(),
        }
    }

    /// The sampled frame number.
    pub fn frame(&self) -> Frames {
        self.frame
    }

    /// Visible clip ids in paint order (bottom first). The ids borrow the
    /// composition, not this scene, so they outlive a probe chain.
    #[must_use]
    pub fn paint_order(&self) -> Vec<&'a str> {
        self.entries
            .iter()
            .filter(|e| e.visible)
            .map(|e| self.comp.clips[e.index].id.as_str())
            .collect()
    }

    /// One clip's resolved props, bbox, and visibility; `None` for an
    /// unknown id.
    #[must_use]
    pub fn resolve(&self, id: &str) -> Option<ResolvedClip> {
        let entry = self
            .entries
            .iter()
            .find(|e| self.comp.clips[e.index].id == id)?;
        let bbox = entry.visible.then(|| self.measure(entry)).flatten();
        Some(ResolvedClip {
            props: entry.props,
            bbox,
            visible: entry.visible,
        })
    }

    /// This frame's element tree: a full-canvas z-stack, one anchored
    /// wrapper per visible clip carrying the resolved style props. The
    /// renderer rasterizes exactly this.
    #[must_use]
    pub fn element(&self) -> Element<()> {
        #[expect(
            clippy::cast_precision_loss,
            reason = "canvas sizes are far below f32's integer range"
        )]
        let (w, h) = (self.comp.width as f32, self.comp.height as f32);
        let mut root = stack().w(w).h(h);
        if self.comp.background.components[3] > 0.0 {
            root = root.bg(self.comp.background);
        }
        for entry in self.entries.iter().filter(|e| e.visible) {
            let clip = &self.comp.clips[entry.index];
            let content = (clip.content)(entry.local);
            let p = entry.props;
            // Color tracks overlay the clip's ROOT element (styles don't
            // cascade in fenestra; deeper nodes belong to the closure).
            let content = if p.fill.is_some() || p.stroke.is_some() || p.text_color.is_some() {
                content.themed(move |_, mut s| {
                    if let Some(c) = p.fill {
                        s = s.bg(c);
                    }
                    if let Some(c) = p.stroke
                        && let Some(b) = s.border
                    {
                        s.border = Some(fenestra_core::Border { color: c, ..b });
                    }
                    if let Some(c) = p.text_color {
                        s = s.color(c);
                    }
                    s
                })
            } else {
                content
            };
            let (fx, fy) = clip.anchor.fractions();
            // The transform wrapper shrink-wraps the content: its rect is
            // the clip bbox, so anchors pivot exactly where probes report.
            // Uniform scale folds into scale_xy (S·Sxy = diag(s·sx, s·sy));
            // `measure` composes the identical matrix.
            let inner = div()
                .id(&clip.id)
                .child(content)
                .opacity(p.opacity)
                .translate(p.translate.0, p.translate.1)
                .rotate(p.rotate)
                .scale_xy(p.scale * p.scale_xy.0, p.scale * p.scale_xy.1)
                .transform_origin(fx, fy);
            // Placement: an explicitly full-canvas flex wrapper aligns the
            // clip into the anchor's zone (stack cells overlap; the size is
            // pinned rather than trusting grid stretch defaults).
            let outer = align_zone(div().w_full().h_full(), fx, fy).child(inner);
            root = root.child(outer);
        }
        root
    }

    /// Lays the scene out once (embedded fonts, scale 1.0, reduced motion)
    /// and measures a clip wrapper's post-transform AABB through the same
    /// `Style::paint_affine` the painter and hit-tester use.
    fn measure(&self, entry: &Entry) -> Option<kurbo::Rect> {
        let frame = self.layout.get_or_init(|| {
            let el = self.element();
            let mut fonts = Fonts::embedded();
            let mut state = FrameState::new();
            state.reduced_motion = true;
            #[expect(
                clippy::cast_precision_loss,
                reason = "canvas sizes are far below f32's integer range"
            )]
            let size = (self.comp.width as f32, self.comp.height as f32);
            build_frame(&el, &self.comp.theme, &mut fonts, &mut state, size, 1.0)
        });
        let clip = &self.comp.clips[entry.index];
        let node = frame.try_get(&by::id(&clip.id)).ok()?;
        let rect = frame.rect_of(node.id)?;

        let p = entry.props;
        let (fx, fy) = clip.anchor.fractions();
        let style = Style::default()
            .translate(p.translate.0, p.translate.1)
            .rotate(p.rotate)
            .scale_xy(p.scale * p.scale_xy.0, p.scale * p.scale_xy.1)
            .transform_origin(fx, fy);
        match style.paint_affine(rect) {
            None => Some(rect),
            Some(a) => Some(a.transform_rect_bbox(rect)),
        }
    }
}

/// Flex alignment for an anchor zone: fractions snap to start / center / end
/// per axis (thresholds at 0.25 / 0.75).
fn align_zone(el: Element<()>, fx: f32, fy: f32) -> Element<()> {
    let el = if fx < 0.25 {
        el.justify_start()
    } else if fx > 0.75 {
        el.justify_end()
    } else {
        el.justify_center()
    };
    if fy < 0.25 {
        el.items_start()
    } else if fy > 0.75 {
        el.items_end()
    } else {
        el.items_center()
    }
}
