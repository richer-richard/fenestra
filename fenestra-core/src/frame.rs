//! The frame pipeline: element tree -> ids -> style resolution -> taffy
//! layout (with parley-backed text measurement) -> a [`Frame`] of resolved
//! absolute rects -> vello scene. Pure given `(tree, theme, size, scale)`
//! plus the retained [`FrameState`].

use kurbo::{Point, Rect};
use serde::Serialize;
use taffy::prelude::{AvailableSpace, NodeId, Size, TaffyTree};
use vello::Scene;

use crate::element::{
    DrawerSide, Element, ExitAnim, Kind, Overlay, OverlayMode, OverlayPlacement, PathData,
    Semantics,
};
use crate::frame_state::{ExitRecord, FrameState};
use crate::ghost::{GhostNode, GhostPaint};
use crate::grid;
use crate::id::WidgetId;
use crate::input::{EditorState, InputPaint};
use crate::layout;
use crate::paint_plan::{MultiPassSpec, PaintMode, PassKind};
use crate::painter;
use crate::style::{
    AlignItems, Direction, Display, GridTemplate, Overflow, Paint, Position, Style, Track,
};
use crate::text::{Fonts, ResolvedText, resolve_text};
use crate::theme::Theme;
use crate::tokens::{FOCUS_RING, R_FULL};

/// Scrollbar thumb width and edge inset, logical px.
const SCROLLBAR_WIDTH: f64 = 6.0;
const SCROLLBAR_INSET: f64 = 2.0;
/// Wheel scrolling needs at least this much overflow to engage.
const MIN_SCROLL_RANGE: f32 = 0.5;

/// Taffy node context for measured leaves.
enum MeasureCtx {
    Text {
        text: String,
        style: ResolvedText,
    },
    Rich {
        spans: Vec<crate::element::Span>,
        style: ResolvedText,
    },
    Input {
        /// Current value, measured for multiline height.
        text: String,
        style: ResolvedText,
        multiline: bool,
    },
}

/// Content height of an input leaf: one line for single-line inputs, the
/// wrapped text height (plus the caret line after a trailing newline) for
/// multiline ones.
fn measure_input_height(
    fonts: &mut Fonts,
    text: &str,
    style: &ResolvedText,
    multiline: bool,
    wrap: Option<f32>,
) -> f32 {
    let line = (style.px * style.line_height).ceil();
    if !multiline || text.is_empty() {
        return line;
    }
    // The editor shows a caret line after a trailing newline; measure one.
    let measured: std::borrow::Cow<'_, str> = if text.ends_with('\n') {
        std::borrow::Cow::Owned(format!("{text} "))
    } else {
        std::borrow::Cow::Borrowed(text)
    };
    let (_, h) = fonts.measure(&measured, style, wrap);
    h.max(line)
}

/// Intrinsic width of an unconstrained input, logical px.
const INPUT_DEFAULT_WIDTH: f32 = 220.0;

enum PaintKind {
    Box,
    Text {
        text: String,
        style: ResolvedText,
    },
    Rich {
        spans: Vec<crate::element::Span>,
        style: ResolvedText,
    },
    Path(PathData),
    Input(InputPaint),
    Image(crate::element::ImageData),
}

/// Interactivity facts the frame needs for hit/focus queries.
#[derive(Debug, Clone, Copy, Default)]
struct NodeMeta {
    /// Focusable and enabled.
    focusable: bool,
    focus_ring: bool,
    /// Paint the focus ring in the danger hue (invalid control).
    invalid: bool,
}

/// Scroll geometry of one scrollable container, resolved for this frame.
struct ScrollInfo {
    offset_y: f32,
    offset_x: f32,
    /// Vertical scrollbar thumb (right edge).
    thumb_v: Option<Rect>,
    /// Horizontal scrollbar thumb (bottom edge).
    thumb_h: Option<Rect>,
    alpha: f32,
    /// Content overflows vertically; wheel routing skips containers that fit.
    can_scroll_y: bool,
    /// Content overflows horizontally.
    can_scroll_x: bool,
}

/// The viewport a `position: sticky` element sticks within — the nearest
/// scroll-container ancestor's content rect, in canvas coordinates.
#[derive(Clone, Copy)]
struct StickyCtx {
    viewport: Rect,
}

/// Clamps a sticky element's natural rect to its scroll viewport per its
/// `sticky_*` thresholds, keeping its size. With no scrolling ancestor the
/// natural rect is returned unchanged (sticky is inert).
fn apply_sticky(natural: Rect, style: &Style, ctx: Option<StickyCtx>) -> Rect {
    let Some(ctx) = ctx else {
        return natural;
    };
    let v = ctx.viewport;
    let (w, h) = (natural.width(), natural.height());
    let mut x0 = natural.x0;
    let mut y0 = natural.y0;
    // Apply the bottom/right (`min`) constraints first, then top/left (`max`),
    // so that when both edges are set and conflict, top/left win — per CSS
    // positioned-layout rules (the last `max` overrides the earlier `min`).
    if let Some(b) = style.sticky_bottom {
        y0 = y0.min((v.y1 - f64::from(b) - h).max(v.y0));
    }
    if let Some(t) = style.sticky_top {
        // Don't push below the element's natural position, nor past the viewport.
        y0 = y0.max((v.y0 + f64::from(t)).min(v.y1 - h));
    }
    if let Some(r) = style.sticky_right {
        x0 = x0.min((v.x1 - f64::from(r) - w).max(v.x0));
    }
    if let Some(l) = style.sticky_left {
        x0 = x0.max((v.x0 + f64::from(l)).min(v.x1 - w));
    }
    Rect::new(x0, y0, x0 + w, y0 + h)
}

/// One node with its final absolute logical rect.
struct FrameNode {
    id: WidgetId,
    kind: PaintKind,
    style: Style,
    rect: Rect,
    /// Effective clip rect inherited from ancestors (None = unclipped).
    visible: Option<Rect>,
    /// `position: sticky` — painted and hit-tested after its siblings (on top).
    is_sticky: bool,
    scroll: Option<ScrollInfo>,
    meta: NodeMeta,
    /// Continuous rotation period (ms) for spinner paths.
    spin: Option<f32>,
    /// Accessibility projection: role/state, name, value, and user key.
    access: (
        Option<Semantics>,
        Option<String>,
        Option<String>,
        Option<String>,
    ),
    /// Live region (polite announcements).
    live: bool,
    /// Text inputs: selected byte range (collapsed = caret position).
    selection: Option<(usize, usize)>,
    /// FLIP/shared-element layout animation: slide from the previous measured
    /// position when this node's center moves between frames.
    animate_layout: bool,
    /// Exit animation to play when this node is removed from the tree.
    exit: Option<ExitAnim>,
    /// Builder call site, for `debug_tree`.
    source: &'static std::panic::Location<'static>,
    children: Vec<FrameNode>,
}

/// One node of a frame's accessibility projection (see
/// [`Frame::access_tree`]): plain data, usable headlessly in tests and
/// mapped to AccessKit by the windowed shell.
#[derive(Debug, Clone)]
pub struct AccessNode {
    /// Stable widget identity (also the platform node id).
    pub id: WidgetId,
    /// Role and state, when the element exposes one.
    pub semantics: Option<Semantics>,
    /// Accessible name.
    pub label: Option<String>,
    /// Current value (text inputs).
    pub value: Option<String>,
    /// Layout rect in logical px.
    pub rect: Rect,
    /// Keyboard focusable (and enabled).
    pub focusable: bool,
    /// Marked invalid (the danger-hued control state — ARIA `aria-invalid`).
    pub invalid: bool,
    /// The stable key assigned via `.id("...")`, when one was set.
    pub key: Option<String>,
    /// Live region: content changes are announced politely.
    pub live: bool,
    /// Text inputs: the selected byte range in the value (collapsed =
    /// caret position). Headlessly testable selection state.
    pub selection: Option<(usize, usize)>,
    /// Children in paint order.
    pub children: Vec<AccessNode>,
}

/// One text node's legibility, measured on the real resolved colors and size —
/// produced by [`Frame::legibility`]. Reports both the APCA `Lc` and the WCAG 2
/// ratio against the floor each standard sets for the rendered size, so an agent
/// can prove a screen is readable without looking at a single pixel.
#[derive(Debug, Clone)]
pub struct TextLegibility {
    /// The text whose legibility this describes.
    pub text: String,
    /// Resolved foreground (text) color.
    pub fg: crate::Color,
    /// Effective background behind the text. A solid ancestor fill, the window
    /// background when no ancestor fills, or — for text over a gradient fill —
    /// the worst-contrast gradient stop (the honest legibility bound). See
    /// [`bg_uniform`](Self::bg_uniform).
    pub bg: crate::Color,
    /// Whether [`bg`](Self::bg) is a single uniform color. `false` when the text
    /// sits over a gradient fill, in which case `bg` is the worst-contrast stop
    /// sampled across the field, so `passes_apca`/`passes_wcag2` are worst-case.
    pub bg_uniform: bool,
    /// Rendered size in logical pixels.
    pub size_px: f32,
    /// Numeric OpenType weight.
    pub weight: f32,
    /// Measured APCA `Lc` magnitude.
    pub lc: f64,
    /// The APCA `Lc` floor required at this size and weight.
    pub required_lc: f64,
    /// WCAG 2 contrast ratio.
    pub wcag2: f64,
    /// Whether the text clears its APCA floor.
    pub passes_apca: bool,
    /// Whether the text clears WCAG 2 AA at its size.
    pub passes_wcag2: bool,
    /// Layout rect of the text in logical pixels.
    pub rect: Rect,
}

/// A static text payload borrowed from the frame (plain or rich).
pub(crate) enum StaticText<'a> {
    Plain(&'a str),
    Rich(&'a [crate::element::Span]),
}

impl StaticText<'_> {
    /// The full string spans shape over (owned only for rich text).
    pub(crate) fn to_text(&self) -> std::borrow::Cow<'_, str> {
        match self {
            Self::Plain(s) => std::borrow::Cow::Borrowed(s),
            Self::Rich(spans) => {
                std::borrow::Cow::Owned(spans.iter().map(|s| s.text.as_str()).collect())
            }
        }
    }
}

/// One realized overlay, painted above the root in stack order.
struct OverlayFrame {
    id: WidgetId,
    mode: OverlayMode,
    node: FrameNode,
    /// Enter progress 0..=1 (drives backdrop fade and slide-up).
    progress: f32,
    backdrop: bool,
    trap_focus: bool,
    hittable: bool,
}

/// A laid-out frame: resolved styles and absolute rects for every element.
/// Paint, input routing, and debug dumps all read from this one structure.
pub struct Frame {
    root: FrameNode,
    overlays: Vec<OverlayFrame>,
    /// Anchor id -> (overlay id, mode) for every overlay child present in
    /// the tree (open or not); dispatch uses it for toggling.
    overlay_anchors: std::collections::HashMap<WidgetId, (WidgetId, OverlayMode)>,
    canvas: Rect,
    scale: f64,
    thumb_color: crate::Color,
    ring_color: crate::Color,
    /// Focus ring for controls marked invalid (danger hue).
    ring_color_invalid: crate::Color,
    /// Static-text selection highlight (matches input selections).
    selection_color: crate::Color,
    /// `true` while any scrollbar fade, style transition, caret blink, or
    /// overlay animation is running; the runner keeps scheduling frames.
    pub animating: bool,
}

// ---------------------------------------------------------------- building

struct BuiltNode {
    taffy: NodeId,
    id: WidgetId,
    kind: PaintKind,
    style: Style,
    focusable: bool,
    disabled: bool,
    /// Recolors the keyboard focus ring to the danger hue.
    invalid: bool,
    spin: Option<f32>,
    /// Scroll containers: pin to the bottom while content grows.
    stick_bottom: bool,
    /// Accessibility projection: role/state, name, value, and user key.
    access: (
        Option<Semantics>,
        Option<String>,
        Option<String>,
        Option<String>,
    ),
    /// Live region (polite announcements).
    live: bool,
    /// Text inputs: selected byte range (collapsed = caret position).
    selection: Option<(usize, usize)>,
    /// FLIP/shared-element layout animation flag.
    animate_layout: bool,
    /// Exit animation to play on removal.
    exit: Option<ExitAnim>,
    /// Builder call site, for `debug_tree`.
    source: &'static std::panic::Location<'static>,
    children: Vec<BuiltNode>,
}

/// The solid color of a style's fill, if it has one (gradients have none).
fn solid_fill(style: &Style) -> Option<crate::Color> {
    match &style.fill {
        Some(Paint::Solid(c)) => Some(*c),
        _ => None,
    }
}

/// The effective background field a node contributes during the legibility walk:
/// a single solid color, or a gradient's stops. A gradient is not one color, so
/// it is carried as its stops and sampled worst-case under each text node.
#[derive(Clone)]
enum BgField {
    Solid(crate::Color),
    Gradient(Vec<crate::style::GradientStop>),
}

/// The gradient color at offset `t` in `0.0..=1.0`, interpolating between the
/// bounding stops in OKLCH (the space the gradient constructors build in).
/// `stops` must be non-empty and sorted by offset.
fn gradient_color_at(stops: &[crate::style::GradientStop], t: f32) -> crate::Color {
    let last = stops.len() - 1;
    if t <= stops[0].offset {
        return stops[0].color;
    }
    if t >= stops[last].offset {
        return stops[last].color;
    }
    for pair in stops.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if t <= b.offset {
            let span = b.offset - a.offset;
            let local = if span > 0.0 {
                (t - a.offset) / span
            } else {
                0.0
            };
            return crate::anim::lerp_color(a.color, b.color, local);
        }
    }
    stops[last].color
}

/// The worst-contrast (lowest APCA `Lc`) background a text `fg` faces over a
/// gradient field, sampled densely *along* the field — not just at the declared
/// stops — so an interior dead-zone between two stops (where the field passes
/// through `fg`'s own luminance) is caught. `None` for empty stops.
fn gradient_worst_bg(
    stops: &[crate::style::GradientStop],
    fg: crate::Color,
) -> Option<crate::Color> {
    if stops.is_empty() {
        return None;
    }
    const SAMPLES: u16 = 32;
    let mut worst: Option<(f64, crate::Color)> = None;
    for i in 0..=SAMPLES {
        let t = f32::from(i) / f32::from(SAMPLES);
        let c = gradient_color_at(stops, t);
        let lc = crate::apca::lc_abs(fg, c);
        if worst.is_none_or(|(w, _)| lc < w) {
            worst = Some((lc, c));
        }
    }
    worst.map(|(_, c)| c)
}

/// The state-layer veil opacity for an element this frame: the strongest
/// applicable interaction state wins (drag > press = focus > hover). Keyboard
/// focus, not pointer focus, raises the focus layer — matching the ring.
/// `None` means no veil (resting).
fn state_layer_opacity(state: &FrameState, id: WidgetId, draggable: bool) -> Option<f32> {
    let sl = crate::tokens::STATE_LAYER;
    let mut op = 0.0_f32;
    if state.is_hovered(id) {
        op = op.max(sl.hover);
    }
    if state.focus_visible && state.focused() == Some(id) {
        op = op.max(sl.focus);
    }
    if state.is_active(id) {
        op = op.max(sl.press);
        if draggable && state.dragging.is_some() {
            op = op.max(sl.drag);
        }
    }
    (op > 0.0).then_some(op)
}

/// Resolves an element's style against the theme: applies the deferred
/// `themed` styling, overlays interaction variants from state (per-widget
/// closures and the uniform state layer), expands shadow tokens, fills
/// role-based defaults, and advances any transition. Returns the style to
/// paint and whether a transition is still running.
fn resolve<Msg>(
    el: &Element<Msg>,
    theme: &Theme,
    state: &mut FrameState,
    id: WidgetId,
) -> (Style, bool) {
    let mut style = el.style.clone();
    if let Some(f) = &el.themed {
        style = f(theme, style);
    }
    if !el.disabled {
        if state.is_hovered(id)
            && let Some(f) = &el.hover_style
        {
            style = f(theme, style);
        }
        if state.is_active(id)
            && let Some(f) = &el.active_style
        {
            style = f(theme, style);
        }
        if state.focused() == Some(id)
            && let Some(f) = &el.focus_style
        {
            style = f(theme, style);
        }
    }
    // Continuous (squircle) corners are a theme-wide default: an element that
    // set no smoothing of its own inherits the theme's — but only where it has
    // a finite rounded corner. Square boxes keep the exact circular-arc path,
    // and pills/avatars (R_FULL, an infinite radius) stay perfectly round.
    if style.corner_smoothing.is_none() {
        let radii = [
            style.corner_radius.tl,
            style.corner_radius.tr,
            style.corner_radius.br,
            style.corner_radius.bl,
        ];
        let rounded = radii.iter().any(|&r| r > 0.0) && radii.iter().all(|&r| r.is_finite());
        style.corner_smoothing = Some(if rounded { theme.corner_smoothing } else { 0.0 });
    }
    // The uniform Material state layer: a translucent veil of the content
    // color, baked into the fill so it animates as a color change. One recipe
    // for every control that opts in, replacing per-state color swaps.
    if let Some(content_fn) = &el.state_layer {
        let content = content_fn(theme);
        if el.disabled {
            // Inert: blend the content color into the resting surface at the
            // disabled-container share, and drop the raised affordances.
            let base = solid_fill(&style).unwrap_or(theme.surface);
            let veil = content.with_alpha(crate::tokens::STATE_LAYER.disabled_container);
            style.fill = Some(Paint::Solid(crate::anim::over(veil, base)));
            style.border = None;
            style.shadows.clear();
            style.shadow_token = None;
            style.highlight_top = None;
        } else if let Some(op) = state_layer_opacity(state, id, el.drag_source.is_some()) {
            // Bake the veil into the fill so it rides the color transition.
            // Over a solid container it composites to a solid (the control
            // fades from its rest color); with no container it stays a
            // translucent veil (ghost controls fade from a transparent base).
            // Gradient fills are left untouched — a veil over them is not a
            // single color.
            match &style.fill {
                Some(Paint::Solid(c)) => {
                    style.fill = Some(Paint::Solid(crate::anim::over(content.with_alpha(op), *c)));
                }
                None => style.fill = Some(Paint::Solid(content.with_alpha(op))),
                Some(_) => {}
            }
        }
    }
    // Press feedback: a subtle paint-time shrink while held (pointer down).
    if el.press_scale && !el.disabled && state.is_active(id) {
        style.scale = crate::tokens::PRESS_SCALE;
    }
    // shadcn focus ring: a keyboard-focused control swaps its border to the
    // ring color (danger when invalid); the soft halo is painted separately.
    if !el.disabled
        && state.focus_visible
        && state.focused() == Some(id)
        && let Some(border) = style.border.as_mut()
    {
        border.color = if el.invalid {
            theme.danger.solid
        } else {
            theme.accent
        };
    }
    if let Some(token) = style.shadow_token {
        let mut layers = theme.shadow(token);
        layers.append(&mut style.shadows);
        style.shadows = layers;
    }
    if matches!(el.kind, Kind::Divider) && style.fill.is_none() {
        style.fill = Some(Paint::Solid(theme.border_subtle));
    }
    if style.text.color.is_none() {
        style.text.color = Some(theme.text);
    }
    // Optical sizing inherits the theme default (`Auto` out of the box) unless
    // the element set its own — the kit-wide `font-optical-sizing` knob.
    if style.text.optical == crate::style::OpticalSizing::Inherit {
        style.text.optical = theme.optical_sizing;
    }

    let mut animating = false;
    let transition = match (el.transition, el.enter) {
        (Some(t), _) => Some(t),
        // Enter-only elements still need a transition to play through.
        (None, Some(enter)) => Some(enter),
        // A bare `.animate_layout()` element still needs a retained animation
        // to carry the FLIP slide: a spatial spring is its implicit
        // transition. The post-realize FLIP pass retargets it to the measured
        // position delta. (A declared `.transition()`/`.enter()` wins and
        // drives the slide instead.)
        (None, None) if el.animate_layout => Some(crate::style::Transition::spring()),
        (None, None) => None,
    };
    // Keyboard-driven state changes snap: a keyboard-focused control shows its
    // focus ring and state layer instantly, since keyboard users move between
    // controls faster than a fade can keep up.
    let keyboard_driven = state.focus_visible && state.focused() == Some(id);
    if let Some(transition) = transition
        && !state.reduced_motion
        && !keyboard_driven
    {
        let now = state.now();
        let seen = state.frame_no;
        let anim = state.anims.entry(id).or_insert_with(|| {
            // First appearance: enter-animated elements seed from the
            // target faded out, so they play in toward it.
            let seed = if el.enter.is_some() {
                let mut from = style.clone();
                from.opacity = 0.0;
                from
            } else {
                style.clone()
            };
            crate::anim::Anim::new(seed, now, seen)
        });
        let (animated, running) = anim.advance(&style, transition, now, seen);
        style = animated;
        animating = running;
    }
    if let Some(kf) = &el.keyframes {
        style = crate::anim::sample_keyframes(kf, theme, &style, state.now(), state.reduced_motion);
        // Looping timelines repaint for as long as they are mounted.
        animating |= !state.reduced_motion && !kf.stops.is_empty();
    }
    (style, animating)
}

/// An overlay child discovered during the main build pass: the path
/// navigates from the root element to the overlay element.
struct PendingOverlay {
    anchor: WidgetId,
    id: WidgetId,
    def: Overlay,
    path: Vec<usize>,
}

/// How many times [`build`] follows a `responsive` wrapper under one id before
/// giving up. Real use needs exactly one hop — the closure returns a concrete
/// element. A closure that returns another `responsive()` under the same id
/// would otherwise recurse forever; this cap (far above any legitimate depth)
/// turns that authoring mistake into graceful degradation instead of a stack
/// overflow.
const RESPONSIVE_MAX_HOPS: u8 = 16;

/// Expands a [`responsive`](crate::responsive) container query into the concrete
/// element [`build`] should lay out, or `None` when `el` is not a responsive
/// wrapper. The available size comes from this container's own rect last frame
/// (`prev_rects`, recorded for every node by the motion pass) — the hint until a
/// measurement exists, giving the one-frame-deferred convergence. A closure that
/// returns another `responsive()` under the same id is followed up to
/// [`RESPONSIVE_MAX_HOPS`] times, then flattened to a plain box, so the
/// pathological self-wrapping case degrades to empty rather than overflowing.
fn expand_responsive<Msg>(
    el: &Element<Msg>,
    id: WidgetId,
    state: &FrameState,
) -> Option<Element<Msg>> {
    let avail_for = |hint: (f32, f32)| -> (f32, f32) {
        state
            .prev_rects
            .get(&id)
            .map(|rc| {
                #[expect(clippy::cast_possible_truncation, reason = "logical px fit f32")]
                (rc.width() as f32, rc.height() as f32)
            })
            .unwrap_or(hint)
    };
    let r = el.responsive.as_ref()?;
    let mut current = (r.f)(avail_for(r.hint));
    // Follow a chain of self-wrapping responsives, bounded. The borrow of
    // `current.responsive` ends before each reassignment (the hint is copied out
    // first, then the closure call returns an owned element).
    for _ in 0..RESPONSIVE_MAX_HOPS {
        let Some(hint) = current.responsive.as_ref().map(|r| r.hint) else {
            return Some(current);
        };
        current = (current.responsive.as_ref().expect("just matched").f)(avail_for(hint));
    }
    // Cap exceeded: lay the wrapper out as the empty transparent box it is.
    current.responsive = None;
    Some(current)
}

#[expect(
    clippy::too_many_arguments,
    reason = "internal recursion carries build context"
)]
fn build<Msg>(
    el: &Element<Msg>,
    theme: &Theme,
    fonts: &mut Fonts,
    tree: &mut TaffyTree<MeasureCtx>,
    state: &mut FrameState,
    animating: &mut bool,
    id: WidgetId,
    in_stack: bool,
    // The parent grid's resolved name tables, for `grid_area` / named-line
    // placement; `None` when the parent is not a named grid.
    parent_grid: Option<&grid::ResolvedGrid>,
    path: &mut Vec<usize>,
    pending: &mut Vec<PendingOverlay>,
    // Canvas height: the materialization viewport for virtual lists.
    viewport: f32,
) -> BuiltNode {
    // Container query: a `responsive(..)` wrapper is transparent — expand it to
    // the element its closure builds from this container's own size last frame,
    // built under the SAME `id` so next frame `prev_rects[id]` is the generated
    // container's rect (closing the loop). One frame deferred: the first frame
    // has no record and uses the hint, then converges. See `expand_responsive`.
    if let Some(generated) = expand_responsive(el, id, state) {
        return build(
            &generated,
            theme,
            fonts,
            tree,
            state,
            animating,
            id,
            in_stack,
            parent_grid,
            path,
            pending,
            viewport,
        );
    }
    if el.autofocus && !el.disabled {
        // Focus when newly appearing (absent last frame or a different
        // element), without a keyboard focus ring.
        let newly = match state.autofocus_last {
            Some((prev, seen)) => prev != id || seen + 1 < state.frame_no,
            None => true,
        };
        state.autofocus_last = Some((id, state.frame_no));
        if newly {
            state.focus = Some(id);
            state.focus_visible = false;
        }
    }
    let (mut style, anim) = resolve(el, theme, state, id);
    *animating |= anim;
    // Resolve any `ch`-based reading measure now that font metrics are
    // available: 1ch is the advance of `'0'` in this element's own resolved
    // text style. This mutates the stored style, so every later `to_taffy`
    // (root override, overlay layout) sees only `Px`. The `'0'` shaping cost
    // is paid only by ch-using elements.
    if style.has_ch() {
        let ch = fonts.ch_width(&resolve_text(&style.text, theme));
        style.resolve_ch(ch);
    }
    // Named grid placement: resolve this element's `grid_area` / named-line
    // placement against its parent grid into numeric lines for taffy. A no-op
    // when the parent names nothing or this element places numerically.
    if let Some(pg) = parent_grid {
        let (col, row) = grid::place(&style, pg);
        style.grid_column = col;
        style.grid_row = row;
    }
    // `grid-template-areas` without explicit tracks implies a grid of `auto`
    // tracks matching the area shape (CSS implicit grid).
    if !style.grid_template_areas.is_empty() {
        let (rows, cols) = grid::area_dims(&style.grid_template_areas);
        if style.grid_template_columns.is_empty() && cols > 0 {
            style.grid_template_columns = vec![GridTemplate::Single(Track::Auto); cols];
        }
        if style.grid_template_rows.is_empty() && rows > 0 {
            style.grid_template_rows = vec![GridTemplate::Single(Track::Auto); rows];
        }
    }
    // This element's own resolved grid, shared by its children for placement.
    let my_grid = grid::resolve(&style);
    // Virtual containers swap their declared children for the materialized
    // window. Overlays inside virtual rows are unsupported (the overlay
    // path machinery indexes the declared tree).
    let generated: Vec<Element<Msg>>;
    let child_slice: &[Element<Msg>] = match &el.virtual_rows {
        Some(v) => {
            generated = expand_virtual(v, id, state, viewport);
            &generated
        }
        None => &el.children,
    };
    let children: Vec<BuiltNode> = child_slice
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let child_id = id.child(i, c.key.as_deref());
            if let Some(def) = c.overlay {
                // Overlay children leave normal flow entirely; they are
                // built separately once openness is known.
                let mut overlay_path = path.clone();
                overlay_path.push(i);
                pending.push(PendingOverlay {
                    anchor: id,
                    id: child_id,
                    def,
                    path: overlay_path,
                });
                return None;
            }
            path.push(i);
            let node = build(
                c,
                theme,
                fonts,
                tree,
                state,
                animating,
                child_id,
                el.stack,
                my_grid.as_ref(),
                path,
                pending,
                viewport,
            );
            path.pop();
            Some(node)
        })
        .collect();
    let taffy_style = layout::to_taffy(&style, in_stack);
    let (taffy, kind) = match &el.kind {
        Kind::Text(content) => {
            let resolved = resolve_text(&style.text, theme);
            let ctx = MeasureCtx::Text {
                text: content.clone(),
                style: resolved,
            };
            (
                tree.new_leaf_with_context(taffy_style, ctx)
                    .expect("taffy new_leaf_with_context"),
                PaintKind::Text {
                    text: content.clone(),
                    style: resolved,
                },
            )
        }
        Kind::Rich(spans) => {
            let resolved = resolve_text(&style.text, theme);
            let ctx = MeasureCtx::Rich {
                spans: spans.clone(),
                style: resolved,
            };
            (
                tree.new_leaf_with_context(taffy_style, ctx)
                    .expect("taffy new_leaf_with_context"),
                PaintKind::Rich {
                    spans: spans.clone(),
                    style: resolved,
                },
            )
        }
        Kind::Input(data) => {
            let resolved = resolve_text(&style.text, theme);
            // Sync the retained editor with the app-provided value.
            let now = state.now();
            let frame_no = state.frame_no;
            let editor = state
                .editors
                .entry(id)
                .or_insert_with(|| EditorState::new(&resolved, now, data.multiline));
            editor.sync(&data.value, &resolved);
            editor.multiline = data.multiline;
            editor.seen = frame_no;
            let focused = state.focused() == Some(id);
            if focused && !state.reduced_motion {
                // Caret blink needs repaints while focused.
                *animating = true;
            }
            (
                tree.new_leaf_with_context(
                    taffy_style,
                    MeasureCtx::Input {
                        text: data.value.clone(),
                        style: resolved,
                        multiline: data.multiline,
                    },
                )
                .expect("taffy new_leaf_with_context"),
                PaintKind::Input(InputPaint {
                    placeholder: data.placeholder.clone(),
                    style: resolved,
                    placeholder_color: theme.text_subtle,
                    caret_color: theme.accent,
                    selection_color: theme.accent.with_alpha(0.25),
                    focused,
                    pad_x: f64::from(style.padding.left),
                    pad_y: f64::from(style.padding.top),
                    multiline: data.multiline,
                }),
            )
        }
        Kind::Path(data) => (
            tree.new_leaf(taffy_style).expect("taffy new_leaf"),
            PaintKind::Path(data.clone()),
        ),
        Kind::Image(data) => (
            tree.new_leaf(taffy_style).expect("taffy new_leaf"),
            PaintKind::Image(data.clone()),
        ),
        Kind::Box | Kind::Divider => {
            let node = if children.is_empty() {
                tree.new_leaf(taffy_style).expect("taffy new_leaf")
            } else {
                let ids: Vec<NodeId> = children.iter().map(|c| c.taffy).collect();
                tree.new_with_children(taffy_style, &ids)
                    .expect("taffy new_with_children")
            };
            (node, PaintKind::Box)
        }
    };
    if el.spin.is_some() && !state.reduced_motion {
        // Spinners rotate continuously.
        *animating = true;
    }
    // Accessibility projection: explicit semantics win; text, image, and
    // input leaves project automatically.
    let semantics = el.semantics.or(match &el.kind {
        Kind::Text(_) | Kind::Rich(_) => Some(Semantics::Label),
        Kind::Image(_) => Some(Semantics::Image),
        Kind::Input(data) => Some(Semantics::TextInput {
            multiline: data.multiline,
        }),
        Kind::Box | Kind::Divider | Kind::Path(_) => None,
    });
    let label = el.label.clone().or(match &el.kind {
        Kind::Text(content) => Some(content.clone()),
        Kind::Rich(spans) => Some(spans.iter().map(|s| s.text.as_str()).collect()),
        _ => None,
    });
    let value = el.access_value.clone().or_else(|| match &el.kind {
        Kind::Input(data) => Some(data.value.clone()),
        _ => None,
    });

    BuiltNode {
        taffy,
        id,
        kind,
        style,
        focusable: el.focusable,
        disabled: el.disabled,
        invalid: el.invalid,
        spin: el.spin,
        stick_bottom: el.stick_bottom,
        access: (semantics, label, value, el.key.clone()),
        live: el.live,
        selection: match &el.kind {
            Kind::Input(_) => state.editors.get(&id).map(|editor| {
                let range = editor.editor.raw_selection().text_range();
                (range.start, range.end)
            }),
            Kind::Text(_) | Kind::Rich(_) if el.selectable => state
                .static_sel
                .filter(|(sid, ..)| *sid == id)
                .map(|(_, sel, _)| {
                    let range = sel.text_range();
                    (range.start, range.end)
                }),
            _ => None,
        },
        animate_layout: el.animate_layout,
        exit: el.exit,
        source: el.source,
        children,
    }
}

/// The materialized index window for a virtual list: the rows overlapping
/// `offset..offset+viewport`, padded by a fixed overscan. Shared by the
/// frame build and event dispatch so ids always agree.
pub(crate) fn virtual_window(
    count: usize,
    row_height: f32,
    offset: f32,
    viewport: f32,
) -> std::ops::Range<usize> {
    const OVERSCAN: usize = 8;
    if count == 0 || row_height <= 0.0 || row_height.is_nan() || !viewport.is_finite() {
        return 0..0;
    }
    // Clamp to the max scroll before layout's own clamp catches up:
    // a beyond-the-end offset (programmatic scroll_to) must realize
    // the last page, not an empty window for one frame.
    #[expect(clippy::cast_precision_loss, reason = "row counts fit in f32")]
    let max_offset = (count as f32 * row_height - viewport.max(0.0)).max(0.0);
    // max-then-min (not `clamp`) so a NaN offset sanitizes to 0.
    let offset = offset.max(0.0).min(max_offset);
    #[expect(clippy::cast_possible_truncation, reason = "row indices fit in usize")]
    #[expect(clippy::cast_sign_loss, reason = "clamped non-negative above")]
    let first = (offset / row_height).floor() as usize;
    #[expect(clippy::cast_possible_truncation, reason = "row indices fit in usize")]
    #[expect(clippy::cast_sign_loss, reason = "clamped non-negative above")]
    let last = ((offset + viewport.max(0.0)) / row_height).ceil() as usize;
    first.saturating_sub(OVERSCAN).min(count)..last.saturating_add(OVERSCAN).min(count)
}

/// Builds one virtual row with the shared invariants applied: keyed by
/// index (so identity is stable as the window slides) and forced to the
/// declared row height.
pub(crate) fn materialize_virtual_row<Msg>(
    v: &crate::element::VirtualData<Msg>,
    i: usize,
) -> Element<Msg> {
    let mut row = (v.builder)(i);
    if row.key.is_none() {
        row = row.id(&format!("v{i}"));
    }
    // A row sliding out of the materialized window is recycled, not removed —
    // it must never spawn an exit ghost or FLIP-slide as the window shifts.
    row.exit = None;
    row.animate_layout = false;
    row.h(v.row_height).shrink0()
}

/// Expands a virtual container into spacer + visible rows + spacer.
fn expand_virtual<Msg>(
    v: &crate::element::VirtualData<Msg>,
    id: WidgetId,
    state: &mut FrameState,
    viewport: f32,
) -> Vec<Element<Msg>> {
    let offset = state.scroll_offset(id);
    if v.variable {
        const OVERSCAN: usize = 8;
        let frame_no = state.frame_no;
        let index = state
            .virtual_heights
            .entry(id)
            .or_insert_with(|| crate::frame_state::HeightIndex::new_with(v.count, v.row_height));
        index.ensure(v.count, v.row_height);
        // Stamp the container alive this frame so `gc_virtual_heights` keeps it;
        // a container absent next frame is dropped instead of leaking.
        index.mark_seen(frame_no);
        let first = index.index_at(offset).saturating_sub(OVERSCAN);
        let last = (index.index_at(offset + viewport.max(0.0)) + 1 + OVERSCAN).min(v.count);
        let window = first..last;
        let top = index.offset_of(window.start);
        let bottom = (index.total() - index.offset_of(window.end)).max(0.0);
        state.virtual_windows.insert(id, window.clone());
        let mut out = Vec::with_capacity(window.len() + 2);
        out.push(crate::element::div().h(top).w_full().shrink0());
        for i in window {
            // Rows size themselves; estimates only place the spacers.
            let mut row = (v.builder)(i);
            if row.key.is_none() {
                row = row.id(&format!("v{i}"));
            }
            // Recycled rows must not spawn exit ghosts or FLIP (see
            // `materialize_virtual_row`).
            row.exit = None;
            row.animate_layout = false;
            out.push(row.shrink0());
        }
        out.push(crate::element::div().h(bottom).w_full().shrink0());
        return out;
    }
    let window = virtual_window(v.count, v.row_height, offset, viewport);
    let mut out = Vec::with_capacity(window.len() + 2);
    #[expect(clippy::cast_precision_loss, reason = "row counts fit in f32")]
    let top = window.start as f32 * v.row_height;
    #[expect(clippy::cast_precision_loss, reason = "row counts fit in f32")]
    let bottom = (v.count - window.end) as f32 * v.row_height;
    out.push(crate::element::div().h(top).w_full().shrink0());
    for i in window {
        out.push(materialize_virtual_row(v, i));
    }
    out.push(crate::element::div().h(bottom).w_full().shrink0());
    out
}

/// Wrap width for a text leaf given taffy's measure inputs.
fn wrap_width(known: Option<f32>, available: AvailableSpace) -> Option<f32> {
    known
        .or(match available {
            AvailableSpace::Definite(w) => Some(w),
            AvailableSpace::MaxContent => None,
            AvailableSpace::MinContent => Some(0.0),
        })
        // A non-finite width would put parley's line breaker in an
        // inconsistent state (hard assert); measure unbounded instead.
        .filter(|w| w.is_finite())
}

/// The baseline of a child for `items_baseline` rows: true first-line
/// baseline for text, bottom edge for boxes (CSS synthesized baseline).
fn child_baseline(fonts: &mut Fonts, tree: &TaffyTree<MeasureCtx>, node: &BuiltNode) -> f64 {
    let l = tree.layout(node.taffy).expect("taffy layout");
    match &node.kind {
        PaintKind::Text { text, style } => {
            f64::from(fonts.first_baseline(text, style, Some(l.size.width)))
        }
        PaintKind::Rich { spans, style } => {
            f64::from(fonts.first_baseline_rich(spans, style, Some(l.size.width)))
        }
        PaintKind::Box | PaintKind::Path(_) | PaintKind::Input(_) | PaintKind::Image(_) => {
            f64::from(l.size.height)
        }
    }
}

struct Realize<'a> {
    tree: &'a TaffyTree<MeasureCtx>,
    fonts: &'a mut Fonts,
    state: &'a mut FrameState,
    animating: bool,
}

impl Realize<'_> {
    /// Converts a built node into a frame node with absolute rects, applying
    /// baseline shifts, scroll offsets, and clip propagation.
    ///
    /// Threads `all_rects` through the whole tree, recording every node's
    /// absolute rect — the next frame's FLIP/exit measurements. Exit snapshots
    /// are taken separately, *after* the FLIP pass (see [`collect_exits`]), so
    /// a leaving ghost captures the same paint-time translate the live element
    /// last showed.
    fn realize(
        &mut self,
        node: BuiltNode,
        origin: Point,
        visible: Option<Rect>,
        sticky_ctx: Option<StickyCtx>,
        all_rects: &mut std::collections::HashMap<WidgetId, Rect>,
    ) -> FrameNode {
        let l = self.tree.layout(node.taffy).expect("taffy layout");
        let x = origin.x + f64::from(l.location.x);
        let y = origin.y + f64::from(l.location.y);
        let natural = Rect::new(
            x,
            y,
            x + f64::from(l.size.width),
            y + f64::from(l.size.height),
        );
        // `position: sticky` clamps the rect to its scroll viewport, post-layout.
        let is_sticky = node.style.position == Position::Sticky;
        let rect = if is_sticky {
            apply_sticky(natural, &node.style, sticky_ctx)
        } else {
            natural
        };

        // Scroll resolution: clamp the persisted offsets to the content range
        // on whichever axes scroll.
        let scrolls_y = node.style.overflow_y == Overflow::Scroll;
        let scrolls_x = node.style.overflow_x == Overflow::Scroll;
        let scroll = (scrolls_y || scrolls_x).then(|| {
            let max_y = if scrolls_y {
                (l.content_size.height - l.size.height).max(0.0)
            } else {
                0.0
            };
            let max_x = if scrolls_x {
                (l.content_size.width - l.size.width).max(0.0)
            } else {
                0.0
            };
            let (offset_y, offset_x) =
                self.state
                    .clamp_scroll_2d(node.id, max_y, max_x, node.stick_bottom);
            let can_scroll_y = max_y >= MIN_SCROLL_RANGE;
            let can_scroll_x = max_x >= MIN_SCROLL_RANGE;
            let alpha = if can_scroll_y || can_scroll_x {
                self.state.scrollbar_alpha(node.id)
            } else {
                0.0
            };
            self.animating |= self.state.scrollbar_animating(node.id);
            let thumb_v = (alpha > 0.0 && can_scroll_y).then(|| {
                let track_h = rect.height() - 2.0 * SCROLLBAR_INSET;
                let content_h = f64::from(l.content_size.height);
                let thumb_h = (track_h * rect.height() / content_h).max(24.0).min(track_h);
                let denom = f64::from(max_y);
                let t = if denom > 0.0 {
                    f64::from(offset_y) / denom
                } else {
                    0.0
                };
                let thumb_y = rect.y0 + SCROLLBAR_INSET + t * (track_h - thumb_h);
                Rect::new(
                    rect.x1 - SCROLLBAR_INSET - SCROLLBAR_WIDTH,
                    thumb_y,
                    rect.x1 - SCROLLBAR_INSET,
                    thumb_y + thumb_h,
                )
            });
            let thumb_h = (alpha > 0.0 && can_scroll_x).then(|| {
                let track_w = rect.width() - 2.0 * SCROLLBAR_INSET;
                let content_w = f64::from(l.content_size.width);
                let thumb_w = (track_w * rect.width() / content_w).max(24.0).min(track_w);
                let denom = f64::from(max_x);
                let t = if denom > 0.0 {
                    f64::from(offset_x) / denom
                } else {
                    0.0
                };
                let thumb_x = rect.x0 + SCROLLBAR_INSET + t * (track_w - thumb_w);
                Rect::new(
                    thumb_x,
                    rect.y1 - SCROLLBAR_INSET - SCROLLBAR_WIDTH,
                    thumb_x + thumb_w,
                    rect.y1 - SCROLLBAR_INSET,
                )
            });
            ScrollInfo {
                offset_y,
                offset_x,
                thumb_v,
                thumb_h,
                alpha,
                can_scroll_y,
                can_scroll_x,
            }
        });

        // Children visibility: intersect with this node's bounds when clipping.
        let child_visible = if node.style.clip {
            Some(visible.map_or(rect, |v| v.intersect(rect)))
        } else {
            visible
        };
        let scroll_dy = scroll.as_ref().map_or(0.0, |s| f64::from(s.offset_y));
        let scroll_dx = scroll.as_ref().map_or(0.0, |s| f64::from(s.offset_x));
        // Children follow this node's resolved (possibly sticky-clamped) origin.
        let child_origin = Point::new(rect.x0 - scroll_dx, rect.y0 - scroll_dy);
        // A scroll container is the viewport its sticky descendants stick within
        // (the content box, inside padding); otherwise the context passes through.
        let child_sticky_ctx = if scroll.is_some() {
            let pad = &node.style.padding;
            let content = Rect::new(
                rect.x0 + f64::from(pad.left),
                rect.y0 + f64::from(pad.top),
                rect.x1 - f64::from(pad.right),
                rect.y1 - f64::from(pad.bottom),
            );
            Some(StickyCtx { viewport: content })
        } else {
            sticky_ctx
        };

        // Baseline rows: shift in-flow children so first baselines align.
        let baseline_offsets: Option<Vec<f64>> = (node.style.display == Display::Flex
            && node.style.direction == Direction::Row
            && node.style.align_items == AlignItems::Baseline)
            .then(|| {
                let baselines: Vec<f64> = node
                    .children
                    .iter()
                    .map(|c| {
                        if c.style.position == Position::Absolute {
                            0.0
                        } else {
                            child_baseline(self.fonts, self.tree, c)
                        }
                    })
                    .collect();
                let target = baselines.iter().copied().fold(0.0, f64::max);
                baselines
                    .iter()
                    .zip(&node.children)
                    .map(|(b, c)| {
                        if c.style.position == Position::Absolute {
                            0.0
                        } else {
                            target - b
                        }
                    })
                    .collect()
            });

        let virtual_window = self.state.virtual_windows.get(&node.id).cloned();
        let children: Vec<FrameNode> = node
            .children
            .into_iter()
            .enumerate()
            .map(|(i, child)| {
                let dy = baseline_offsets.as_ref().map_or(0.0, |o| o[i]);
                self.realize(
                    child,
                    Point::new(child_origin.x, child_origin.y + dy),
                    child_visible,
                    child_sticky_ctx,
                    all_rects,
                )
            })
            .collect();
        // Variable-height virtual lists: record the materialized rows'
        // real heights (children are spacer + rows + spacer); offsets
        // self-correct on the next frame.
        if let Some(window) = virtual_window
            && let Some(index) = self.state.virtual_heights.get_mut(&node.id)
        {
            for (row, child) in window.zip(children.iter().skip(1)) {
                #[expect(clippy::cast_possible_truncation, reason = "row heights fit in f32")]
                index.record(row, child.rect.height() as f32);
            }
        }

        let meta = NodeMeta {
            focusable: node.focusable && !node.disabled,
            focus_ring: node.focusable
                && !node.disabled
                && self.state.focused() == Some(node.id)
                && self.state.focus_visible,
            invalid: node.invalid,
        };
        let frame_node = FrameNode {
            id: node.id,
            kind: node.kind,
            style: node.style,
            rect,
            visible,
            is_sticky,
            scroll,
            meta,
            spin: node.spin,
            access: node.access,
            live: node.live,
            selection: node.selection,
            animate_layout: node.animate_layout,
            exit: node.exit,
            source: node.source,
            children,
        };
        // Record this node's measured rect for next frame's FLIP / departure
        // detection. Exit snapshots are taken later, after the FLIP pass has
        // adjusted paint-time translate (see `collect_exits`).
        all_rects.insert(frame_node.id, frame_node.rect);
        frame_node
    }
}

/// Snapshots a realized subtree into a clonable, paint-only [`GhostNode`] — the
/// frozen image an exit animation paints while its element is gone. A text
/// input collapses to [`GhostPaint::InputBox`] (its live editor left with it).
fn to_ghost(node: &FrameNode) -> GhostNode {
    let paint = match &node.kind {
        PaintKind::Box => GhostPaint::Box,
        PaintKind::Text { text, style } => GhostPaint::Text {
            text: text.clone(),
            style: *style,
        },
        PaintKind::Rich { spans, style } => GhostPaint::Rich {
            spans: spans.clone(),
            style: *style,
        },
        PaintKind::Path(data) => GhostPaint::Path(data.clone()),
        PaintKind::Image(data) => GhostPaint::Image(data.clone()),
        PaintKind::Input(_) => GhostPaint::InputBox,
    };
    GhostNode {
        rect: node.rect,
        style: node.style.clone(),
        visible: node.visible,
        paint,
        children: node.children.iter().map(to_ghost).collect(),
    }
}

/// Snapshots every exit-tagged node in a realized subtree into `out`. Run for
/// the root and each overlay *after* [`apply_flip`], so a node leaving mid-slide
/// captures the FLIP translate it last painted with — the ghost then animates
/// out from exactly where the element was, not from its settled layout rect.
fn collect_exits(node: &FrameNode, out: &mut Vec<(WidgetId, GhostNode, ExitAnim)>) {
    if let Some(exit) = node.exit {
        out.push((node.id, to_ghost(node), exit));
    }
    for child in &node.children {
        collect_exits(child, out);
    }
}

/// Slides every `animate_layout` node from its previous measured center to the
/// new one (FLIP): when the center moved more than half a pixel, retarget the
/// node's retained spring to start at the position delta and paint it there
/// this frame, so it appears at the old spot and springs to the new. Composes
/// with any existing `translate`. Walks the realized tree in place.
fn apply_flip(
    node: &mut FrameNode,
    state: &mut FrameState,
    now: f64,
    seen: u64,
    animating: &mut bool,
) {
    if node.animate_layout
        && let Some(prev) = state.prev_rects.get(&node.id).copied()
    {
        let prev_c = prev.center();
        let new_c = node.rect.center();
        let dx = prev_c.x - new_c.x;
        let dy = prev_c.y - new_c.y;
        if dx.hypot(dy) > 0.5 {
            #[expect(clippy::cast_possible_truncation, reason = "logical px fit in f32")]
            let (dx, dy) = (dx as f32, dy as f32);
            // Target = the natural resolved style; from = the same, shifted by
            // the delta. Only `translate` differs, so nothing but position
            // animates. Compose the delta onto any existing translate.
            let to = node.style.clone();
            let mut from = to.clone();
            from.translate.0 += dx;
            from.translate.1 += dy;
            state
                .anims
                .entry(node.id)
                .or_insert_with(|| crate::anim::Anim::new(to.clone(), now, seen))
                .inject(from, to, now, seen);
            // Paint at the old position this frame; resolve advances the spring
            // toward zero on subsequent frames.
            node.style.translate.0 += dx;
            node.style.translate.1 += dy;
            *animating = true;
        }
    }
    for child in &mut node.children {
        apply_flip(child, state, now, seen, animating);
    }
}

/// Lays out an element tree into a [`Frame`] at the given logical size and
/// DPI scale. A root with `Auto` width/height is stretched to the canvas.
pub fn build_frame<Msg>(
    root: &Element<Msg>,
    theme: &Theme,
    fonts: &mut Fonts,
    state: &mut FrameState,
    size: (f32, f32),
    scale: f64,
) -> Frame {
    state.virtual_windows.clear();
    // Drop exit animations that finished playing on the previous frame (a
    // settled ghost is painted once more at its final, faded state, then GC'd
    // here). Under reduced motion exits settle on creation, so this clears
    // them the very next frame.
    state.exiting.retain(|_, r| !r.settled);
    let mut tree: TaffyTree<MeasureCtx> = TaffyTree::new();
    state.frame_no += 1;
    let mut transitions_running = false;
    let mut path = Vec::new();
    let mut pending = Vec::new();
    let mut node = build(
        root,
        theme,
        fonts,
        &mut tree,
        state,
        &mut transitions_running,
        WidgetId::ROOT,
        false,
        None,
        &mut path,
        &mut pending,
        size.1,
    );
    if root.style.width == crate::style::Length::Auto {
        node.style.width = crate::style::Length::Px(size.0);
    }
    if root.style.height == crate::style::Length::Auto {
        node.style.height = crate::style::Length::Px(size.1);
    }
    tree.set_style(node.taffy, layout::to_taffy(&node.style, false))
        .expect("taffy set_style");
    tree.compute_layout_with_measure(
        node.taffy,
        Size {
            width: AvailableSpace::Definite(size.0),
            height: AvailableSpace::Definite(size.1),
        },
        |known, available, _id, ctx, _style| match ctx {
            Some(MeasureCtx::Text { text, style }) => {
                let (w, h) = fonts.measure(text, style, wrap_width(known.width, available.width));
                Size {
                    width: known.width.unwrap_or(w),
                    height: known.height.unwrap_or(h),
                }
            }
            Some(MeasureCtx::Rich { spans, style }) => {
                let (w, h) =
                    fonts.measure_rich(spans, style, wrap_width(known.width, available.width));
                Size {
                    width: known.width.unwrap_or(w),
                    height: known.height.unwrap_or(h),
                }
            }
            Some(MeasureCtx::Input {
                text,
                style,
                multiline,
            }) => Size {
                width: known.width.unwrap_or(INPUT_DEFAULT_WIDTH),
                height: known.height.unwrap_or_else(|| {
                    measure_input_height(
                        fonts,
                        text,
                        style,
                        *multiline,
                        wrap_width(known.width, available.width),
                    )
                }),
            },
            None => Size::ZERO,
        },
    )
    .expect("taffy compute_layout");

    // Every node's absolute rect this frame, threaded through every realize
    // pass (root and overlays); it becomes next frame's `prev_rects`, the FLIP
    // and departure baseline. Exit ghosts are snapshotted later, after FLIP.
    let mut all_rects: std::collections::HashMap<WidgetId, Rect> = std::collections::HashMap::new();

    let mut realize = Realize {
        tree: &tree,
        fonts,
        state,
        animating: false,
    };
    let mut root_node = realize.realize(node, Point::ORIGIN, None, None, &mut all_rects);
    let mut animating = realize.animating || transitions_running;
    let canvas = Rect::new(0.0, 0.0, f64::from(size.0), f64::from(size.1));

    // ---- overlay passes: openness, layout against the canvas, placement.
    let mut overlay_anchors = std::collections::HashMap::new();
    let mut overlays: Vec<OverlayFrame> = Vec::new();
    let mut queue = pending;
    let mut present: Vec<WidgetId> = Vec::new();
    while !queue.is_empty() {
        let batch = std::mem::take(&mut queue);
        for p in batch {
            present.push(p.id);
            overlay_anchors.insert(p.anchor, (p.id, p.def.mode));
            let open = match p.def.mode {
                OverlayMode::Open => {
                    state.open_overlay(p.id);
                    true
                }
                OverlayMode::Toggle => state.overlay_open(p.id),
                OverlayMode::Hover { delay_ms } => {
                    match state.hovered_for(p.anchor) {
                        Some(t) if t >= f64::from(delay_ms) / 1000.0 => true,
                        Some(_) => {
                            // Waiting out the delay: keep frames coming.
                            animating = true;
                            false
                        }
                        None => false,
                    }
                }
            };
            if !open {
                continue;
            }
            let Some(el) = element_at(root, &p.path) else {
                continue;
            };
            // Anchor rect from the realized main tree or earlier overlays.
            let anchor_rect = rect_in(&root_node, p.anchor)
                .or_else(|| overlays.iter().find_map(|o| rect_in(&o.node, p.anchor)))
                .unwrap_or(canvas);

            let mut opath = Vec::new();
            let mut nested = Vec::new();
            let built = build(
                el,
                theme,
                fonts,
                &mut tree,
                state,
                &mut animating,
                p.id,
                false,
                None,
                &mut opath,
                &mut nested,
                size.1,
            );
            // Nested overlay paths are relative to `el`; rebase onto root.
            for mut q in nested {
                let mut full = p.path.clone();
                full.extend(q.path.iter());
                q.path = full;
                queue.push(q);
            }
            tree.compute_layout_with_measure(
                built.taffy,
                Size {
                    width: AvailableSpace::Definite(size.0),
                    height: AvailableSpace::Definite(size.1),
                },
                |known, available, _id, ctx, _style| match ctx {
                    Some(MeasureCtx::Text { text, style }) => {
                        let (w, h) =
                            fonts.measure(text, style, wrap_width(known.width, available.width));
                        Size {
                            width: known.width.unwrap_or(w),
                            height: known.height.unwrap_or(h),
                        }
                    }
                    Some(MeasureCtx::Rich { spans, style }) => {
                        let (w, h) = fonts.measure_rich(
                            spans,
                            style,
                            wrap_width(known.width, available.width),
                        );
                        Size {
                            width: known.width.unwrap_or(w),
                            height: known.height.unwrap_or(h),
                        }
                    }
                    Some(MeasureCtx::Input {
                        text,
                        style,
                        multiline,
                    }) => Size {
                        width: known.width.unwrap_or(INPUT_DEFAULT_WIDTH),
                        height: known.height.unwrap_or_else(|| {
                            measure_input_height(
                                fonts,
                                text,
                                style,
                                *multiline,
                                wrap_width(known.width, available.width),
                            )
                        }),
                    },
                    None => Size::ZERO,
                },
            )
            .expect("taffy compute_layout (overlay)");
            let measured = tree.layout(built.taffy).expect("overlay layout").size;
            let (w, h) = (f64::from(measured.width), f64::from(measured.height));

            // Enter animation progress.
            let progress = if state.reduced_motion {
                1.0
            } else {
                let opened = state.overlay_opened.get(&p.id).copied().unwrap_or(0.0);
                let t = ((state.now() - opened) / 0.2).clamp(0.0, 1.0);
                #[expect(clippy::cast_possible_truncation, reason = "progress is 0..=1")]
                {
                    crate::tokens::EASE_STANDARD.eval(t as f32)
                }
            };
            if progress < 1.0 {
                animating = true;
            }

            let state_pointer = state.pointer;
            let origin = match p.def.placement {
                OverlayPlacement::Below { gap } => {
                    let gap = f64::from(gap);
                    let y = if anchor_rect.y1 + gap + h <= canvas.y1
                        || anchor_rect.y0 - gap - h < canvas.y0
                    {
                        anchor_rect.y1 + gap
                    } else {
                        anchor_rect.y0 - gap - h
                    };
                    // Rise up 8px as the menu materializes (it fades in too).
                    let dy = 8.0 * (1.0 - f64::from(progress));
                    Point::new(
                        anchor_rect.x0.clamp(canvas.x0, (canvas.x1 - w).max(0.0)),
                        y + dy,
                    )
                }
                OverlayPlacement::BelowCenter { gap } => {
                    let gap = f64::from(gap);
                    let x = anchor_rect.x0 + (anchor_rect.width() - w) * 0.5;
                    // Flip above when there's no room below (tooltips at
                    // the bottom edge) but room above exists.
                    let y = if anchor_rect.y1 + gap + h <= canvas.y1
                        || anchor_rect.y0 - gap - h < canvas.y0
                    {
                        anchor_rect.y1 + gap
                    } else {
                        anchor_rect.y0 - gap - h
                    };
                    // Tooltips rise the same 8px as they fade in.
                    let dy = 8.0 * (1.0 - f64::from(progress));
                    Point::new(x.clamp(canvas.x0, (canvas.x1 - w).max(0.0)), y + dy)
                }
                OverlayPlacement::TopRight { margin } => {
                    let m = f64::from(margin);
                    Point::new((canvas.x1 - w - m).max(canvas.x0), canvas.y0 + m)
                }
                OverlayPlacement::Pointer { gap } => {
                    // Pin where the pointer was when the overlay opened.
                    let fallback = (
                        #[expect(clippy::cast_possible_truncation, reason = "logical px")]
                        {
                            anchor_rect.x0 as f32
                        },
                        #[expect(clippy::cast_possible_truncation, reason = "logical px")]
                        {
                            anchor_rect.y1 as f32
                        },
                    );
                    let (px, py) = *state
                        .pointer_pins
                        .entry(p.id)
                        .or_insert_with(|| state_pointer.unwrap_or(fallback));
                    let g = f64::from(gap);
                    // Context menus rise the same 8px as they fade in.
                    let dy = 8.0 * (1.0 - f64::from(progress));
                    Point::new(
                        (f64::from(px) + g).clamp(canvas.x0, (canvas.x1 - w).max(canvas.x0)),
                        (f64::from(py) + g + dy).clamp(canvas.y0, (canvas.y1 - h).max(canvas.y0)),
                    )
                }
                OverlayPlacement::Center => {
                    // Slide up 8px as the modal enters.
                    let dy = 8.0 * (1.0 - f64::from(progress));
                    Point::new(
                        canvas.x0 + (canvas.width() - w) * 0.5,
                        canvas.y0 + (canvas.height() - h) * 0.5 + dy,
                    )
                }
                OverlayPlacement::Edge { side } => {
                    // Slide in from off-canvas: fully off the edge at progress 0,
                    // flush at progress 1.
                    let hidden = 1.0 - f64::from(progress);
                    match side {
                        DrawerSide::Left => Point::new(canvas.x0 - w * hidden, canvas.y0),
                        DrawerSide::Right => Point::new(canvas.x1 - w + w * hidden, canvas.y0),
                        DrawerSide::Top => Point::new(canvas.x0, canvas.y0 - h * hidden),
                        DrawerSide::Bottom => Point::new(canvas.x0, canvas.y1 - h + h * hidden),
                    }
                }
                OverlayPlacement::RightStart { gap } => {
                    let gap = f64::from(gap);
                    // To the right of the anchor, flipping to its left when the
                    // flyout would overrun the canvas (and there is room left).
                    let x = if anchor_rect.x1 + gap + w <= canvas.x1
                        || anchor_rect.x0 - gap - w < canvas.x0
                    {
                        anchor_rect.x1 + gap
                    } else {
                        anchor_rect.x0 - gap - w
                    };
                    // Submenu flyouts rise the same 8px as they fade in.
                    let dy = 8.0 * (1.0 - f64::from(progress));
                    let y = anchor_rect
                        .y0
                        .clamp(canvas.y0, (canvas.y1 - h).max(canvas.y0));
                    Point::new(x, y + dy)
                }
            };

            let mut orealize = Realize {
                tree: &tree,
                fonts,
                state,
                animating: false,
            };
            let onode = orealize.realize(built, origin, None, None, &mut all_rects);
            animating |= orealize.animating;

            overlays.push(OverlayFrame {
                id: p.id,
                mode: p.def.mode,
                node: onode,
                progress,
                backdrop: p.def.backdrop,
                trap_focus: p.def.trap_focus,
                hittable: !matches!(p.def.mode, OverlayMode::Hover { .. }),
            });
        }
    }
    // Drop stale stack entries for overlays no longer in the tree.
    let stale: Vec<WidgetId> = state
        .overlays
        .iter()
        .copied()
        .filter(|id| !present.contains(id))
        .collect();
    for id in stale {
        state.close_overlay(id);
    }
    // Stack order: state.overlays is bottom-to-top; sort realized overlays.
    overlays.sort_by_key(|o| {
        state
            .overlays
            .iter()
            .position(|id| *id == o.id)
            .unwrap_or(usize::MAX)
    });

    // ---- motion completion: FLIP layout slides, then exit-ghost lifecycle ----
    let now = state.now();
    let seen = state.frame_no;
    // FLIP: slide `animate_layout` nodes from their previous measured center.
    // Skipped under reduced motion — they snap, so headless goldens are inert.
    // Runs before the anim GC below so freshly injected springs survive.
    if !state.reduced_motion {
        apply_flip(&mut root_node, state, now, seen, &mut animating);
        for overlay in &mut overlays {
            apply_flip(&mut overlay.node, state, now, seen, &mut animating);
        }
    }
    // Snapshot exit-tagged nodes now, after FLIP, so a node leaving mid-slide
    // carries the translate it last painted with. (Always run; under reduced
    // motion the FLIP pass was a no-op, so these snapshots are untranslated —
    // and the ghosts they seed settle instantly and never paint anyway.)
    let mut exit_entries: Vec<(WidgetId, GhostNode, ExitAnim)> = Vec::new();
    collect_exits(&root_node, &mut exit_entries);
    for overlay in &overlays {
        collect_exits(&overlay.node, &mut exit_entries);
    }
    // Exit detection: cancel any exit whose id reappeared this frame, then for
    // each exit-tagged node present last frame but absent now, start its exit.
    state.exiting.retain(|id, _| !all_rects.contains_key(id));
    let reduced = state.reduced_motion;
    for (id, (ghost, exit)) in std::mem::take(&mut state.exit_cache) {
        if !all_rects.contains_key(&id) && !state.exiting.contains_key(&id) {
            state.exiting.insert(
                id,
                // Settle instantly under reduced motion: the ghost is never
                // painted, removal is immediate, and goldens are unchanged.
                ExitRecord {
                    ghost,
                    exit,
                    t0: now,
                    settled: reduced,
                },
            );
        }
    }
    // Refresh the cache with this frame's live exit-tagged nodes, and persist
    // every node's rect as next frame's FLIP / departure baseline.
    state.exit_cache = exit_entries
        .into_iter()
        .map(|(id, ghost, exit)| (id, (ghost, exit)))
        .collect();
    state.prev_rects = all_rects;
    // Keep the runner scheduling while any exit is still playing.
    animating |= state.exiting.values().any(|r| !r.settled);

    let frame_no = state.frame_no;
    state.anims.retain(|_, a| a.seen == frame_no);
    state.editors.retain(|_, e| e.seen == frame_no);
    state.gc_scroll(frame_no);
    state.gc_virtual_heights(frame_no);

    // Right-to-left: mirror the realized geometry horizontally as a final pass.
    // All motion math (FLIP deltas, `prev_rects` above) stays in logical,
    // unmirrored space; the mirror preserves widths, so container queries and
    // FLIP magnitudes are unaffected. Paint, hit-testing, and the access tree all
    // read these mirrored rects, so they agree.
    if theme.is_rtl() {
        let w = canvas.x1;
        mirror_rtl(&mut root_node, w);
        for overlay in &mut overlays {
            mirror_rtl(&mut overlay.node, w);
        }
    }

    let frame = Frame {
        root: root_node,
        overlays,
        overlay_anchors,
        canvas,
        scale,
        thumb_color: theme.text_subtle,
        ring_color: theme.accent.with_alpha(FOCUS_RING.alpha),
        ring_color_invalid: theme.danger.solid.with_alpha(FOCUS_RING.alpha),
        selection_color: theme.accent.with_alpha(0.25),
        animating,
    };
    // Every element in a frame must own a unique WidgetId: it keys every
    // FrameState map (scroll/focus/editor/anim/hover). A collision — almost always
    // a non-unique `.id("…")` or a duplicate keyed-list key — makes two elements
    // silently cross-talk all of that retained state. Loud in debug; compiled out
    // of release, where a stale shared id is a latent bug, not a crash.
    #[cfg(debug_assertions)]
    {
        let dup = frame.first_duplicate_id();
        debug_assert!(
            dup.is_none(),
            "duplicate WidgetId {dup:?} within one frame — two elements share an id and \
             will cross-talk retained state (scroll/focus/editor/anim/hover); check for \
             a non-unique .id(\"…\") or a duplicate keyed-list key",
        );
    }
    frame
}

/// Navigates the element tree by child indices.
fn element_at<'a, Msg>(root: &'a Element<Msg>, path: &[usize]) -> Option<&'a Element<Msg>> {
    let mut el = root;
    for &i in path {
        el = el.children.get(i)?;
    }
    Some(el)
}

/// Mirrors a realized subtree horizontally about width `w` (right-to-left): each
/// node's rect and clip flip about the canvas, so a leading element on the left
/// lands on the right and row children reverse order. Recurses; widths are
/// preserved. Scroll offsets are content-relative (already baked into child
/// rects), so they are left as-is.
fn mirror_rtl(node: &mut FrameNode, w: f64) {
    node.rect = Rect::new(
        w - node.rect.x1,
        node.rect.y0,
        w - node.rect.x0,
        node.rect.y1,
    );
    if let Some(v) = node.visible {
        node.visible = Some(Rect::new(w - v.x1, v.y0, w - v.x0, v.y1));
    }
    for child in &mut node.children {
        mirror_rtl(child, w);
    }
}

/// Finds a node's rect within a realized subtree.
fn rect_in(node: &FrameNode, id: WidgetId) -> Option<Rect> {
    if node.id == id {
        return Some(node.rect);
    }
    node.children.iter().find_map(|c| rect_in(c, id))
}

/// Composes the inverse of every `node_transform` from `node` down to `id`,
/// mapping `point` from screen space into `id`'s untransformed layout space.
/// See [`Frame::to_layout_point`].
fn point_in(node: &FrameNode, id: WidgetId, point: Point) -> Option<Point> {
    let point = match node_transform(node) {
        Some(t) if t.determinant().abs() > 1e-12 => t.inverse() * point,
        Some(_) => return None,
        None => point,
    };
    if node.id == id {
        return Some(point);
    }
    node.children.iter().find_map(|c| point_in(c, id, point))
}

/// Convenience: lays out and paints in one call with throwaway state.
pub fn build_scene<Msg>(
    root: &Element<Msg>,
    theme: &Theme,
    fonts: &mut Fonts,
    size: (f32, f32),
) -> Scene {
    let mut state = FrameState::new();
    state.reduced_motion = true;
    let frame = build_frame(root, theme, fonts, &mut state, size, 1.0);
    frame.paint(fonts, &mut state)
}

// ---------------------------------------------------------------- painting

/// The paint-time affine for a node — translate / rotate / skew / scale composed
/// about the node's (untransformed) rect center — or `None` when the transform is
/// identity. This is the single source of truth for that matrix: `paint_node`
/// draws the subtree under it and `walk_hit` inverts it, so the activatable region
/// always matches the painted one ("what you hit-test is exactly what you
/// painted", even under a transform).
fn node_transform(node: &FrameNode) -> Option<kurbo::Affine> {
    let s = &node.style;
    let has_transform = (s.scale - 1.0).abs() > 1e-4
        || s.translate.0.abs() > 1e-4
        || s.translate.1.abs() > 1e-4
        || s.rotate.abs() > 1e-4
        || s.skew.0.abs() > 1e-4
        || s.skew.1.abs() > 1e-4;
    if !has_transform {
        return None;
    }
    let c = node.rect.center();
    // origin = center: T(translate) · T(c) · R · Skew · S · T(-c)
    let mut a = kurbo::Affine::translate((f64::from(s.translate.0), f64::from(s.translate.1)))
        * kurbo::Affine::translate((c.x, c.y));
    if s.rotate.abs() > 1e-4 {
        a *= kurbo::Affine::rotate(f64::from(s.rotate).to_radians());
    }
    if s.skew.0.abs() > 1e-4 || s.skew.1.abs() > 1e-4 {
        a *= kurbo::Affine::new([
            1.0,
            f64::from(s.skew.1).to_radians().tan(),
            f64::from(s.skew.0).to_radians().tan(),
            1.0,
            0.0,
            0.0,
        ]);
    }
    if (s.scale - 1.0).abs() > 1e-4 {
        a *= kurbo::Affine::scale(f64::from(s.scale));
    }
    a *= kurbo::Affine::translate((-c.x, -c.y));
    Some(a)
}

impl Frame {
    /// Paints the frame into a fresh scene (logical coordinates). Needs the
    /// retained state for editor layouts and caret blink phase. This is the
    /// single-pass look: glass renders as its translucent tint and foreground
    /// filters are inert, exactly as before backdrop blur existed.
    pub fn paint(&self, fonts: &mut Fonts, state: &mut FrameState) -> Scene {
        self.paint_with(fonts, state, &mut PaintMode::Full)
    }

    /// The first of the two backdrop-blur passes: a scene with every glass
    /// subtree painted as *nothing* (so the pixels behind each pane survive a
    /// read-back), plus the [`MultiPassSpec`]s describing each region to filter.
    /// When the returned plan is empty the scene is identical to [`paint`](Self::paint)
    /// and is the final image — the shell's fast path.
    pub fn paint_backdrop(
        &self,
        fonts: &mut Fonts,
        state: &mut FrameState,
    ) -> (Scene, Vec<MultiPassSpec>) {
        let mut specs = Vec::new();
        let scene = self.paint_with(fonts, state, &mut PaintMode::Backdrop(&mut specs));
        (scene, specs)
    }

    /// The second backdrop-blur pass: the composited scene, with each filtered
    /// element drawing the image the shell produced for it (`injected`, keyed by
    /// [`WidgetId`]) — a glass pane lays its blurred backdrop under the tint, a
    /// foreground-filtered element draws its filtered content in place. Elements
    /// with no entry paint normally, so this matches [`paint`](Self::paint)
    /// everywhere except the filtered regions.
    pub fn paint_final(
        &self,
        fonts: &mut Fonts,
        state: &mut FrameState,
        injected: &std::collections::HashMap<WidgetId, peniko::ImageData>,
    ) -> Scene {
        self.paint_with(fonts, state, &mut PaintMode::Final(injected))
    }

    /// The frame's device scale factor (logical → physical). The shell uses it
    /// to map logical spec rects onto the physical read-back image.
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// The shared paint walk, with the multi-pass `mode` threaded through it.
    /// The root, overlays, and exit ghosts are painted identically in every
    /// mode; only filtered nodes (glass / `element_filter`) read `mode`, so a
    /// frame with none renders byte-for-byte the same in all three.
    fn paint_with(
        &self,
        fonts: &mut Fonts,
        state: &mut FrameState,
        mode: &mut PaintMode<'_>,
    ) -> Scene {
        // Recomputed below when a focused editor paints its caret.
        state.ime_caret = None;
        let mut scene = Scene::new();
        self.paint_node(&mut scene, fonts, state, &self.root, mode);
        for overlay in &self.overlays {
            if overlay.backdrop {
                let alpha = 0.4 * overlay.progress;
                scene.fill(
                    peniko::Fill::NonZero,
                    kurbo::Affine::IDENTITY,
                    crate::Color::new([0.0, 0.0, 0.0, alpha]),
                    None,
                    &self.canvas,
                );
            }
            let faded = overlay.progress < 1.0;
            if faded {
                scene.push_layer(
                    peniko::Fill::NonZero,
                    peniko::Mix::Normal,
                    overlay.progress,
                    kurbo::Affine::IDENTITY,
                    &self.canvas,
                );
            }
            self.paint_node(&mut scene, fonts, state, &overlay.node, mode);
            if faded {
                scene.pop_layer();
            }
        }
        self.paint_exits(&mut scene, fonts, state);
        scene
    }

    /// Paints every in-flight exit ghost on top of the frame. Each ghost
    /// advances its own spring/ease progress from `now - t0`, marks itself
    /// settled when complete (the next build GCs it), and draws inside an
    /// opacity layer and a scale/translate sub-scene about its center. Settled
    /// records are skipped, so under reduced motion (where exits settle on
    /// creation) nothing is drawn and goldens are byte-identical.
    fn paint_exits(&self, scene: &mut Scene, fonts: &mut Fonts, state: &mut FrameState) {
        let now = state.now();
        for record in state.exiting.values_mut() {
            if record.settled {
                continue;
            }
            let (progress, done) =
                crate::anim::progress_at(record.exit.transition, now - record.t0);
            if done {
                record.settled = true;
            }
            // Ghost visuals clamp at the target (a spring may overshoot in
            // position, but opacity/scale never pass their endpoints).
            let p = progress.clamp(0.0, 1.0);
            let opacity = crate::anim::lerp_f32(1.0, record.exit.opacity_to, p).clamp(0.0, 1.0);
            if opacity <= 0.0 {
                continue;
            }
            let scale = crate::anim::lerp_f32(1.0, record.exit.scale_to, p);
            let tx = f64::from(crate::anim::lerp_f32(0.0, record.exit.translate_to.0, p));
            let ty = f64::from(crate::anim::lerp_f32(0.0, record.exit.translate_to.1, p));
            // Honor the clip the ghost lived within (None = unclipped → canvas).
            let clip = record.ghost.visible.unwrap_or(self.canvas);
            let layered = opacity < 1.0;
            if layered {
                scene.push_layer(
                    peniko::Fill::NonZero,
                    peniko::Mix::Normal,
                    opacity,
                    kurbo::Affine::IDENTITY,
                    &clip,
                );
            }
            let mut sub = Scene::new();
            self.paint_ghost_node(&mut sub, fonts, &record.ghost);
            let c = record.ghost.rect.center();
            let mut a = kurbo::Affine::translate((tx, ty)) * kurbo::Affine::translate((c.x, c.y));
            if (scale - 1.0).abs() > 1e-4 {
                a *= kurbo::Affine::scale(f64::from(scale));
            }
            a *= kurbo::Affine::translate((-c.x, -c.y));
            scene.append(&sub, Some(a));
            if layered {
                scene.pop_layer();
            }
        }
    }

    /// Paints one ghost subtree, replaying its frozen paint transform
    /// (translate / rotate / skew / scale about the element center) exactly as
    /// [`Self::paint_node`] does for a live node — so a ghost that left
    /// mid-FLIP-slide, or carrying a static transform, animates out from where
    /// it last painted rather than snapping to its untransformed layout rect.
    /// The exit animation's own transform composes on top (applied by the
    /// [`Self::paint_exits`] caller).
    fn paint_ghost_node(&self, scene: &mut Scene, fonts: &mut Fonts, node: &GhostNode) {
        if node.style.display == Display::None {
            return;
        }
        let s = &node.style;
        let has_transform = (s.scale - 1.0).abs() > 1e-4
            || s.translate.0.abs() > 1e-4
            || s.translate.1.abs() > 1e-4
            || s.rotate.abs() > 1e-4
            || s.skew.0.abs() > 1e-4
            || s.skew.1.abs() > 1e-4;
        if has_transform {
            let mut sub = Scene::new();
            self.paint_ghost_node_unscaled(&mut sub, fonts, node);
            let c = node.rect.center();
            // origin = center: T(translate) · T(c) · R · Skew · S · T(-c)
            let mut a =
                kurbo::Affine::translate((f64::from(s.translate.0), f64::from(s.translate.1)))
                    * kurbo::Affine::translate((c.x, c.y));
            if s.rotate.abs() > 1e-4 {
                a *= kurbo::Affine::rotate(f64::from(s.rotate).to_radians());
            }
            if s.skew.0.abs() > 1e-4 || s.skew.1.abs() > 1e-4 {
                a *= kurbo::Affine::new([
                    1.0,
                    f64::from(s.skew.1).to_radians().tan(),
                    f64::from(s.skew.0).to_radians().tan(),
                    1.0,
                    0.0,
                    0.0,
                ]);
            }
            if (s.scale - 1.0).abs() > 1e-4 {
                a *= kurbo::Affine::scale(f64::from(s.scale));
            }
            a *= kurbo::Affine::translate((-c.x, -c.y));
            scene.append(&sub, Some(a));
            return;
        }
        self.paint_ghost_node_unscaled(scene, fonts, node);
    }

    /// Paints one ghost subtree without its transform (mirrors
    /// [`Self::paint_node_unscaled`] over a [`GhostNode`]): the box layers, then
    /// the frozen content by kind, then children. No focus ring, scrollbars,
    /// selection, or caret — a ghost is an inert snapshot.
    fn paint_ghost_node_unscaled(&self, scene: &mut Scene, fonts: &mut Fonts, node: &GhostNode) {
        // Exit ghosts are inert snapshots — never glass — so no backdrop image.
        let layers =
            painter::push_box(scene, &node.style, node.rect, self.canvas, self.scale, None);
        match &node.paint {
            GhostPaint::Text { text, style } => {
                fonts.paint(scene, text, style, node.rect, None);
            }
            GhostPaint::Rich { spans, style } => {
                fonts.paint_rich(scene, spans, style, node.rect, None);
            }
            GhostPaint::Path(data) => {
                let color = node.style.text.color.unwrap_or(self.thumb_color);
                painter::draw_path_rotated(
                    scene,
                    data,
                    node.style.path_trim,
                    color,
                    node.rect,
                    0.0,
                );
            }
            GhostPaint::Image(data) => {
                painter::draw_image(
                    scene,
                    &data.image,
                    node.rect,
                    node.style.corner_radius,
                    node.style.corner_smoothing.unwrap_or(0.0),
                );
            }
            GhostPaint::Box | GhostPaint::InputBox => {}
        }
        for child in &node.children {
            self.paint_ghost_node(scene, fonts, child);
        }
        painter::pop_box(scene, layers);
    }

    fn paint_node(
        &self,
        scene: &mut Scene,
        fonts: &mut Fonts,
        state: &mut FrameState,
        node: &FrameNode,
        mode: &mut PaintMode<'_>,
    ) {
        if node.style.display == Display::None {
            return;
        }
        // Paint-time transform (translate / rotate / skew / scale, about the
        // element center): paint the subtree into a child scene, then append it
        // under the node's affine. `node_transform` is the single source of
        // truth — `walk_hit` inverts the same matrix so the activatable region
        // follows the painted one. Press-scale is the common case.
        if let Some(a) = node_transform(node) {
            let mut sub = Scene::new();
            self.paint_node_unscaled(&mut sub, fonts, state, node, mode);
            scene.append(&sub, Some(a));
            return;
        }
        self.paint_node_unscaled(scene, fonts, state, node, mode);
    }

    fn paint_node_unscaled(
        &self,
        scene: &mut Scene,
        fonts: &mut Fonts,
        state: &mut FrameState,
        node: &FrameNode,
        mode: &mut PaintMode<'_>,
    ) {
        // Multi-pass dispatch. In `Full` mode none of this fires, so the walk is
        // byte-identical to the plain single-pass paint.
        if let PaintMode::Backdrop(specs) = mode {
            // Glass: record the blur and paint the subtree as nothing, so the
            // content behind the pane survives in the read-back.
            if let Some(radius) = node.style.backdrop_blur {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "DPI scale × logical blur radius fits in f32"
                )]
                let std_dev = (f64::from(radius) * self.scale) as f32;
                // The pane's uniform corner radius drives the lensing displacement
                // in the shell (its silhouette is uniformly rounded; average the
                // corners so a per-corner radius still yields one bevel radius).
                let cr = node.style.corner_radius;
                let corner = 0.25 * (cr.tl + cr.tr + cr.br + cr.bl);
                specs.push(MultiPassSpec {
                    id: node.id,
                    rect: node.rect,
                    kind: PassKind::BackdropBlur {
                        std_dev,
                        radius: corner,
                    },
                });
                return;
            }
            // Foreground filter: record it, then paint normally so the element's
            // own content lands in the backdrop for the shell to filter.
            if let Some(filter) = node.style.element_filter {
                specs.push(MultiPassSpec {
                    id: node.id,
                    rect: node.rect,
                    kind: PassKind::ElementFilter(filter),
                });
            }
        }
        // Final pass: a foreground-filtered element draws its filtered image in
        // place of its whole content (its box, content, and children are baked
        // into the image already).
        if node.style.element_filter.is_some()
            && let Some(image) = mode.injected(node.id)
        {
            painter::draw_image(
                scene,
                image,
                node.rect,
                node.style.corner_radius,
                node.style.corner_smoothing.unwrap_or(0.0),
            );
            return;
        }
        // A glass pane composites its blurred backdrop under the box in the
        // final pass; every other element (and every other mode) gets `None`.
        let backdrop = if node.style.backdrop_blur.is_some() {
            mode.injected(node.id)
        } else {
            None
        };
        let layers = painter::push_box(
            scene,
            &node.style,
            node.rect,
            self.canvas,
            self.scale,
            backdrop,
        );
        if node.meta.focus_ring {
            let ring = if node.meta.invalid {
                self.ring_color_invalid
            } else {
                self.ring_color
            };
            painter::focus_ring(
                scene,
                node.rect,
                node.style.corner_radius,
                node.style.corner_smoothing.unwrap_or(0.0),
                ring,
            );
        }
        match &node.kind {
            PaintKind::Text { text, style } => {
                let selection = state
                    .static_sel
                    .filter(|(sid, ..)| *sid == node.id)
                    .map(|(_, sel, _)| (sel, self.selection_color));
                fonts.paint(scene, text, style, node.rect, selection);
            }
            PaintKind::Rich { spans, style } => {
                let selection = state
                    .static_sel
                    .filter(|(sid, ..)| *sid == node.id)
                    .map(|(_, sel, _)| (sel, self.selection_color));
                fonts.paint_rich(scene, spans, style, node.rect, selection);
            }
            PaintKind::Path(data) => {
                let color = node.style.text.color.unwrap_or(self.thumb_color);
                let rotation = node
                    .spin
                    .filter(|_| !state.reduced_motion)
                    .map_or(0.0, |p| {
                        let period = f64::from(p.max(1.0)) / 1000.0;
                        (state.now() % period) / period * std::f64::consts::TAU
                    });
                painter::draw_path_rotated(
                    scene,
                    data,
                    node.style.path_trim,
                    color,
                    node.rect,
                    rotation,
                );
            }
            PaintKind::Input(data) => {
                let now = state.now();
                let reduced = state.reduced_motion;
                if let Some(editor) = state.editors.get_mut(&node.id) {
                    let caret =
                        crate::input::paint(scene, fonts, editor, data, node.rect, now, reduced);
                    if caret.is_some() {
                        state.ime_caret = caret;
                    }
                }
            }
            PaintKind::Image(data) => {
                painter::draw_image(
                    scene,
                    &data.image,
                    node.rect,
                    node.style.corner_radius,
                    node.style.corner_smoothing.unwrap_or(0.0),
                );
            }
            PaintKind::Box => {}
        }
        // Non-sticky children first, then sticky children on top.
        for child in node.children.iter().filter(|c| !c.is_sticky) {
            self.paint_node(scene, fonts, state, child, mode);
        }
        for child in node.children.iter().filter(|c| c.is_sticky) {
            self.paint_node(scene, fonts, state, child, mode);
        }
        if let Some(scroll) = &node.scroll {
            let color = self.thumb_color.multiply_alpha(scroll.alpha * 0.6);
            if let Some(thumb) = scroll.thumb_v {
                painter::fill_rounded(scene, thumb, R_FULL, color);
            }
            if let Some(thumb) = scroll.thumb_h {
                painter::fill_rounded(scene, thumb, R_FULL, color);
            }
        }
        painter::pop_box(scene, layers);
    }

    // ------------------------------------------------------------- queries

    /// Canvas height in logical px: the virtual-list materialization
    /// viewport used by event dispatch.
    pub(crate) fn canvas_height(&self) -> f32 {
        #[expect(clippy::cast_possible_truncation, reason = "canvas sizes fit in f32")]
        {
            self.canvas.height() as f32
        }
    }

    /// The accessibility projection of this frame: roles, names, values,
    /// logical rects, and focusability, with open overlays appended after
    /// the root content in paint order. Headless and dependency-free; the
    /// windowed shell maps it to the platform tree via AccessKit. Reported
    /// rects are the painted bounding box (every ancestor's `node_transform`
    /// composed in), not the untransformed layout rect — so bounds-driven AT
    /// tools (magnifiers, explore-by-touch) agree with where the pointer
    /// actually activates the element.
    pub fn access_tree(&self) -> AccessNode {
        fn project(node: &FrameNode, ancestor: kurbo::Affine) -> AccessNode {
            let (semantics, label, value, key) = node.access.clone();
            let this = match node_transform(node) {
                Some(t) => ancestor * t,
                None => ancestor,
            };
            let rect = if this == kurbo::Affine::IDENTITY {
                node.rect
            } else {
                this.transform_rect_bbox(node.rect)
            };
            AccessNode {
                id: node.id,
                semantics,
                label,
                value,
                rect,
                focusable: node.meta.focusable,
                invalid: node.meta.invalid,
                key,
                live: node.live,
                selection: node.selection,
                children: node.children.iter().map(|c| project(c, this)).collect(),
            }
        }
        let mut root = project(&self.root, kurbo::Affine::IDENTITY);
        for overlay in &self.overlays {
            root.children
                .push(project(&overlay.node, kurbo::Affine::IDENTITY));
        }
        root
    }

    /// Per-text-node legibility, measured on the resolved colors and sizes —
    /// the data behind "prove this UI is legible". For every text run it reports
    /// the APCA `Lc` against [`required_lc`](crate::required_lc) and the WCAG 2
    /// ratio against the AA threshold, using the nearest ancestor fill as the
    /// background — a solid fill directly, or for a gradient fill the worst-contrast
    /// point sampled across it (`window_bg` when no ancestor fills). Non-text nodes
    /// are skipped.
    ///
    /// `window_bg` is the color the frame is composited over (the theme
    /// background); the frame does not store it, so the caller supplies it.
    pub fn legibility(&self, window_bg: crate::Color) -> Vec<TextLegibility> {
        /// A node's effective background: its own fill when it has one, else the
        /// inherited field from its ancestors.
        fn field_of(style: &Style, inherited: &BgField) -> BgField {
            match &style.fill {
                Some(Paint::Solid(c)) => BgField::Solid(*c),
                Some(
                    Paint::LinearGradient { stops, .. }
                    | Paint::RadialGradient { stops, .. }
                    | Paint::ConicGradient { stops, .. },
                ) => BgField::Gradient(stops.clone()),
                _ => inherited.clone(),
            }
        }
        fn walk(
            node: &FrameNode,
            inherited: &BgField,
            window_bg: crate::Color,
            out: &mut Vec<TextLegibility>,
        ) {
            let bg = field_of(&node.style, inherited);
            let text_style = match &node.kind {
                PaintKind::Text { text, style } => Some((text.clone(), style)),
                PaintKind::Rich { spans, style } => Some((
                    spans.iter().map(|s| s.text.as_str()).collect::<String>(),
                    style,
                )),
                _ => None,
            };
            if let Some((text, style)) = text_style {
                let fg = style.color;
                // A gradient is not one color: sample it densely and report the
                // worst-contrast point — the honest legibility bound for text
                // anywhere over the field (an interior dead-zone between two stops
                // is caught, not only the declared stops).
                let (bg_color, bg_uniform) = match &bg {
                    BgField::Solid(c) => (*c, true),
                    BgField::Gradient(stops) => {
                        (gradient_worst_bg(stops, fg).unwrap_or(window_bg), false)
                    }
                };
                let lc = crate::apca::lc_abs(fg, bg_color);
                let required_lc = crate::apca::required_lc(style.px, style.weight);
                let wcag2 = crate::apca::wcag2_ratio(fg, bg_color);
                // WCAG large text: >= 24px (18pt), or >= 18.66px (14pt) bold.
                let large = style.px >= 24.0 || (style.px >= 18.66 && style.weight >= 700.0);
                out.push(TextLegibility {
                    text,
                    fg,
                    bg: bg_color,
                    bg_uniform,
                    size_px: style.px,
                    weight: style.weight,
                    lc,
                    required_lc,
                    wcag2,
                    passes_apca: lc >= required_lc,
                    passes_wcag2: wcag2 >= if large { 3.0 } else { 4.5 },
                    rect: node.rect,
                });
            }
            for child in &node.children {
                walk(child, &bg, window_bg, out);
            }
        }
        let mut out = Vec::new();
        let root = BgField::Solid(window_bg);
        walk(&self.root, &root, window_bg, &mut out);
        for overlay in &self.overlays {
            walk(&overlay.node, &root, window_bg, &mut out);
        }
        out
    }

    /// A human- and agent-readable dump of the built frame: one line per
    /// node — kind, `#key`, layout rect, flags (scroll/focusable/
    /// disabled), semantics with label and value, and the builder call
    /// site (`src=file:line`, captured via `#[track_caller]`). Open
    /// overlays follow the root, marked `overlay`. The headless
    /// equivalent of a visual-tree inspector; grep it.
    pub fn debug_tree(&self) -> String {
        fn fmt_rect(rect: Rect) -> String {
            format!(
                "({:.0},{:.0} {:.0}x{:.0})",
                rect.x0,
                rect.y0,
                rect.width(),
                rect.height()
            )
        }
        fn emit(node: &FrameNode, depth: usize, tag: &str, out: &mut String) {
            let kind = match &node.kind {
                PaintKind::Box => "box",
                PaintKind::Text { .. } => "text",
                PaintKind::Rich { .. } => "richtext",
                PaintKind::Path(_) => "path",
                PaintKind::Input(_) => "input",
                PaintKind::Image(_) => "image",
            };
            out.push_str(&"  ".repeat(depth));
            out.push_str(kind);
            let (semantics, label, value, key) = &node.access;
            if let Some(key) = key {
                out.push_str(&format!(" #{key}"));
            }
            out.push(' ');
            out.push_str(&fmt_rect(node.rect));
            if !tag.is_empty() {
                out.push_str(&format!(" {tag}"));
            }
            if node.scroll.is_some() {
                out.push_str(" scroll");
            }
            if node.meta.focusable {
                out.push_str(" focusable");
            }
            if let Some(semantics) = semantics {
                out.push_str(&format!(" {}", crate::query::role_name(semantics)));
            }
            if let Some(label) = label {
                out.push_str(&format!(" {label:?}"));
            }
            if let Some(value) = value {
                out.push_str(&format!(" value={value:?}"));
            }
            // Normalized separators: dumps read the same on Windows.
            out.push_str(&format!(
                " src={}:{}",
                node.source.file().replace('\\', "/"),
                node.source.line()
            ));
            out.push('\n');
            for child in &node.children {
                emit(child, depth + 1, "", out);
            }
        }
        let mut out = String::new();
        emit(&self.root, 0, "", &mut out);
        for overlay in &self.overlays {
            emit(&overlay.node, 0, "overlay", &mut out);
        }
        out
    }

    /// The text payload of a static text/rich node, with its resolved
    /// style — what static selection shapes against.
    pub(crate) fn static_text_of(&self, id: WidgetId) -> Option<(StaticText<'_>, &ResolvedText)> {
        fn find(node: &FrameNode, id: WidgetId) -> Option<(StaticText<'_>, &ResolvedText)> {
            if node.id == id {
                return match &node.kind {
                    PaintKind::Text { text, style } => Some((StaticText::Plain(text), style)),
                    PaintKind::Rich { spans, style } => Some((StaticText::Rich(spans), style)),
                    _ => None,
                };
            }
            node.children.iter().find_map(|c| find(c, id))
        }
        find(&self.root, id).or_else(|| {
            self.overlays
                .iter()
                .find_map(|overlay| find(&overlay.node, id))
        })
    }

    /// The scrollable that keyboard paging should drive: the nearest
    /// scrollable ancestor of `focus` (itself included), else the first
    /// overflowing scrollable in paint order. Returns its rect too (the
    /// paging viewport).
    pub(crate) fn scroll_target_for(&self, focus: Option<WidgetId>) -> Option<(WidgetId, Rect)> {
        fn path_scrollables(
            node: &FrameNode,
            id: WidgetId,
            out: &mut Vec<(WidgetId, Rect)>,
        ) -> bool {
            let here = node
                .scroll
                .as_ref()
                .is_some_and(|s| s.can_scroll_y)
                .then_some((node.id, node.rect));
            if let Some(h) = here {
                out.push(h);
            }
            if node.id == id {
                return true;
            }
            for child in &node.children {
                if path_scrollables(child, id, out) {
                    return true;
                }
            }
            if here.is_some() {
                out.pop();
            }
            false
        }
        fn first_scrollable(node: &FrameNode) -> Option<(WidgetId, Rect)> {
            if node.scroll.as_ref().is_some_and(|s| s.can_scroll_y) {
                return Some((node.id, node.rect));
            }
            node.children.iter().find_map(first_scrollable)
        }
        if let Some(focus) = focus {
            let mut path = Vec::new();
            let found = path_scrollables(&self.root, focus, &mut path)
                || self.overlays.iter().any(|o| {
                    path.clear();
                    path_scrollables(&o.node, focus, &mut path)
                });
            if found && let Some(last) = path.last() {
                return Some(*last);
            }
        }
        first_scrollable(&self.root)
            .or_else(|| self.overlays.iter().find_map(|o| first_scrollable(&o.node)))
    }

    /// The deepest scrollable container whose visible area contains `point`
    /// and which actually has overflowing content on either axis.
    pub fn scrollable_at(&self, point: Point) -> Option<WidgetId> {
        self.scrollable_axis_at(point, &|s| s.can_scroll_x || s.can_scroll_y)
    }

    /// The deepest container under `point` that scrolls *vertically* — wheel
    /// routing for `dy`.
    pub fn scrollable_y_at(&self, point: Point) -> Option<WidgetId> {
        self.scrollable_axis_at(point, &|s| s.can_scroll_y)
    }

    /// The deepest container under `point` that scrolls *horizontally* — wheel
    /// routing for `dx`. May differ from the vertical scroller (nested panes).
    pub fn scrollable_x_at(&self, point: Point) -> Option<WidgetId> {
        self.scrollable_axis_at(point, &|s| s.can_scroll_x)
    }

    /// The deepest scrollable container under `point` whose `ScrollInfo`
    /// satisfies `can` — so `dx` and `dy` route to the nearest scroller on
    /// *their own* axis, which may be different nodes.
    fn scrollable_axis_at(
        &self,
        point: Point,
        can: &dyn Fn(&ScrollInfo) -> bool,
    ) -> Option<WidgetId> {
        fn walk(
            node: &FrameNode,
            point: Point,
            can: &dyn Fn(&ScrollInfo) -> bool,
        ) -> Option<WidgetId> {
            if node.style.display == Display::None {
                return None;
            }
            // Same transform-aware mapping as `walk_hit`: the ancestor clip
            // (`node.visible`) is in untransformed layout space and must be
            // tested before this node's own transform is undone, so wheel
            // routing agrees with click routing on transformed scrollables.
            if let Some(v) = node.visible
                && !v.contains(point)
            {
                return None;
            }
            let point = match node_transform(node) {
                Some(t) if t.determinant().abs() > 1e-12 => t.inverse() * point,
                Some(_) => return None,
                None => point,
            };
            if !node.rect.contains(point) && node.style.clip {
                return None;
            }
            // Children win over the container (deepest scrollable first);
            // later children paint on top, so walk them in reverse.
            for child in node.children.iter().rev() {
                if let Some(id) = walk(child, point, can) {
                    return Some(id);
                }
            }
            (node.scroll.as_ref().is_some_and(can) && node.rect.contains(point)).then_some(node.id)
        }
        walk(&self.root, point, can)
    }

    /// All elements containing `point` along the topmost branch (later
    /// siblings paint on top and win), ordered root to deepest. Clip-aware:
    /// content scrolled out of a clipped container does not hit.
    pub fn hit_chain(&self, point: Point) -> Vec<WidgetId> {
        // Overlays hit-test first, topmost first; a modal backdrop swallows
        // everything beneath it.
        for overlay in self.overlays.iter().rev() {
            if !overlay.hittable {
                continue;
            }
            let mut chain = Vec::new();
            if Self::walk_hit(&overlay.node, point, &mut chain) {
                return chain;
            }
            if overlay.backdrop {
                return Vec::new();
            }
        }
        let mut chain = Vec::new();
        Self::walk_hit(&self.root, point, &mut chain);
        chain
    }

    fn walk_hit(node: &FrameNode, point: Point, out: &mut Vec<WidgetId>) -> bool {
        if node.style.display == Display::None {
            return false;
        }
        // `node.visible` is the intersection of every ANCESTOR's clip rect,
        // computed by `realize` purely from untransformed layout rects — it
        // has no knowledge of paint-time transforms. Paint matches that: an
        // ancestor's clip layer is pushed in the ancestor's own paint call,
        // wrapping this node's transformed sub-scene from the outside, so it
        // clips in the space *before* this node's own transform is entered.
        // Test it against the incoming point first.
        if let Some(v) = node.visible
            && !v.contains(point)
        {
            return false;
        }
        // Paint applies the node's OWN transform (translate/rotate/skew/scale,
        // about the rect center) to its whole subtree, so map the test point
        // into the node's local space by the inverse of the same
        // `node_transform` matrix: the activatable region then tracks the
        // painted one ("what you hit-test is exactly what you painted"). A
        // singular transform (e.g. scale 0) paints nothing, so it hit-tests as
        // a clean miss. `node.rect` and this node's own `style.clip` (checked
        // just below) are local to this node, so they need the inverted point.
        let point = match node_transform(node) {
            Some(t) if t.determinant().abs() > 1e-12 => t.inverse() * point,
            Some(_) => return false,
            None => point,
        };
        let inside = node.rect.contains(point);
        if node.style.clip && !inside {
            return false;
        }
        let mark = out.len();
        if inside {
            out.push(node.id);
        }
        // Sticky children sit on top, so hit-test them before non-sticky ones.
        for child in node.children.iter().rev().filter(|c| c.is_sticky) {
            if Self::walk_hit(child, point, out) {
                return true;
            }
        }
        for child in node.children.iter().rev().filter(|c| !c.is_sticky) {
            if Self::walk_hit(child, point, out) {
                return true;
            }
        }
        if inside {
            true
        } else {
            out.truncate(mark);
            false
        }
    }

    /// The first `WidgetId` that appears more than once across this frame's
    /// realized tree — the root plus every overlay, which together form the
    /// single-frame id namespace that indexes [`FrameState`]. `None` when every id
    /// is unique (the invariant `build_frame` debug-asserts). Two elements sharing
    /// an id silently share all retained state (scroll/focus/editor/anim/hover),
    /// almost always from a non-unique `.id("…")` or a duplicate keyed-list key.
    pub(crate) fn first_duplicate_id(&self) -> Option<WidgetId> {
        fn walk(
            node: &FrameNode,
            seen: &mut std::collections::HashSet<WidgetId>,
        ) -> Option<WidgetId> {
            if !seen.insert(node.id) {
                return Some(node.id);
            }
            node.children.iter().find_map(|c| walk(c, seen))
        }
        let mut seen = std::collections::HashSet::new();
        walk(&self.root, &mut seen)
            .or_else(|| self.overlays.iter().find_map(|o| walk(&o.node, &mut seen)))
    }

    /// The absolute rect of the element with the given id.
    pub fn rect_of(&self, id: WidgetId) -> Option<Rect> {
        rect_in(&self.root, id).or_else(|| self.overlays.iter().find_map(|o| rect_in(&o.node, id)))
    }

    /// Maps a screen point into the same untransformed layout space
    /// `rect_of` reports its rects in, composing the inverse of every paint-
    /// time transform (`node_transform`) from the root down to `id` — the
    /// same mapping `walk_hit` performs during hit-testing, so callers that
    /// need a point *relative to* an id's rect (caret placement, drag
    /// fractions, text selection) agree with where the pointer actually
    /// activated it. `None` when `id` isn't in this frame, or a transformed
    /// ancestor is singular (paints nothing, so no point maps into it).
    pub(crate) fn to_layout_point(&self, id: WidgetId, point: Point) -> Option<Point> {
        point_in(&self.root, id, point).or_else(|| {
            self.overlays
                .iter()
                .find_map(|o| point_in(&o.node, id, point))
        })
    }

    /// The toggle overlay anchored at `anchor`, if any.
    pub fn toggle_overlay_of(&self, anchor: WidgetId) -> Option<WidgetId> {
        match self.overlay_anchors.get(&anchor) {
            Some((id, OverlayMode::Toggle)) => Some(*id),
            _ => None,
        }
    }

    /// The open overlay whose subtree contains `id`.
    pub fn overlay_containing(&self, id: WidgetId) -> Option<WidgetId> {
        fn contains(node: &FrameNode, id: WidgetId) -> bool {
            node.id == id || node.children.iter().any(|c| contains(c, id))
        }
        self.overlays
            .iter()
            .find(|o| contains(&o.node, id))
            .map(|o| o.id)
    }

    /// Open overlays from top of the stack down: `(id, mode)`.
    pub fn open_overlays_top_down(&self) -> Vec<(WidgetId, OverlayMode)> {
        self.overlays.iter().rev().map(|o| (o.id, o.mode)).collect()
    }

    /// Whether the topmost open overlay paints a backdrop (modal).
    pub fn top_overlay_is_modal(&self) -> Option<WidgetId> {
        self.overlays
            .iter()
            .next_back()
            .filter(|o| o.backdrop)
            .map(|o| o.id)
    }

    /// The pointer position as fractions (0..=1) of the id's rect.
    pub fn fraction_in(&self, id: WidgetId, point: Point) -> Option<(f32, f32)> {
        let rect = self.rect_of(id)?;
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return None;
        }
        let point = self.to_layout_point(id, point)?;
        #[expect(clippy::cast_possible_truncation, reason = "fractions are 0..=1")]
        Some((
            (((point.x - rect.x0) / rect.width()).clamp(0.0, 1.0)) as f32,
            (((point.y - rect.y0) / rect.height()).clamp(0.0, 1.0)) as f32,
        ))
    }

    /// Focusable element ids in tree order (disabled elements excluded).
    /// While a focus-trapping overlay (modal) is open, only its subtree
    /// participates.
    pub fn focusables(&self) -> Vec<WidgetId> {
        fn walk(node: &FrameNode, out: &mut Vec<WidgetId>) {
            if node.style.display == Display::None {
                return;
            }
            if node.meta.focusable {
                out.push(node.id);
            }
            for child in &node.children {
                walk(child, out);
            }
        }
        let mut out = Vec::new();
        if let Some(trap) = self.overlays.iter().rev().find(|o| o.trap_focus) {
            walk(&trap.node, &mut out);
            return out;
        }
        walk(&self.root, &mut out);
        for overlay in &self.overlays {
            if overlay.hittable {
                walk(&overlay.node, &mut out);
            }
        }
        out
    }

    /// `true` if the id resolves to a scrollable with room to scroll.
    pub fn is_scrollable(&self, id: WidgetId) -> bool {
        fn walk(node: &FrameNode, id: WidgetId) -> bool {
            (node.id == id
                && node
                    .scroll
                    .as_ref()
                    .is_some_and(|s| s.can_scroll_x || s.can_scroll_y))
                || node.children.iter().any(|c| walk(c, id))
        }
        walk(&self.root, id)
    }

    // ---------------------------------------------------------------- dump

    /// A serde debug dump of the resolved layout tree: ids, rects, and key
    /// style properties. Locked with insta snapshots in tests.
    pub fn dump(&self) -> String {
        let dump = NodeDump::from_node(&self.root);
        serde_json::to_string_pretty(&dump).expect("layout dump serializes")
    }
}

#[derive(Serialize)]
struct NodeDump {
    id: u64,
    kind: &'static str,
    /// `[x, y, w, h]` in logical px.
    rect: [f32; 4],
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scroll_offset: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scroll_offset_x: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<NodeDump>,
}

impl NodeDump {
    fn from_node(node: &FrameNode) -> Self {
        #[expect(clippy::cast_possible_truncation, reason = "logical px fit in f32")]
        let rect = [
            node.rect.x0 as f32,
            node.rect.y0 as f32,
            node.rect.width() as f32,
            node.rect.height() as f32,
        ];
        Self {
            id: node.id.0,
            kind: match &node.kind {
                PaintKind::Box => "box",
                PaintKind::Text { .. } => "text",
                PaintKind::Rich { .. } => "richtext",
                PaintKind::Path(_) => "path",
                PaintKind::Input(_) => "input",
                PaintKind::Image(_) => "image",
            },
            rect,
            text: match &node.kind {
                PaintKind::Text { text, .. } => Some(text.clone()),
                PaintKind::Rich { spans, .. } => {
                    Some(spans.iter().map(|s| s.text.as_str()).collect())
                }
                PaintKind::Box | PaintKind::Path(_) | PaintKind::Input(_) | PaintKind::Image(_) => {
                    None
                }
            },
            fill: node.style.fill.as_ref().map(|f| match f {
                Paint::Solid(c) => {
                    let c = c.to_rgba8();
                    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
                }
                Paint::LinearGradient { .. } => "linear-gradient".to_owned(),
                Paint::RadialGradient { .. } => "radial-gradient".to_owned(),
                Paint::ConicGradient { .. } => "conic-gradient".to_owned(),
            }),
            scroll_offset: node.scroll.as_ref().map(|s| s.offset_y),
            scroll_offset_x: node
                .scroll
                .as_ref()
                .filter(|s| s.offset_x != 0.0)
                .map(|s| s.offset_x),
            children: node.children.iter().map(Self::from_node).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::STATE_LAYER;

    #[test]
    fn gradient_worst_bg_finds_the_dead_zone_between_stops() {
        use crate::style::GradientStop;
        // Mid-gray text over a raw 2-stop black->white gradient: both endpoint stops
        // contrast strongly with gray, but the field passes through ~gray midway,
        // where contrast collapses. Dense sampling must find that interior point;
        // sampling only the declared stops would miss it (the M1 fix).
        let gray = crate::Color::from_rgba8(128, 128, 128, 255);
        let stops = vec![
            GradientStop {
                offset: 0.0,
                color: crate::Color::from_rgba8(0, 0, 0, 255),
            },
            GradientStop {
                offset: 1.0,
                color: crate::Color::from_rgba8(255, 255, 255, 255),
            },
        ];
        let worst = gradient_worst_bg(&stops, gray).expect("non-empty stops");
        let lc_worst = crate::apca::lc_abs(gray, worst);
        let lc_lo = crate::apca::lc_abs(gray, stops[0].color);
        let lc_hi = crate::apca::lc_abs(gray, stops[1].color);
        assert!(
            lc_worst < lc_lo.min(lc_hi) * 0.5,
            "dense sampling finds the interior dead-zone: worst {lc_worst:.1} should be far \
             below the endpoints ({lc_lo:.1}, {lc_hi:.1})"
        );
        // Empty stops -> None (no panic).
        assert!(gradient_worst_bg(&[], gray).is_none());
    }

    #[test]
    fn state_layer_opacity_picks_the_strongest_state() {
        let id = WidgetId::ROOT;
        let mut s = FrameState::new();
        // Resting: no veil.
        assert_eq!(state_layer_opacity(&s, id, false), None);
        // Hover only.
        s.hovered.insert(id, 0.0);
        assert_eq!(state_layer_opacity(&s, id, false), Some(STATE_LAYER.hover));
        // Keyboard focus outranks hover.
        s.focus = Some(id);
        s.focus_visible = true;
        assert_eq!(state_layer_opacity(&s, id, false), Some(STATE_LAYER.focus));
        // Press matches the focus weight.
        s.active = Some(id);
        assert_eq!(state_layer_opacity(&s, id, false), Some(STATE_LAYER.press));
        // A draggable mid-drag outranks everything.
        s.dragging = Some("payload".to_owned());
        assert_eq!(state_layer_opacity(&s, id, true), Some(STATE_LAYER.drag));
    }

    #[test]
    fn pointer_focus_raises_no_focus_veil() {
        let id = WidgetId::ROOT;
        let mut s = FrameState::new();
        s.focus = Some(id);
        s.focus_visible = false; // focused by pointer, not keyboard
        assert_eq!(state_layer_opacity(&s, id, false), None);
    }

    fn test_frame(root: &crate::element::Element<()>, size: (f32, f32)) -> Frame {
        let theme = Theme::light();
        let mut fonts = Fonts::embedded();
        let mut state = FrameState::new();
        build_frame(root, &theme, &mut fonts, &mut state, size, 1.0)
    }

    /// Two siblings pinned to the same `.id("…")` realize the *same* `WidgetId`
    /// (the key wins over the child index), so they would silently share every
    /// `FrameState` map — scroll, focus, editor, anim. `build_frame` must trip its
    /// debug assert instead of shipping the collision.
    #[test]
    #[should_panic(expected = "duplicate WidgetId")]
    #[cfg_attr(
        not(debug_assertions),
        ignore = "the collision check is a debug_assert!, compiled out in release"
    )]
    fn duplicate_ids_trip_the_debug_assert() {
        use crate::element::div;
        let root = div::<()>().children(vec![
            div::<()>().id("dup").h(10.0),
            div::<()>().id("dup").h(10.0),
        ]);
        let _ = test_frame(&root, (100.0, 100.0));
    }

    /// A tree of distinct ids has no collision.
    #[test]
    fn unique_ids_have_no_duplicate() {
        use crate::element::div;
        let root = div::<()>().children(vec![
            div::<()>().id("a").h(10.0),
            div::<()>().id("b").h(10.0),
            div::<()>().child(div::<()>().id("c").h(10.0)),
        ]);
        let frame = test_frame(&root, (100.0, 100.0));
        assert_eq!(frame.first_duplicate_id(), None);
    }

    /// The walk reports the colliding id directly (built from a unique tree, then
    /// forced to collide so `build_frame`'s assert does not pre-empt the check).
    #[test]
    fn first_duplicate_id_finds_a_collision() {
        use crate::element::div;
        let root = div::<()>().children(vec![
            div::<()>().id("a").h(10.0),
            div::<()>().id("b").h(10.0),
        ]);
        let mut frame = test_frame(&root, (100.0, 100.0));
        let collide = frame.root.children[1].id;
        frame.root.children[0].id = collide;
        assert_eq!(frame.first_duplicate_id(), Some(collide));
    }
}
