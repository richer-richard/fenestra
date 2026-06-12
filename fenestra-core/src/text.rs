//! Text: the embedded Inter family, fontique registration with system
//! fallback, the parley layout cache, max-lines ellipsis truncation, and
//! glyph painting into vello scenes.

use std::collections::HashMap;

use fontique::{Blob, CollectionOptions, GenericFamily};
use kurbo::{Affine, Rect};
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, FontWeight, Layout, LayoutContext,
    LineHeight, PositionedLayoutItem, StyleProperty,
};
use peniko::{Color, Fill};
use vello::Scene;

use crate::style::{TextAlign, TextStyle};
use crate::theme::Theme;
use crate::tokens::FamilyRole;

const INTER_REGULAR: &[u8] = include_bytes!("../assets/inter/Inter-Regular.otf");
const INTER_MEDIUM: &[u8] = include_bytes!("../assets/inter/Inter-Medium.otf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/inter/Inter-SemiBold.otf");

/// Preferred mono families, matched against installed system fonts in order.
/// The `monospace` generic remains the final fallback.
const MONO_FAMILIES: &[&str] = &["SF Mono", "Cascadia Code", "JetBrains Mono"];

/// Parley brush type. Colors are applied at draw time via `DrawGlyphs`, so
/// layouts are color-independent and cache across recolors.
pub(crate) type LayoutBrush = [u8; 4];

/// A text style with every token resolved to a concrete value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedText {
    pub px: f32,
    pub weight: f32,
    /// Line height as a multiple of the font size.
    pub line_height: f32,
    /// Letter spacing in logical px (em value already multiplied out).
    pub letter_spacing: f32,
    pub family: FamilyRole,
    pub align: TextAlign,
    pub max_lines: Option<u32>,
    pub color: Color,
}

/// Resolves the text style group against the theme and size tokens.
pub(crate) fn resolve_text(ts: &TextStyle, theme: &Theme) -> ResolvedText {
    let px = ts.size_px.unwrap_or_else(|| ts.size.px());
    ResolvedText {
        px,
        weight: ts.weight.value(),
        line_height: ts.line_height.unwrap_or_else(|| {
            // Free-form sizes default to a tight display leading; tokens
            // keep their scale's value.
            if ts.size_px.is_some() {
                1.25
            } else {
                ts.size.line_height()
            }
        }),
        letter_spacing: ts
            .letter_spacing
            .unwrap_or_else(|| ts.size.letter_spacing())
            * px,
        family: ts.family,
        align: ts.align,
        max_lines: ts.max_lines,
        color: ts.color.unwrap_or(theme.text),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct LayoutKey {
    text: String,
    px: u32,
    weight: u32,
    line_height: u32,
    letter_spacing: u32,
    family: FamilyRole,
    align: TextAlign,
    max_lines: Option<u32>,
    /// Quantized max advance (quarter-px buckets); `u32::MAX` = unbounded.
    width_bucket: u32,
}

impl LayoutKey {
    fn new(text: &str, style: &ResolvedText, max_advance: Option<f32>) -> Self {
        Self {
            text: text.to_owned(),
            px: style.px.to_bits(),
            weight: style.weight.to_bits(),
            line_height: style.line_height.to_bits(),
            letter_spacing: style.letter_spacing.to_bits(),
            family: style.family,
            align: style.align,
            max_lines: style.max_lines,
            #[expect(clippy::cast_sign_loss, reason = "advances are non-negative")]
            width_bucket: match max_advance {
                Some(w) => (w.max(0.0) * 4.0).round() as u32,
                None => u32::MAX,
            },
        }
    }
}

/// The font system: a fontique collection with the embedded Inter family,
/// parley contexts, and the layout cache. One per app (or per test process).
pub struct Fonts {
    font_cx: FontContext,
    layout_cx: LayoutContext<LayoutBrush>,
    cache: HashMap<LayoutKey, Layout<LayoutBrush>>,
    /// Family names registered per role (Display/Serif design faces).
    roles: HashMap<FamilyRole, String>,
}

impl Fonts {
    /// Embedded fonts only: fully deterministic, used by headless rendering.
    pub fn embedded() -> Self {
        Self::new(false)
    }

    /// Embedded fonts plus discovered system fonts for fallback glyphs and
    /// the mono family role. Used by the windowed runner.
    pub fn with_system() -> Self {
        Self::new(true)
    }

    fn new(system_fonts: bool) -> Self {
        let mut font_cx = FontContext {
            collection: fontique::Collection::new(CollectionOptions {
                shared: false,
                system_fonts,
            }),
            source_cache: fontique::SourceCache::default(),
        };
        let collection = &mut font_cx.collection;
        let mut inter_ids = Vec::new();
        for bytes in [INTER_REGULAR, INTER_MEDIUM, INTER_SEMIBOLD] {
            let data = Blob::new(std::sync::Arc::new(bytes));
            for (family, _fonts) in collection.register_fonts(data, None) {
                if !inter_ids.contains(&family) {
                    inter_ids.push(family);
                }
            }
        }
        // Inter heads the sans-serif generic so unspecified text uses it.
        let mut sans = inter_ids.clone();
        sans.extend(collection.generic_families(GenericFamily::SansSerif));
        collection.set_generic_families(GenericFamily::SansSerif, sans.into_iter());
        // Preferred mono families head the monospace generic when installed;
        // Inter is the last resort so mono text never disappears (embedded
        // collections have no system monospace mapping at all).
        let mut mono: Vec<_> = MONO_FAMILIES
            .iter()
            .filter_map(|name| collection.family_id(name))
            .collect();
        mono.extend(collection.generic_families(GenericFamily::Monospace));
        mono.extend(inter_ids.iter().copied());
        collection.set_generic_families(GenericFamily::Monospace, mono.into_iter());

        Self {
            font_cx,
            layout_cx: LayoutContext::new(),
            cache: HashMap::new(),
            roles: HashMap::new(),
        }
    }

    /// Registers font data (TTF/OTF, collections too) under a family role,
    /// so text styled `.family(FamilyRole::Display)` (or `Serif`) resolves
    /// to it. The layout cache is cleared. Returns `false` when no face
    /// could be parsed from `data`.
    pub fn register(&mut self, role: FamilyRole, data: Vec<u8>) -> bool {
        let collection = &mut self.font_cx.collection;
        let mut name = None;
        for (family, _fonts) in
            collection.register_fonts(Blob::new(std::sync::Arc::new(data)), None)
        {
            if name.is_none() {
                name = collection.family_name(family).map(str::to_owned);
            }
        }
        let Some(name) = name else {
            return false;
        };
        self.roles.insert(role, name);
        self.cache.clear();
        true
    }

    /// The font and layout contexts, for parley editor drivers.
    pub(crate) fn editor_contexts(
        &mut self,
    ) -> (&mut FontContext, &mut LayoutContext<LayoutBrush>) {
        (&mut self.font_cx, &mut self.layout_cx)
    }

    /// Lays out `text`, wrapped at `max_advance` logical px (`None` =
    /// unbounded), through the cache.
    pub(crate) fn layout(
        &mut self,
        text: &str,
        style: &ResolvedText,
        max_advance: Option<f32>,
    ) -> &Layout<LayoutBrush> {
        let key = LayoutKey::new(text, style, max_advance);
        if !self.cache.contains_key(&key) {
            let layout = self.build_layout(text, style, max_advance);
            self.cache.insert(key.clone(), layout);
        }
        &self.cache[&key]
    }

    fn build_layout(
        &mut self,
        text: &str,
        style: &ResolvedText,
        max_advance: Option<f32>,
    ) -> Layout<LayoutBrush> {
        let mut layout = self.shape(text, style, max_advance);
        if let (Some(max_lines), Some(_)) = (style.max_lines, max_advance) {
            let max_lines = max_lines.max(1) as usize;
            if layout.lines().count() > max_lines {
                layout = self.truncate(text, style, max_advance, max_lines);
            }
        }
        layout
    }

    fn shape(
        &mut self,
        text: &str,
        style: &ResolvedText,
        max_advance: Option<f32>,
    ) -> Layout<LayoutBrush> {
        // parley's line breaker overflows (and asserts) on enormous or
        // non-finite advances; clamp at the boundary.
        let max_advance = max_advance.and_then(|w| w.is_finite().then(|| w.clamp(0.0, 1.0e9)));
        let family = match style.family {
            FamilyRole::Sans => FontFamily::named("Inter"),
            FamilyRole::Mono => FontFamily::Single(GenericFamily::Monospace.into()),
            FamilyRole::Display | FamilyRole::Serif => match self.roles.get(&style.family) {
                Some(name) => FontFamily::named(name),
                None => FontFamily::named("Inter"),
            },
        };
        let mut builder = self
            .layout_cx
            .ranged_builder(&mut self.font_cx, text, 1.0, true);
        builder.push_default(StyleProperty::FontFamily(family));
        builder.push_default(StyleProperty::FontSize(style.px));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(style.weight)));
        builder.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(
            style.line_height,
        )));
        builder.push_default(StyleProperty::LetterSpacing(style.letter_spacing));
        let mut layout = builder.build(text);
        layout.break_all_lines(max_advance);
        let alignment = match style.align {
            TextAlign::Start => Alignment::Start,
            TextAlign::Center => Alignment::Center,
            TextAlign::End => Alignment::End,
        };
        layout.align(alignment, AlignmentOptions::default());
        layout
    }

    /// Finds the longest prefix whose layout (with an appended ellipsis)
    /// fits in `max_lines`, by binary search over char boundaries.
    fn truncate(
        &mut self,
        text: &str,
        style: &ResolvedText,
        max_advance: Option<f32>,
        max_lines: usize,
    ) -> Layout<LayoutBrush> {
        let boundaries: Vec<usize> = text
            .char_indices()
            .map(|(i, _)| i)
            .chain([text.len()])
            .collect();
        let fits = |fonts: &mut Self, end: usize| {
            let candidate = format!("{}\u{2026}", text[..end].trim_end());
            fonts.shape(&candidate, style, max_advance).lines().count() <= max_lines
        };
        let (mut lo, mut hi) = (0_usize, boundaries.len() - 1);
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            if fits(self, boundaries[mid]) {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        let candidate = format!("{}\u{2026}", text[..boundaries[lo]].trim_end());
        self.shape(&candidate, style, max_advance)
    }

    /// Shapes a rich paragraph: one layout, ranged style overrides per
    /// span. Not cached (span lists make poor hash keys; paragraphs are
    /// short and shaping is cheap at this scale).
    fn shape_rich(
        &mut self,
        spans: &[crate::element::Span],
        style: &ResolvedText,
        max_advance: Option<f32>,
    ) -> Layout<LayoutBrush> {
        let text: String = spans.iter().map(|s| s.text.as_str()).collect();
        let base_brush = style.color.to_rgba8().to_u8_array();
        // Family names resolve to owned strings first, so the builder
        // can borrow them for ranged pushes.
        let resolve_family =
            |role: FamilyRole, roles: &std::collections::HashMap<FamilyRole, String>| match role {
                FamilyRole::Sans => "Inter".to_owned(),
                FamilyRole::Mono => String::new(), // marker: generic monospace
                FamilyRole::Display | FamilyRole::Serif => roles
                    .get(&role)
                    .cloned()
                    .unwrap_or_else(|| "Inter".to_owned()),
            };
        let base_family = resolve_family(style.family, &self.roles);
        let span_families: Vec<Option<String>> = spans
            .iter()
            .map(|s| s.family.map(|f| resolve_family(f, &self.roles)))
            .collect();
        fn to_family(name: &str) -> FontFamily<'_> {
            if name.is_empty() {
                FontFamily::Single(GenericFamily::Monospace.into())
            } else {
                FontFamily::named(name)
            }
        }
        let max_advance = max_advance.and_then(|w| w.is_finite().then(|| w.clamp(0.0, 1.0e9)));
        let mut builder = self
            .layout_cx
            .ranged_builder(&mut self.font_cx, &text, 1.0, true);
        builder.push_default(StyleProperty::FontFamily(to_family(&base_family)));
        builder.push_default(StyleProperty::FontSize(style.px));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(style.weight)));
        builder.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(
            style.line_height,
        )));
        builder.push_default(StyleProperty::LetterSpacing(style.letter_spacing));
        builder.push_default(StyleProperty::Brush(base_brush));
        let mut start = 0usize;
        for (i, span) in spans.iter().enumerate() {
            let range = start..start + span.text.len();
            start = range.end;
            if let Some(weight) = span.weight {
                builder.push(
                    StyleProperty::FontWeight(FontWeight::new(weight.value())),
                    range.clone(),
                );
            }
            if let Some(px) = span.size_px {
                builder.push(StyleProperty::FontSize(px), range.clone());
            }
            if let Some(color) = span.color {
                builder.push(
                    StyleProperty::Brush(color.to_rgba8().to_u8_array()),
                    range.clone(),
                );
            }
            if let Some(name) = &span_families[i] {
                builder.push(StyleProperty::FontFamily(to_family(name)), range.clone());
            }
            if span.italic {
                builder.push(
                    StyleProperty::FontStyle(parley::FontStyle::Italic),
                    range.clone(),
                );
            }
        }
        let mut layout = builder.build(&text);
        layout.break_all_lines(max_advance);
        let alignment = match style.align {
            TextAlign::Start => Alignment::Start,
            TextAlign::Center => Alignment::Center,
            TextAlign::End => Alignment::End,
        };
        layout.align(alignment, AlignmentOptions::default());
        layout
    }

    /// Measured size of a rich paragraph at the given wrap width.
    pub(crate) fn measure_rich(
        &mut self,
        spans: &[crate::element::Span],
        style: &ResolvedText,
        max_advance: Option<f32>,
    ) -> (f32, f32) {
        let layout = self.shape_rich(spans, style, max_advance);
        (layout.width().ceil(), layout.height().ceil())
    }

    /// First-line baseline of a rich paragraph.
    pub(crate) fn first_baseline_rich(
        &mut self,
        spans: &[crate::element::Span],
        style: &ResolvedText,
        max_advance: Option<f32>,
    ) -> f32 {
        let layout = self.shape_rich(spans, style, max_advance);
        layout
            .lines()
            .next()
            .map_or(0.0, |line| line.metrics().baseline)
    }

    /// Paints a rich paragraph; each glyph run draws with its span brush.
    pub(crate) fn paint_rich(
        &mut self,
        scene: &mut Scene,
        spans: &[crate::element::Span],
        style: &ResolvedText,
        rect: Rect,
        selection: Option<(parley::Selection, Color)>,
    ) {
        #[expect(clippy::cast_possible_truncation, reason = "logical px fit in f32")]
        let max_advance = Some(rect.width() as f32);
        let transform = Affine::translate((rect.x0, rect.y0));
        if let Some((sel, highlight)) = selection {
            for r in self.static_selection_rects(
                &crate::frame::StaticText::Rich(spans),
                style,
                max_advance,
                sel,
            ) {
                scene.fill(Fill::NonZero, transform, highlight, None, &r);
            }
        }
        let layout = self.shape_rich(spans, style, max_advance);
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let [r, g, b, a] = glyph_run.style().brush;
                let color = Color::from_rgba8(r, g, b, a);
                let mut x = glyph_run.offset();
                let y = glyph_run.baseline();
                let run = glyph_run.run();
                let glyph_xform = run
                    .synthesis()
                    .skew()
                    .map(|angle| Affine::skew(f64::from(angle.to_radians().tan()), 0.0));
                scene
                    .draw_glyphs(run.font())
                    .brush(color)
                    .hint(true)
                    .transform(transform)
                    .glyph_transform(glyph_xform)
                    .font_size(run.font_size())
                    .normalized_coords(run.normalized_coords())
                    .draw(
                        Fill::NonZero,
                        glyph_run.glyphs().map(|glyph| {
                            let gx = x + glyph.x;
                            let gy = y + glyph.y;
                            x += glyph.advance;
                            vello::Glyph {
                                id: glyph.id,
                                x: gx,
                                y: gy,
                            }
                        }),
                    );
            }
        }
    }

    /// The layout a static text node shapes with (cached for plain
    /// text; rich paragraphs shape per call).
    fn static_layout(
        &mut self,
        text: &crate::frame::StaticText<'_>,
        style: &ResolvedText,
        max_advance: Option<f32>,
    ) -> std::borrow::Cow<'_, Layout<LayoutBrush>> {
        match text {
            crate::frame::StaticText::Plain(s) => {
                std::borrow::Cow::Borrowed(self.layout(s, style, max_advance))
            }
            crate::frame::StaticText::Rich(spans) => {
                std::borrow::Cow::Owned(self.shape_rich(spans, style, max_advance))
            }
        }
    }

    /// Starts a static selection at a local point: press count 1 places
    /// a collapsed selection, 2 selects the word, 3 the line.
    pub(crate) fn static_select(
        &mut self,
        text: &crate::frame::StaticText<'_>,
        style: &ResolvedText,
        max_advance: Option<f32>,
        count: u8,
        x: f32,
        y: f32,
    ) -> parley::Selection {
        let layout = self.static_layout(text, style, max_advance);
        match count {
            2 => parley::Selection::word_from_point(layout.as_ref(), x, y),
            3 => parley::Selection::line_from_point(layout.as_ref(), x, y),
            _ => parley::Selection::from_point(layout.as_ref(), x, y),
        }
    }

    /// Extends a static selection to a local point (drag).
    pub(crate) fn static_extend(
        &mut self,
        text: &crate::frame::StaticText<'_>,
        style: &ResolvedText,
        max_advance: Option<f32>,
        sel: parley::Selection,
        x: f32,
        y: f32,
    ) -> parley::Selection {
        let layout = self.static_layout(text, style, max_advance);
        sel.extend_to_point(layout.as_ref(), x, y)
    }

    /// Selection highlight rects for painting, in layout-local coords.
    pub(crate) fn static_selection_rects(
        &mut self,
        text: &crate::frame::StaticText<'_>,
        style: &ResolvedText,
        max_advance: Option<f32>,
        sel: parley::Selection,
    ) -> Vec<Rect> {
        let layout = self.static_layout(text, style, max_advance);
        sel.geometry(layout.as_ref())
            .into_iter()
            .map(|(bb, _)| Rect::new(bb.x0, bb.y0, bb.x1, bb.y1))
            .collect()
    }

    /// Measured size of `text` at the given wrap width, for taffy.
    pub(crate) fn measure(
        &mut self,
        text: &str,
        style: &ResolvedText,
        max_advance: Option<f32>,
    ) -> (f32, f32) {
        let layout = self.layout(text, style, max_advance);
        (layout.width().ceil(), layout.height().ceil())
    }

    /// Distance from the top of the text box to the first line's baseline.
    pub(crate) fn first_baseline(
        &mut self,
        text: &str,
        style: &ResolvedText,
        max_advance: Option<f32>,
    ) -> f32 {
        let layout = self.layout(text, style, max_advance);
        layout
            .lines()
            .next()
            .map_or(0.0, |line| line.metrics().baseline)
    }

    /// Paints `text` into the scene at `rect` (the laid-out text box).
    pub(crate) fn paint(
        &mut self,
        scene: &mut Scene,
        text: &str,
        style: &ResolvedText,
        rect: Rect,
        selection: Option<(parley::Selection, Color)>,
    ) {
        #[expect(clippy::cast_possible_truncation, reason = "logical px fit in f32")]
        let max_advance = Some(rect.width() as f32);
        let color = style.color;
        let transform = Affine::translate((rect.x0, rect.y0));
        if let Some((sel, highlight)) = selection {
            for r in self.static_selection_rects(
                &crate::frame::StaticText::Plain(text),
                style,
                max_advance,
                sel,
            ) {
                scene.fill(Fill::NonZero, transform, highlight, None, &r);
            }
        }
        let layout = self.layout(text, style, max_advance);
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let mut x = glyph_run.offset();
                let y = glyph_run.baseline();
                let run = glyph_run.run();
                let glyph_xform = run
                    .synthesis()
                    .skew()
                    .map(|angle| Affine::skew(f64::from(angle.to_radians().tan()), 0.0));
                scene
                    .draw_glyphs(run.font())
                    .brush(color)
                    .hint(true)
                    .transform(transform)
                    .glyph_transform(glyph_xform)
                    .font_size(run.font_size())
                    .normalized_coords(run.normalized_coords())
                    .draw(
                        Fill::NonZero,
                        glyph_run.glyphs().map(|glyph| {
                            let gx = x + glyph.x;
                            let gy = y + glyph.y;
                            x += glyph.advance;
                            vello::Glyph {
                                id: glyph.id,
                                x: gx,
                                y: gy,
                            }
                        }),
                    );
            }
        }
    }
}
