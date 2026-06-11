//! The frame pipeline: element tree -> ids -> style resolution -> taffy
//! layout (with parley-backed text measurement) -> a [`Frame`] of resolved
//! absolute rects -> vello scene. Pure given `(tree, theme, size, scale)`
//! plus the retained [`FrameState`].

use kurbo::{Point, Rect};
use serde::Serialize;
use taffy::prelude::{AvailableSpace, NodeId, Size, TaffyTree};
use vello::Scene;

use crate::element::{Element, Kind, Overlay, OverlayMode, OverlayPlacement, PathData, Semantics};
use crate::frame_state::FrameState;
use crate::id::WidgetId;
use crate::input::{EditorState, InputPaint};
use crate::layout;
use crate::painter;
use crate::style::{AlignItems, Direction, Display, Overflow, Paint, Position, Style};
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
    Text { text: String, style: ResolvedText },
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
}

/// Scroll geometry of one scrollable container, resolved for this frame.
struct ScrollInfo {
    offset: f32,
    thumb: Option<Rect>,
    alpha: f32,
    /// Content actually overflows; wheel routing skips containers that fit.
    can_scroll: bool,
}

/// One node with its final absolute logical rect.
struct FrameNode {
    id: WidgetId,
    kind: PaintKind,
    style: Style,
    rect: Rect,
    /// Effective clip rect inherited from ancestors (None = unclipped).
    visible: Option<Rect>,
    scroll: Option<ScrollInfo>,
    meta: NodeMeta,
    /// Continuous rotation period (ms) for spinner paths.
    spin: Option<f32>,
    /// Accessibility projection: role/state, name, and value.
    access: (Option<Semantics>, Option<String>, Option<String>),
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
    /// Children in paint order.
    pub children: Vec<AccessNode>,
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
    spin: Option<f32>,
    /// Scroll containers: pin to the bottom while content grows.
    stick_bottom: bool,
    /// Accessibility projection: role/state, name, and value.
    access: (Option<Semantics>, Option<String>, Option<String>),
    children: Vec<BuiltNode>,
}

/// Resolves an element's style against the theme: applies the deferred
/// `themed` styling, overlays interaction variants from state, expands
/// shadow tokens, fills role-based defaults, and advances any transition.
/// Returns the style to paint and whether a transition is still running.
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

    let mut animating = false;
    if let Some(transition) = el.transition
        && !state.reduced_motion
    {
        let now = state.now();
        let seen = state.frame_no;
        let anim = state
            .anims
            .entry(id)
            .or_insert_with(|| crate::anim::Anim::new(style.clone(), now, seen));
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

#[expect(
    clippy::too_many_arguments,
    reason = "internal recursion carries build context"
)]
fn build<Msg>(
    el: &Element<Msg>,
    theme: &Theme,
    tree: &mut TaffyTree<MeasureCtx>,
    state: &mut FrameState,
    animating: &mut bool,
    id: WidgetId,
    in_stack: bool,
    path: &mut Vec<usize>,
    pending: &mut Vec<PendingOverlay>,
    // Canvas height: the materialization viewport for virtual lists.
    viewport: f32,
) -> BuiltNode {
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
    let (style, anim) = resolve(el, theme, state, id);
    *animating |= anim;
    // Virtual containers swap their declared children for the materialized
    // window. Overlays inside virtual rows are unsupported (the overlay
    // path machinery indexes the declared tree).
    let generated: Vec<Element<Msg>>;
    let child_slice: &[Element<Msg>] = match &el.virtual_rows {
        Some(v) => {
            generated = expand_virtual(v, state.scroll_offset(id), viewport);
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
                c, theme, tree, state, animating, child_id, el.stack, path, pending, viewport,
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
        Kind::Text(_) => Some(Semantics::Label),
        Kind::Image(_) => Some(Semantics::Image),
        Kind::Input(data) => Some(Semantics::TextInput {
            multiline: data.multiline,
        }),
        Kind::Box | Kind::Divider | Kind::Path(_) => None,
    });
    let label = el.label.clone().or(match &el.kind {
        Kind::Text(content) => Some(content.clone()),
        _ => None,
    });
    let value = match &el.kind {
        Kind::Input(data) => Some(data.value.clone()),
        _ => None,
    };

    BuiltNode {
        taffy,
        id,
        kind,
        style,
        focusable: el.focusable,
        disabled: el.disabled,
        spin: el.spin,
        stick_bottom: el.stick_bottom,
        access: (semantics, label, value),
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
    let offset = offset.max(0.0);
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
    row.h(v.row_height).shrink0()
}

/// Expands a virtual container into spacer + visible rows + spacer.
fn expand_virtual<Msg>(
    v: &crate::element::VirtualData<Msg>,
    offset: f32,
    viewport: f32,
) -> Vec<Element<Msg>> {
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
    known.or(match available {
        AvailableSpace::Definite(w) => Some(w),
        AvailableSpace::MaxContent => None,
        AvailableSpace::MinContent => Some(0.0),
    })
}

/// The baseline of a child for `items_baseline` rows: true first-line
/// baseline for text, bottom edge for boxes (CSS synthesized baseline).
fn child_baseline(fonts: &mut Fonts, tree: &TaffyTree<MeasureCtx>, node: &BuiltNode) -> f64 {
    let l = tree.layout(node.taffy).expect("taffy layout");
    match &node.kind {
        PaintKind::Text { text, style } => {
            f64::from(fonts.first_baseline(text, style, Some(l.size.width)))
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
    fn realize(&mut self, node: BuiltNode, origin: Point, visible: Option<Rect>) -> FrameNode {
        let l = self.tree.layout(node.taffy).expect("taffy layout");
        let x = origin.x + f64::from(l.location.x);
        let y = origin.y + f64::from(l.location.y);
        let rect = Rect::new(
            x,
            y,
            x + f64::from(l.size.width),
            y + f64::from(l.size.height),
        );

        // Scroll resolution: clamp the persisted offset to the content range.
        let scroll = (node.style.overflow_y == Overflow::Scroll).then(|| {
            let max = (l.content_size.height - l.size.height).max(0.0);
            let offset = if max >= MIN_SCROLL_RANGE {
                self.state.clamp_scroll(node.id, max, node.stick_bottom)
            } else {
                self.state.clamp_scroll(node.id, 0.0, node.stick_bottom)
            };
            let alpha = if max >= MIN_SCROLL_RANGE {
                self.state.scrollbar_alpha(node.id)
            } else {
                0.0
            };
            self.animating |= self.state.scrollbar_animating(node.id);
            let thumb = (alpha > 0.0 && max >= MIN_SCROLL_RANGE).then(|| {
                let track_h = rect.height() - 2.0 * SCROLLBAR_INSET;
                let content_h = f64::from(l.content_size.height);
                let thumb_h = (track_h * rect.height() / content_h).max(24.0).min(track_h);
                let denom = f64::from(max);
                let t = if denom > 0.0 {
                    f64::from(offset) / denom
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
            ScrollInfo {
                offset,
                thumb,
                alpha,
                can_scroll: max >= MIN_SCROLL_RANGE,
            }
        });

        // Children visibility: intersect with this node's bounds when clipping.
        let child_visible = if node.style.clip {
            Some(visible.map_or(rect, |v| v.intersect(rect)))
        } else {
            visible
        };
        let scroll_dy = scroll.as_ref().map_or(0.0, |s| f64::from(s.offset));
        let child_origin = Point::new(x, y - scroll_dy);

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

        let children = node
            .children
            .into_iter()
            .enumerate()
            .map(|(i, child)| {
                let dy = baseline_offsets.as_ref().map_or(0.0, |o| o[i]);
                self.realize(
                    child,
                    Point::new(child_origin.x, child_origin.y + dy),
                    child_visible,
                )
            })
            .collect();

        let meta = NodeMeta {
            focusable: node.focusable && !node.disabled,
            focus_ring: node.focusable
                && !node.disabled
                && self.state.focused() == Some(node.id)
                && self.state.focus_visible,
        };
        FrameNode {
            id: node.id,
            kind: node.kind,
            style: node.style,
            rect,
            visible,
            scroll,
            meta,
            spin: node.spin,
            access: node.access,
            children,
        }
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
    let mut tree: TaffyTree<MeasureCtx> = TaffyTree::new();
    state.frame_no += 1;
    let mut transitions_running = false;
    let mut path = Vec::new();
    let mut pending = Vec::new();
    let mut node = build(
        root,
        theme,
        &mut tree,
        state,
        &mut transitions_running,
        WidgetId::ROOT,
        false,
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

    let mut realize = Realize {
        tree: &tree,
        fonts,
        state,
        animating: false,
    };
    let root_node = realize.realize(node, Point::ORIGIN, None);
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
                &mut tree,
                state,
                &mut animating,
                p.id,
                false,
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
                    Point::new(anchor_rect.x0.clamp(canvas.x0, (canvas.x1 - w).max(0.0)), y)
                }
                OverlayPlacement::BelowCenter { gap } => {
                    let gap = f64::from(gap);
                    let x = anchor_rect.x0 + (anchor_rect.width() - w) * 0.5;
                    Point::new(
                        x.clamp(canvas.x0, (canvas.x1 - w).max(0.0)),
                        anchor_rect.y1 + gap,
                    )
                }
                OverlayPlacement::TopRight { margin } => {
                    let m = f64::from(margin);
                    Point::new((canvas.x1 - w - m).max(canvas.x0), canvas.y0 + m)
                }
                OverlayPlacement::Center => {
                    // Slide up 8px as the modal enters.
                    let dy = 8.0 * (1.0 - f64::from(progress));
                    Point::new(
                        canvas.x0 + (canvas.width() - w) * 0.5,
                        canvas.y0 + (canvas.height() - h) * 0.5 + dy,
                    )
                }
            };

            let mut orealize = Realize {
                tree: &tree,
                fonts,
                state,
                animating: false,
            };
            let onode = orealize.realize(built, origin, None);
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

    let frame_no = state.frame_no;
    state.anims.retain(|_, a| a.seen == frame_no);
    state.editors.retain(|_, e| e.seen == frame_no);
    state.gc_scroll(frame_no);

    Frame {
        root: root_node,
        overlays,
        overlay_anchors,
        canvas,
        scale,
        thumb_color: theme.text_subtle,
        ring_color: theme.accent.with_alpha(FOCUS_RING.alpha),
        animating,
    }
}

/// Navigates the element tree by child indices.
fn element_at<'a, Msg>(root: &'a Element<Msg>, path: &[usize]) -> Option<&'a Element<Msg>> {
    let mut el = root;
    for &i in path {
        el = el.children.get(i)?;
    }
    Some(el)
}

/// Finds a node's rect within a realized subtree.
fn rect_in(node: &FrameNode, id: WidgetId) -> Option<Rect> {
    if node.id == id {
        return Some(node.rect);
    }
    node.children.iter().find_map(|c| rect_in(c, id))
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

impl Frame {
    /// Paints the frame into a fresh scene (logical coordinates). Needs the
    /// retained state for editor layouts and caret blink phase.
    pub fn paint(&self, fonts: &mut Fonts, state: &mut FrameState) -> Scene {
        let mut scene = Scene::new();
        self.paint_node(&mut scene, fonts, state, &self.root);
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
            self.paint_node(&mut scene, fonts, state, &overlay.node);
            if faded {
                scene.pop_layer();
            }
        }
        scene
    }

    fn paint_node(
        &self,
        scene: &mut Scene,
        fonts: &mut Fonts,
        state: &mut FrameState,
        node: &FrameNode,
    ) {
        if node.style.display == Display::None {
            return;
        }
        let layers = painter::push_box(scene, &node.style, node.rect, self.canvas, self.scale);
        if node.meta.focus_ring {
            painter::focus_ring(scene, node.rect, node.style.corner_radius, self.ring_color);
        }
        match &node.kind {
            PaintKind::Text { text, style } => fonts.paint(scene, text, style, node.rect),
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
                    crate::input::paint(scene, fonts, editor, data, node.rect, now, reduced);
                }
            }
            PaintKind::Image(data) => {
                painter::draw_image(scene, &data.image, node.rect, node.style.corner_radius);
            }
            PaintKind::Box => {}
        }
        for child in &node.children {
            self.paint_node(scene, fonts, state, child);
        }
        if let Some(scroll) = &node.scroll
            && let Some(thumb) = scroll.thumb
        {
            let color = self.thumb_color.multiply_alpha(scroll.alpha * 0.6);
            painter::fill_rounded(scene, thumb, R_FULL, color);
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
    /// windowed shell maps it to the platform tree via AccessKit.
    pub fn access_tree(&self) -> AccessNode {
        fn project(node: &FrameNode) -> AccessNode {
            let (semantics, label, value) = node.access.clone();
            AccessNode {
                id: node.id,
                semantics,
                label,
                value,
                rect: node.rect,
                focusable: node.meta.focusable,
                children: node.children.iter().map(project).collect(),
            }
        }
        let mut root = project(&self.root);
        for overlay in &self.overlays {
            root.children.push(project(&overlay.node));
        }
        root
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
                .is_some_and(|s| s.can_scroll)
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
            if node.scroll.as_ref().is_some_and(|s| s.can_scroll) {
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
    /// and which actually has overflowing content.
    pub fn scrollable_at(&self, point: Point) -> Option<WidgetId> {
        fn walk(node: &FrameNode, point: Point) -> Option<WidgetId> {
            if node.style.display == Display::None {
                return None;
            }
            if let Some(v) = node.visible
                && !v.contains(point)
            {
                return None;
            }
            if !node.rect.contains(point) && node.style.clip {
                return None;
            }
            // Children win over the container (deepest scrollable first);
            // later children paint on top, so walk them in reverse.
            for child in node.children.iter().rev() {
                if let Some(id) = walk(child, point) {
                    return Some(id);
                }
            }
            (node.scroll.as_ref().is_some_and(|s| s.can_scroll) && node.rect.contains(point))
                .then_some(node.id)
        }
        walk(&self.root, point)
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
        if let Some(v) = node.visible
            && !v.contains(point)
        {
            return false;
        }
        let inside = node.rect.contains(point);
        if node.style.clip && !inside {
            return false;
        }
        let mark = out.len();
        if inside {
            out.push(node.id);
        }
        for child in node.children.iter().rev() {
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

    /// The absolute rect of the element with the given id.
    pub fn rect_of(&self, id: WidgetId) -> Option<Rect> {
        rect_in(&self.root, id).or_else(|| self.overlays.iter().find_map(|o| rect_in(&o.node, id)))
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
            (node.id == id && node.scroll.as_ref().is_some_and(|s| s.can_scroll))
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
                PaintKind::Path(_) => "path",
                PaintKind::Input(_) => "input",
                PaintKind::Image(_) => "image",
            },
            rect,
            text: match &node.kind {
                PaintKind::Text { text, .. } => Some(text.clone()),
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
            }),
            scroll_offset: node.scroll.as_ref().map(|s| s.offset),
            children: node.children.iter().map(Self::from_node).collect(),
        }
    }
}
