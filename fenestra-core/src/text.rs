//! Text: the embedded Inter family, fontique registration with system
//! fallback, the parley layout cache, max-lines ellipsis truncation, and
//! glyph painting into vello scenes.

use std::cmp::Ordering;
use std::collections::HashMap;

use fontique::{Blob, CollectionOptions, GenericFamily};
use kurbo::{Affine, Rect};
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, FontWeight, Layout, LayoutContext,
    LineHeight, PositionedLayoutItem, StyleProperty,
};
use peniko::{Color, Fill};
use vello::Scene;

use crate::style::{FontFeatures, OpticalSizing, TextAlign, TextStyle, TextWrap};
use crate::theme::Theme;
use crate::tokens::{FamilyRole, tracking_em};

/// Wrap-width search grid in logical px, deliberately equal to the layout
/// cache's quarter-px bucket (`LayoutKey::width_bucket`), so the searched
/// width and the cache bucket quantize on the same grid and the measure
/// and paint passes converge to the same cache entry.
const WRAP_GRID: f32 = 0.25;

const INTER_REGULAR: &[u8] = include_bytes!("../assets/inter/Inter-Regular.otf");
const INTER_MEDIUM: &[u8] = include_bytes!("../assets/inter/Inter-Medium.otf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/inter/Inter-SemiBold.otf");

/// Preferred mono families, matched against installed system fonts in order.
/// The `monospace` generic remains the final fallback.
const MONO_FAMILIES: &[&str] = &["SF Mono", "Cascadia Code", "JetBrains Mono"];

/// Parley brush type. Colors are applied at draw time via `DrawGlyphs`, so
/// layouts are color-independent and cache across recolors.
pub(crate) type LayoutBrush = [u8; 4];

/// Clamps a wrap width to parley's safe finite range (its line breaker
/// overflows and asserts on enormous or non-finite advances). `None`
/// (unbounded) passes through.
fn clamp_advance(max_advance: Option<f32>) -> Option<f32> {
    max_advance.and_then(|w| w.is_finite().then(|| w.clamp(0.0, 1.0e9)))
}

/// Word count of the last line's source text — the orphan predicate for
/// [`prettify`].
fn last_line_word_count(layout: &Layout<LayoutBrush>, text: &str) -> usize {
    layout
        .lines()
        .last()
        .map(|l| text[l.text_range()].split_whitespace().count())
        .unwrap_or(0)
}

/// Re-applies horizontal alignment after a re-break (re-breaking clears
/// line data, so alignment must be recomputed).
fn apply_align(layout: &mut Layout<LayoutBrush>, align: TextAlign) {
    let alignment = match align {
        TextAlign::Start => Alignment::Start,
        TextAlign::Center => Alignment::Center,
        TextAlign::End => Alignment::End,
    };
    layout.align(alignment, AlignmentOptions::default());
}

/// The width to report to layout for a (possibly refined) text box. For a
/// refined layout whose wrap width `w*` is narrower than the requested
/// width, reports `w*` (`= layout_max_advance().ceil()`) so the box is
/// never narrower than `w*` and the paint pass — which re-wraps at the box
/// width — re-derives the identical break. Otherwise the natural content
/// width.
fn box_width(layout: &Layout<LayoutBrush>, upper: Option<f32>) -> f32 {
    let lma = layout.layout_max_advance();
    match upper {
        Some(u) if lma.is_finite() && lma < u - 0.5 => lma.ceil(),
        _ => layout.width().ceil(),
    }
}

/// Balances line lengths in place (CSS `text-wrap: balance`): finds the
/// smallest grid-aligned wrap width still yielding the greedy line count
/// `n`, so the `n` lines come out evenly filled. `O(log W)` re-break
/// passes; no glyph re-shaping. `upper` is the clamped requested width.
fn rebalance(layout: &mut Layout<LayoutBrush>, style: &ResolvedText, upper: f32) {
    let n = layout.len();
    if n < 2 {
        return; // a single line is already balanced
    }
    let lo = layout.calculate_content_widths().min; // widest unbreakable token
    if lo >= upper {
        return;
    }
    // Line count is monotonic non-decreasing as the width shrinks, and
    // equals `n` at `upper`. Binary-search the smallest grid width in
    // `(lo, upper]` that still yields `n` lines.
    let mut lo_w = (lo / WRAP_GRID).floor() * WRAP_GRID; // count > n side
    let mut hi_w = (upper / WRAP_GRID).ceil() * WRAP_GRID; // known good (n lines)
    while hi_w - lo_w > WRAP_GRID {
        let mid = ((lo_w + hi_w) * 0.5 / WRAP_GRID).round() * WRAP_GRID;
        layout.break_all_lines(Some(mid));
        if layout.len() <= n {
            hi_w = mid;
        } else {
            lo_w = mid;
        }
    }
    layout.break_all_lines(Some(hi_w));
    apply_align(layout, style.align); // re-align after the final re-break
}

/// Avoids a stranded last word in place (CSS `text-wrap: pretty`),
/// best-effort: the largest grid width below `upper` that keeps the greedy
/// line count `n` and gives the last line `>= 2` words. When none exists,
/// restores the greedy break unchanged. Re-break only; no re-shaping.
fn prettify(layout: &mut Layout<LayoutBrush>, text: &str, style: &ResolvedText, upper: f32) {
    let n = layout.len();
    if n < 2 || last_line_word_count(layout, text) >= 2 {
        return; // no orphan to fix
    }
    let lo = layout.calculate_content_widths().min;
    if lo >= upper {
        return;
    }
    // Scan downward in grid steps for the widest (least disruptive) width
    // that fixes the orphan without adding a line; stop early once
    // narrowing would add one.
    let mut w = (upper / WRAP_GRID).floor() * WRAP_GRID;
    let mut fixed = None;
    while w > lo {
        layout.break_all_lines(Some(w));
        match layout.len().cmp(&n) {
            Ordering::Equal if last_line_word_count(layout, text) >= 2 => {
                fixed = Some(w);
                break;
            }
            Ordering::Greater => break, // narrowing now adds a line: give up
            _ => {}
        }
        w -= WRAP_GRID;
    }
    // Re-break at the fix width, or restore the greedy break, then re-align.
    layout.break_all_lines(Some(fixed.unwrap_or(upper)));
    apply_align(layout, style.align);
}

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
    /// OpenType features applied to the run.
    pub features: FontFeatures,
    /// Line-breaking refinement (greedy / balance / pretty).
    pub wrap: TextWrap,
    /// Optical sizing: drives a variable font's `opsz` axis (no-op on static
    /// faces). The base value applies at [`px`](Self::px); rich spans that
    /// override the size re-track `opsz` to their own size under
    /// [`OpticalSizing::Auto`].
    pub optical: OpticalSizing,
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
        // Inter's optical tracking at the actual px (covers free-form display
        // sizes too); an explicit override still wins.
        letter_spacing: ts.letter_spacing.unwrap_or_else(|| tracking_em(px)) * px,
        family: ts.family,
        align: ts.align,
        max_lines: ts.max_lines,
        color: ts.color.unwrap_or(theme.text),
        features: ts.features,
        wrap: ts.wrap,
        optical: ts.optical,
    }
}

/// The CSS `font-variation-settings` source applying an explicit `opsz`
/// (optical size) axis value, e.g. `"opsz" 16`. parley parses this grammar
/// (`<string> <number>`); the float prints in its shortest form, so an
/// integral value emits without a trailing `.0`.
fn opsz_source(opsz: f32) -> String {
    format!("\"opsz\" {opsz}")
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
    /// OpenType features (every flag affects shaping, so all are hashed).
    features: FontFeatures,
    /// Line-breaking refinement; different modes search to different wrap
    /// widths, so the result must not be cached across modes.
    wrap: TextWrap,
    /// Resolved `opsz` axis value bits (variable-font optical sizing);
    /// `u32::MAX` = no axis set. `f32::to_bits` of a finite non-negative
    /// value is always `< u32::MAX`, so it never collides with the sentinel.
    opsz: u32,
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
            features: style.features,
            wrap: style.wrap,
            opsz: match style.optical.opsz_at(style.px) {
                Some(v) => v.to_bits(),
                None => u32::MAX,
            },
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

    /// Shapes and greedily wraps `text` (parley's native line breaking).
    /// The pure pass: no balance/pretty refinement, so the prefix search in
    /// [`Self::truncate`] and the `ch` advance in [`Self::ch_width`] use it
    /// directly.
    fn shape_greedy(
        &mut self,
        text: &str,
        style: &ResolvedText,
        max_advance: Option<f32>,
    ) -> Layout<LayoutBrush> {
        // parley's line breaker overflows (and asserts) on enormous or
        // non-finite advances; clamp at the boundary.
        let max_advance = clamp_advance(max_advance);
        // Registered faces win for every role; Sans falls back to the
        // embedded Inter, Mono to the generic system monospace.
        let family = match self.roles.get(&style.family) {
            Some(name) => FontFamily::named(name),
            None => match style.family {
                FamilyRole::Mono => FontFamily::Single(GenericFamily::Monospace.into()),
                _ => FontFamily::named("Inter"),
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
        if let Some(s) = style.features.feature_string() {
            builder.push_default(StyleProperty::FontFeatures(parley::FontFeatures::Source(
                std::borrow::Cow::Owned(s),
            )));
        }
        // Optical sizing: drive the `opsz` axis (variable faces only; a no-op
        // for the embedded static fonts). Applied after weight, whose `wght`
        // axis fontique already sets via the matched font's synthesis.
        if let Some(opsz) = style.optical.opsz_at(style.px) {
            builder.push_default(StyleProperty::FontVariations(
                parley::FontVariations::Source(std::borrow::Cow::Owned(opsz_source(opsz))),
            ));
        }
        let mut layout = builder.build(text);
        layout.break_all_lines(max_advance);
        apply_align(&mut layout, style.align);
        layout
    }

    /// Shapes `text` with the greedy wrap, then refines the line breaks per
    /// [`ResolvedText::wrap`] (balance / pretty). Refinement re-breaks the
    /// already-shaped layout — no glyph re-shaping — and only runs with a
    /// finite wrap width; [`TextWrap::Normal`] (the default) is a pure passthrough.
    fn shape(
        &mut self,
        text: &str,
        style: &ResolvedText,
        max_advance: Option<f32>,
    ) -> Layout<LayoutBrush> {
        let upper = clamp_advance(max_advance);
        let mut layout = self.shape_greedy(text, style, max_advance);
        if let Some(upper) = upper {
            match style.wrap {
                TextWrap::Normal => {}
                TextWrap::Balance => rebalance(&mut layout, style, upper),
                TextWrap::Pretty => prettify(&mut layout, text, style, upper),
            }
        }
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
            fonts
                .shape_greedy(&candidate, style, max_advance)
                .lines()
                .count()
                <= max_lines
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
        self.shape_greedy(&candidate, style, max_advance)
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
            |role: FamilyRole, roles: &std::collections::HashMap<FamilyRole, String>| {
                roles.get(&role).cloned().unwrap_or_else(|| match role {
                    FamilyRole::Mono => String::new(), // marker: generic monospace
                    _ => "Inter".to_owned(),
                })
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
        let max_advance = clamp_advance(max_advance);
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
        if let Some(s) = style.features.feature_string() {
            builder.push_default(StyleProperty::FontFeatures(parley::FontFeatures::Source(
                std::borrow::Cow::Owned(s),
            )));
        }
        // Base optical size at the paragraph's size; spans that override the
        // size re-track `opsz` below (Auto only).
        if let Some(opsz) = style.optical.opsz_at(style.px) {
            builder.push_default(StyleProperty::FontVariations(
                parley::FontVariations::Source(std::borrow::Cow::Owned(opsz_source(opsz))),
            ));
        }
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
                // Under Auto, re-track this span's `opsz` to its own size so a
                // large display span and small body span in one paragraph each
                // get their right optical master.
                if matches!(style.optical, OpticalSizing::Auto)
                    && let Some(opsz) = style.optical.opsz_at(px)
                {
                    builder.push(
                        StyleProperty::FontVariations(parley::FontVariations::Source(
                            std::borrow::Cow::Owned(opsz_source(opsz)),
                        )),
                        range.clone(),
                    );
                }
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
        // Align the greedy break first (so an early-returning refinement on
        // short text keeps a correctly-aligned layout), then refine in place.
        apply_align(&mut layout, style.align);
        if let Some(upper) = max_advance {
            match style.wrap {
                TextWrap::Normal => {}
                TextWrap::Balance => rebalance(&mut layout, style, upper),
                TextWrap::Pretty => prettify(&mut layout, &text, style, upper),
            }
        }
        layout
    }

    /// Measured size of a rich paragraph at the given wrap width.
    pub(crate) fn measure_rich(
        &mut self,
        spans: &[crate::element::Span],
        style: &ResolvedText,
        max_advance: Option<f32>,
    ) -> (f32, f32) {
        let upper = clamp_advance(max_advance);
        let layout = self.shape_rich(spans, style, max_advance);
        (box_width(&layout, upper), layout.height().ceil())
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
        let upper = clamp_advance(max_advance);
        let layout = self.layout(text, style, max_advance);
        (box_width(layout, upper), layout.height().ceil())
    }

    /// Width of one `ch`: the advance of the digit `'0'` shaped in `style`,
    /// in logical px, ignoring letter-spacing (CSS `ch` semantics). Used to
    /// resolve [`crate::style::Length::Ch`] before layout. Sub-pixel: the
    /// raw glyph advance (un-`ceil`'d, unlike [`Self::measure`]).
    pub(crate) fn ch_width(&mut self, style: &ResolvedText) -> f32 {
        let mut zero = *style;
        zero.letter_spacing = 0.0;
        let layout = self.shape_greedy("0", &zero, None);
        let mut advance = 0.0_f32;
        for line in layout.lines() {
            for item in line.items() {
                if let PositionedLayoutItem::GlyphRun(run) = item {
                    for glyph in run.glyphs() {
                        advance += glyph.advance;
                    }
                }
            }
        }
        advance
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Length, Style, TextStyle};
    use crate::tokens::{MEASURE_CH, TextSize};

    fn rt(size: TextSize) -> ResolvedText {
        resolve_text(
            &TextStyle {
                size,
                ..TextStyle::default()
            },
            &Theme::light(),
        )
    }

    /// Sums the glyph advances of `text` shaped in `style` (no caching, no
    /// `ceil`) — the independent reference for the `ch` advance.
    fn glyph_advance_sum(fonts: &mut Fonts, text: &str, style: &ResolvedText) -> f32 {
        let layout = fonts.shape(text, style, None);
        let mut advance = 0.0_f32;
        for line in layout.lines() {
            for item in line.items() {
                if let PositionedLayoutItem::GlyphRun(run) = item {
                    for glyph in run.glyphs() {
                        advance += glyph.advance;
                    }
                }
            }
        }
        advance
    }

    #[test]
    fn ch_width_matches_zero_advance() {
        let mut fonts = Fonts::embedded();
        let style = rt(TextSize::Base);
        let ch = fonts.ch_width(&style);
        // The embedded Inter's `'0'` advance at 16px (~10.09px on the
        // macOS/Metal golden-reference build).
        assert!((9.5..=10.5).contains(&ch), "ch {ch}");
        // It is exactly the summed glyph advance of `'0'` with letter spacing
        // ignored (CSS `ch` semantics).
        let mut zeroed = style;
        zeroed.letter_spacing = 0.0;
        let independent = glyph_advance_sum(&mut fonts, "0", &zeroed);
        assert!((ch - independent).abs() < 1e-3, "ch {ch} vs {independent}");
    }

    #[test]
    fn ch_width_ignores_letter_spacing() {
        let mut fonts = Fonts::embedded();
        let mut tight = rt(TextSize::Base);
        tight.letter_spacing = -2.0;
        let mut loose = rt(TextSize::Base);
        loose.letter_spacing = 5.0;
        // CSS `ch` is the bare `'0'` advance, independent of tracking.
        assert!((fonts.ch_width(&tight) - fonts.ch_width(&loose)).abs() < 1e-3);
    }

    #[test]
    fn resolve_ch_scales_by_zero_advance() {
        let mut fonts = Fonts::embedded();
        let ch = fonts.ch_width(&rt(TextSize::Base));
        let mut style = Style::default().measure(MEASURE_CH);
        style.resolve_ch(ch);
        // The cap is exactly MEASURE_CH zero-advances of px.
        assert_eq!(style.max_width, Length::Px(MEASURE_CH * ch));
        // …which lands the reading column near 525px at body size with the
        // embedded Inter (52ch × ~10.1px '0'; derived, not hardcoded).
        let px = MEASURE_CH * ch;
        assert!((490.0..=560.0).contains(&px), "{MEASURE_CH}ch = {px}px");
    }

    #[test]
    fn resolve_ch_tracks_text_size() {
        let mut fonts = Fonts::embedded();
        let ch_base = fonts.ch_width(&rt(TextSize::Base)); // 16px
        let ch_lg = fonts.ch_width(&rt(TextSize::Lg)); // 20px
        // A larger text style yields a wider measure: the column follows the
        // element's own prose, and the advance scales with the 20/16 ratio.
        assert!(ch_lg > ch_base, "lg {ch_lg} base {ch_base}");
        assert!(
            (ch_lg - ch_base * 1.25).abs() < 0.5,
            "lg {ch_lg} base {ch_base}"
        );
    }

    #[test]
    fn has_ch_only_flags_ch_lengths() {
        assert!(!Style::default().has_ch());
        assert!(Style::default().measure(MEASURE_CH).has_ch());
        assert!(Style::default().w_ch(40.0).has_ch());
        // A pixel width is not a `ch` constraint.
        assert!(!Style::default().w(100.0).has_ch());
    }

    /// True when two styles produce distinct layout cache keys for the same
    /// text and wrap width — i.e. a flag flip is not silently cached away.
    fn keys_differ(a: &ResolvedText, b: &ResolvedText) -> bool {
        LayoutKey::new("0123456789", a, None) != LayoutKey::new("0123456789", b, None)
    }

    /// A `ResolvedText` whose features are set by `f`, everything else default.
    fn rt_feat(f: impl FnOnce(&mut crate::style::FontFeatures)) -> ResolvedText {
        let mut style = rt(TextSize::Base);
        f(&mut style.features);
        style
    }

    #[test]
    fn layout_key_differs_on_spacing() {
        use crate::style::NumericSpacing::{Default, Proportional, Tabular};
        let def = rt_feat(|x| x.spacing = Default);
        let tab = rt_feat(|x| x.spacing = Tabular);
        let prop = rt_feat(|x| x.spacing = Proportional);
        assert!(keys_differ(&def, &tab));
        assert!(keys_differ(&def, &prop));
        assert!(keys_differ(&tab, &prop));
    }

    #[test]
    fn layout_key_differs_on_figures() {
        use crate::style::FigureStyle::{Default, Lining, OldStyle};
        let def = rt_feat(|x| x.figures = Default);
        let lin = rt_feat(|x| x.figures = Lining);
        let old = rt_feat(|x| x.figures = OldStyle);
        assert!(keys_differ(&def, &lin));
        assert!(keys_differ(&def, &old));
        assert!(keys_differ(&lin, &old));
    }

    #[test]
    fn layout_key_differs_on_small_caps() {
        let off = rt_feat(|x| x.small_caps = false);
        let on = rt_feat(|x| x.small_caps = true);
        assert!(keys_differ(&off, &on));
    }

    #[test]
    fn layout_key_differs_on_ligatures() {
        let def = rt_feat(|x| x.ligatures = None);
        let off = rt_feat(|x| x.ligatures = Some(false));
        let on = rt_feat(|x| x.ligatures = Some(true));
        assert!(keys_differ(&def, &off));
        assert!(keys_differ(&def, &on));
        assert!(keys_differ(&off, &on));
    }

    #[test]
    fn layout_key_differs_on_fractions() {
        let off = rt_feat(|x| x.fractions = false);
        let on = rt_feat(|x| x.fractions = true);
        assert!(keys_differ(&off, &on));
    }

    #[test]
    fn layout_key_differs_on_opsz() {
        // Optical sizing is a shaping input, so it must be in the cache key
        // (the 0.16 features-cache lesson): a different opsz value, or Default
        // vs a set value, must never collide. The key hashes the *resolved*
        // opsz, so Auto-at-16px and Fixed(16) coincide (same render, correct
        // hit) while everything else is distinct.
        use crate::style::OpticalSizing::{Auto, Default, Fixed};
        let rt_opt = |o: crate::style::OpticalSizing| {
            let mut s = rt(TextSize::Base); // 16px
            s.optical = o;
            s
        };
        let def = rt_opt(Default);
        let auto = rt_opt(Auto); // opsz = 16
        let fixed_big = rt_opt(Fixed(96.0));
        let fixed_small = rt_opt(Fixed(12.0));
        assert!(keys_differ(&def, &auto), "none vs 16");
        assert!(keys_differ(&def, &fixed_big), "none vs 96");
        assert!(keys_differ(&auto, &fixed_big), "16 vs 96");
        assert!(keys_differ(&fixed_small, &fixed_big), "12 vs 96");
        // Auto@16 and Fixed(16) resolve to the same opsz at the same px: a
        // correct cache hit, not a collision bug.
        assert!(
            !keys_differ(&auto, &rt_opt(Fixed(16.0))),
            "auto@16 == fixed(16)"
        );
    }

    #[test]
    fn opsz_source_formats_clean() {
        assert_eq!(opsz_source(16.0), "\"opsz\" 16");
        assert_eq!(opsz_source(48.0), "\"opsz\" 48");
        assert_eq!(opsz_source(9.5), "\"opsz\" 9.5");
    }

    #[test]
    fn layout_key_equal_when_features_equal() {
        // Negative control: identical features ⇒ identical key (cache hits).
        let a = rt_feat(|x| {
            x.spacing = crate::style::NumericSpacing::Tabular;
            x.small_caps = true;
        });
        let b = rt_feat(|x| {
            x.spacing = crate::style::NumericSpacing::Tabular;
            x.small_caps = true;
        });
        assert!(!keys_differ(&a, &b));
    }

    // ----- text-wrap: balance / pretty -----

    /// A `ResolvedText` at a free-form px size with a given wrap mode,
    /// everything else default (display leading 1.25).
    fn rt_px(px: f32, wrap: TextWrap) -> ResolvedText {
        resolve_text(
            &TextStyle {
                size_px: Some(px),
                wrap,
                ..TextStyle::default()
            },
            &Theme::light(),
        )
    }

    /// Visible filled width of each line (advance minus trailing whitespace).
    fn line_advances(layout: &Layout<LayoutBrush>) -> Vec<f32> {
        layout
            .lines()
            .map(|l| {
                let m = l.metrics();
                m.advance - m.trailing_whitespace
            })
            .collect()
    }

    /// Word count of the last line's source text.
    fn last_words(layout: &Layout<LayoutBrush>, text: &str) -> usize {
        layout
            .lines()
            .last()
            .map(|l| text[l.text_range()].split_whitespace().count())
            .unwrap_or(0)
    }

    fn max_adv(advs: &[f32]) -> f32 {
        advs.iter().copied().fold(0.0_f32, f32::max)
    }

    fn min_adv(advs: &[f32]) -> f32 {
        advs.iter().copied().fold(f32::MAX, f32::min)
    }

    #[test]
    fn balance_evens_a_two_line_heading() {
        let mut fonts = Fonts::embedded();
        let text = "Balanced headings keep their lines visually even";
        // Derive the width from the single-line advance so the greedy break
        // is reliably [full, short] on the real Inter metrics, not a guess.
        let full = fonts
            .shape(text, &rt_px(28.0, TextWrap::Normal), None)
            .width();
        let w = full * 0.62;
        let greedy = fonts.shape(text, &rt_px(28.0, TextWrap::Normal), Some(w));
        let bal = fonts.shape(text, &rt_px(28.0, TextWrap::Balance), Some(w));
        assert_eq!(greedy.len(), 2, "precondition: greedy wraps to two lines");
        // Line count is preserved (explicit acceptance criterion).
        assert_eq!(bal.len(), greedy.len(), "N preserved");
        let g = line_advances(&greedy);
        let b = line_advances(&bal);
        // The balanced longest line is shorter, and the spread tightens.
        assert!(
            max_adv(&b) < max_adv(&g),
            "balanced longest {} < greedy longest {}",
            max_adv(&b),
            max_adv(&g)
        );
        assert!(
            (max_adv(&b) - min_adv(&b)) < (max_adv(&g) - min_adv(&g)),
            "balanced spread {} < greedy spread {}",
            max_adv(&b) - min_adv(&b),
            max_adv(&g) - min_adv(&g)
        );
    }

    #[test]
    fn balance_single_line_is_noop() {
        let mut fonts = Fonts::embedded();
        let text = "Short title";
        let full = fonts
            .shape(text, &rt_px(28.0, TextWrap::Normal), None)
            .width();
        let w = full * 2.0; // plenty of room: one line either way
        let normal = fonts.shape(text, &rt_px(28.0, TextWrap::Normal), Some(w));
        let bal = fonts.shape(text, &rt_px(28.0, TextWrap::Balance), Some(w));
        assert_eq!(normal.len(), 1);
        assert_eq!(bal.len(), 1);
        // Same single-line geometry: balance is a no-op (this is why the
        // markdown golden stays byte-identical).
        assert!((normal.width() - bal.width()).abs() < 1e-3);
    }

    #[test]
    fn balance_preserves_line_count_never_overflows() {
        let mut fonts = Fonts::embedded();
        let text = "Balanced headings keep their lines visually even";
        let full = fonts
            .shape(text, &rt_px(28.0, TextWrap::Normal), None)
            .width();
        let w = full * 0.62;
        let greedy = fonts.shape(text, &rt_px(28.0, TextWrap::Normal), Some(w));
        let bal = fonts.shape(text, &rt_px(28.0, TextWrap::Balance), Some(w));
        assert_eq!(bal.len(), greedy.len());
        // No balanced line exceeds the requested upper bound.
        assert!(
            max_adv(&line_advances(&bal)) <= w + 0.5,
            "widest balanced line exceeds W {w}"
        );
    }

    #[test]
    fn pretty_pulls_word_onto_last_line() {
        let mut fonts = Fonts::embedded();
        let text = "Typesetters avoid leaving a single short word stranded alone here";
        let style_n = rt_px(18.0, TextWrap::Normal);
        let full = fonts.shape(text, &style_n, None).width();
        // Scan from the single-line width downward for the widest width that
        // orphans the last word — that orphan is always fixable (narrowing
        // a touch pulls the previous word down without adding a line).
        let mut target = None;
        let mut w = full;
        while w > full * 0.2 {
            let g = fonts.shape(text, &style_n, Some(w));
            if g.len() >= 2 && last_words(&g, text) == 1 {
                target = Some(w);
                break;
            }
            w -= 1.0;
        }
        let w = target.expect("a width that strands the last word");
        let greedy = fonts.shape(text, &style_n, Some(w));
        let pretty = fonts.shape(text, &rt_px(18.0, TextWrap::Pretty), Some(w));
        assert_eq!(pretty.len(), greedy.len(), "pretty adds no line");
        assert!(
            last_words(&pretty, text) >= 2,
            "orphan pulled onto the last line"
        );
    }

    #[test]
    fn pretty_never_worse_when_no_orphan() {
        let mut fonts = Fonts::embedded();
        let text = "These words wrap into lines that each already end with several words";
        let style_n = rt_px(18.0, TextWrap::Normal);
        let full = fonts.shape(text, &style_n, None).width();
        // Find a width whose greedy last line already holds >= 2 words.
        let mut chosen = None;
        let mut w = full;
        while w > full * 0.2 {
            let g = fonts.shape(text, &style_n, Some(w));
            if g.len() >= 2 && last_words(&g, text) >= 2 {
                chosen = Some(w);
                break;
            }
            w -= 1.0;
        }
        let w = chosen.expect("a width with a non-orphan last line");
        let greedy = fonts.shape(text, &style_n, Some(w));
        let pretty = fonts.shape(text, &rt_px(18.0, TextWrap::Pretty), Some(w));
        // Best-effort contract: never adds a line, never shortens the last
        // line, and with no orphan it keeps the greedy break exactly.
        assert_eq!(pretty.len(), greedy.len());
        assert_eq!(last_words(&pretty, text), last_words(&greedy, text));
        assert_eq!(
            line_advances(&pretty),
            line_advances(&greedy),
            "no-op break"
        );
    }

    #[test]
    fn layout_key_differs_on_wrap() {
        let mk = |wrap| {
            let mut s = rt(TextSize::Base);
            s.wrap = wrap;
            s
        };
        let normal = mk(TextWrap::Normal);
        let balance = mk(TextWrap::Balance);
        let pretty = mk(TextWrap::Pretty);
        assert!(keys_differ(&normal, &balance));
        assert!(keys_differ(&normal, &pretty));
        assert!(keys_differ(&balance, &pretty));
        // Equal mode ⇒ equal key (cache hits).
        assert!(!keys_differ(&balance, &mk(TextWrap::Balance)));
    }

    #[test]
    fn balance_idempotent_reproduces_break() {
        let mut fonts = Fonts::embedded();
        let text = "Balanced headings keep their lines visually even";
        let full = fonts
            .shape(text, &rt_px(28.0, TextWrap::Normal), None)
            .width();
        let w = full * 0.62;
        let bal = fonts.shape(text, &rt_px(28.0, TextWrap::Balance), Some(w));
        // The measured box width feeds back as the paint-time wrap width.
        let bw = box_width(&bal, clamp_advance(Some(w)));
        let bal2 = fonts.shape(text, &rt_px(28.0, TextWrap::Balance), Some(bw));
        // Re-deriving the refinement at the box width reproduces the break
        // exactly — the fixpoint that keeps measure and paint in agreement.
        assert_eq!(bal.len(), bal2.len());
        assert_eq!(line_advances(&bal), line_advances(&bal2));
    }

    #[test]
    fn pretty_idempotent_reproduces_break() {
        // Pretty also narrows the measured box (it re-breaks below the column),
        // so paint must re-derive the same break from that box width — the same
        // measure/paint fixpoint balance has.
        let mut fonts = Fonts::embedded();
        let text = "Typesetters avoid leaving a single short word stranded alone here";
        let style_n = rt_px(18.0, TextWrap::Normal);
        let full = fonts.shape(text, &style_n, None).width();
        let mut target = None;
        let mut w = full;
        while w > full * 0.2 {
            let g = fonts.shape(text, &style_n, Some(w));
            if g.len() >= 2 && last_words(&g, text) == 1 {
                target = Some(w);
                break;
            }
            w -= 1.0;
        }
        let w = target.expect("a width that strands the last word");
        let pretty = fonts.shape(text, &rt_px(18.0, TextWrap::Pretty), Some(w));
        let bw = box_width(&pretty, clamp_advance(Some(w)));
        let pretty2 = fonts.shape(text, &rt_px(18.0, TextWrap::Pretty), Some(bw));
        assert_eq!(pretty.len(), pretty2.len());
        assert_eq!(line_advances(&pretty), line_advances(&pretty2));
    }
}
