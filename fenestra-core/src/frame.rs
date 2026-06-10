//! The frame pipeline: element tree -> ids -> style resolution -> taffy
//! layout (with parley-backed text measurement) -> a [`Frame`] of resolved
//! absolute rects -> vello scene. Pure given `(tree, theme, size, scale)`
//! plus the retained [`FrameState`].

use kurbo::{Point, Rect};
use serde::Serialize;
use taffy::prelude::{AvailableSpace, NodeId, Size, TaffyTree};
use vello::Scene;

use crate::element::{Element, Kind, PathData};
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
    Text { text: String, style: ResolvedText },
    Input { style: ResolvedText },
}

/// Intrinsic width of an unconstrained input, logical px.
const INPUT_DEFAULT_WIDTH: f32 = 220.0;

enum PaintKind {
    Box,
    Text { text: String, style: ResolvedText },
    Path(PathData),
    Input(InputPaint),
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
    children: Vec<FrameNode>,
}

/// A laid-out frame: resolved styles and absolute rects for every element.
/// Paint, input routing, and debug dumps all read from this one structure.
pub struct Frame {
    root: FrameNode,
    canvas: Rect,
    scale: f64,
    thumb_color: crate::Color,
    ring_color: crate::Color,
    /// `true` while any scrollbar fade or style transition is running; the
    /// runner keeps scheduling frames.
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
    (style, animating)
}

fn build<Msg>(
    el: &Element<Msg>,
    theme: &Theme,
    tree: &mut TaffyTree<MeasureCtx>,
    state: &mut FrameState,
    animating: &mut bool,
    id: WidgetId,
    in_stack: bool,
) -> BuiltNode {
    let (style, anim) = resolve(el, theme, state, id);
    *animating |= anim;
    let children: Vec<BuiltNode> = el
        .children
        .iter()
        .enumerate()
        .map(|(i, c)| {
            build(
                c,
                theme,
                tree,
                state,
                animating,
                id.child(i, c.key.as_deref()),
                el.stack,
            )
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
                .or_insert_with(|| EditorState::new(&resolved, now));
            editor.sync(&data.value, &resolved);
            editor.seen = frame_no;
            let focused = state.focused() == Some(id);
            if focused && !state.reduced_motion {
                // Caret blink needs repaints while focused.
                *animating = true;
            }
            (
                tree.new_leaf_with_context(taffy_style, MeasureCtx::Input { style: resolved })
                    .expect("taffy new_leaf_with_context"),
                PaintKind::Input(InputPaint {
                    placeholder: data.placeholder.clone(),
                    style: resolved,
                    placeholder_color: theme.text_subtle,
                    caret_color: theme.accent,
                    selection_color: theme.accent.with_alpha(0.25),
                    focused,
                    pad_x: f64::from(style.padding.left),
                }),
            )
        }
        Kind::Path(data) => (
            tree.new_leaf(taffy_style).expect("taffy new_leaf"),
            PaintKind::Path(data.clone()),
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
    BuiltNode {
        taffy,
        id,
        kind,
        style,
        focusable: el.focusable,
        disabled: el.disabled,
        children,
    }
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
        PaintKind::Box | PaintKind::Path(_) | PaintKind::Input(_) => f64::from(l.size.height),
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
                self.state.clamp_scroll(node.id, max)
            } else {
                self.state.clamp_scroll(node.id, 0.0)
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
    let mut node = build(
        root,
        theme,
        &mut tree,
        state,
        &mut transitions_running,
        WidgetId::ROOT,
        false,
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
            Some(MeasureCtx::Input { style }) => Size {
                width: known.width.unwrap_or(INPUT_DEFAULT_WIDTH),
                height: known
                    .height
                    .unwrap_or_else(|| (style.px * style.line_height).ceil()),
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
    let animating = realize.animating || transitions_running;
    let frame_no = state.frame_no;
    state.anims.retain(|_, a| a.seen == frame_no);
    state.editors.retain(|_, e| e.seen == frame_no);

    Frame {
        root: root_node,
        canvas: Rect::new(0.0, 0.0, f64::from(size.0), f64::from(size.1)),
        scale,
        thumb_color: theme.text_subtle,
        ring_color: theme.accent.with_alpha(FOCUS_RING.alpha),
        animating,
    }
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
                painter::draw_path(scene, data, node.style.path_trim, color, node.rect);
            }
            PaintKind::Input(data) => {
                let now = state.now();
                let reduced = state.reduced_motion;
                if let Some(editor) = state.editors.get_mut(&node.id) {
                    crate::input::paint(scene, fonts, editor, data, node.rect, now, reduced);
                }
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
        fn walk(node: &FrameNode, point: Point, out: &mut Vec<WidgetId>) -> bool {
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
                if walk(child, point, out) {
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
        let mut chain = Vec::new();
        walk(&self.root, point, &mut chain);
        chain
    }

    /// The absolute rect of the element with the given id.
    pub fn rect_of(&self, id: WidgetId) -> Option<Rect> {
        fn find(node: &FrameNode, id: WidgetId) -> Option<Rect> {
            if node.id == id {
                return Some(node.rect);
            }
            node.children.iter().find_map(|c| find(c, id))
        }
        find(&self.root, id)
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
        walk(&self.root, &mut out);
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
            },
            rect,
            text: match &node.kind {
                PaintKind::Text { text, .. } => Some(text.clone()),
                PaintKind::Box | PaintKind::Path(_) | PaintKind::Input(_) => None,
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
