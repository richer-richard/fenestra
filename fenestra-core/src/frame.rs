//! The frame pipeline: element tree -> ids -> style resolution -> taffy
//! layout (with parley-backed text measurement) -> vello scene. Pure: same
//! inputs, same scene.

use kurbo::{Point, Rect};
use taffy::prelude::{AvailableSpace, NodeId, Size, TaffyTree};
use vello::Scene;

use crate::element::{Element, Kind};
use crate::id::WidgetId;
use crate::layout;
use crate::painter;
use crate::style::{AlignItems, Direction, Display, Paint, Position, Style};
use crate::text::{Fonts, ResolvedText, resolve_text};
use crate::theme::Theme;

/// Taffy node context for text leaves.
struct TextCtx {
    text: String,
    style: ResolvedText,
}

/// One resolved node: stable id, resolved style, taffy handle, and children.
struct Node {
    taffy: NodeId,
    #[expect(dead_code, reason = "ids drive FrameState lookups from M4 on")]
    id: WidgetId,
    kind: PaintKind,
    style: Style,
    children: Vec<Node>,
}

enum PaintKind {
    Box,
    Text { text: String, style: ResolvedText },
}

/// Resolves an element's style against the theme: expands shadow tokens,
/// fills role-based defaults. Interaction-state overlays arrive in M4.
fn resolve<Msg>(el: &Element<Msg>, theme: &Theme) -> Style {
    let mut style = el.style.clone();
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
    style
}

fn build<Msg>(
    el: &Element<Msg>,
    theme: &Theme,
    tree: &mut TaffyTree<TextCtx>,
    id: WidgetId,
    in_stack: bool,
) -> Node {
    let style = resolve(el, theme);
    let children: Vec<Node> = el
        .children
        .iter()
        .enumerate()
        .map(|(i, c)| build(c, theme, tree, id.child(i, c.key.as_deref()), el.stack))
        .collect();
    let taffy_style = layout::to_taffy(&style, in_stack);
    let (taffy, kind) = match &el.kind {
        Kind::Text(content) => {
            let resolved = resolve_text(&style.text, theme);
            let ctx = TextCtx {
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
    Node {
        taffy,
        id,
        kind,
        style,
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
fn child_baseline(fonts: &mut Fonts, tree: &TaffyTree<TextCtx>, node: &Node) -> f64 {
    let l = tree.layout(node.taffy).expect("taffy layout");
    match &node.kind {
        PaintKind::Text { text, style } => {
            f64::from(fonts.first_baseline(text, style, Some(l.size.width)))
        }
        PaintKind::Box => f64::from(l.size.height),
    }
}

fn paint(
    scene: &mut Scene,
    fonts: &mut Fonts,
    tree: &TaffyTree<TextCtx>,
    node: &Node,
    origin: Point,
    canvas: Rect,
) {
    if node.style.display == Display::None {
        return;
    }
    let l = tree.layout(node.taffy).expect("taffy layout");
    let x = origin.x + f64::from(l.location.x);
    let y = origin.y + f64::from(l.location.y);
    let rect = Rect::new(
        x,
        y,
        x + f64::from(l.size.width),
        y + f64::from(l.size.height),
    );
    let layers = painter::push_box(scene, &node.style, rect, canvas);
    if let PaintKind::Text { text, style } = &node.kind {
        fonts.paint(scene, text, style, rect);
    }

    // Baseline rows: shift each in-flow child down so first baselines align.
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
                        child_baseline(fonts, tree, c)
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

    for (i, child) in node.children.iter().enumerate() {
        let dy = baseline_offsets.as_ref().map_or(0.0, |offsets| offsets[i]);
        paint(scene, fonts, tree, child, Point::new(x, y + dy), canvas);
    }
    painter::pop_box(scene, layers);
}

/// Renders an element tree to a vello scene at the given logical size.
/// A root with `Auto` width/height is stretched to fill the canvas.
pub fn build_scene<Msg>(
    root: &Element<Msg>,
    theme: &Theme,
    fonts: &mut Fonts,
    size: (f32, f32),
) -> Scene {
    let mut tree: TaffyTree<TextCtx> = TaffyTree::new();
    let mut node = build(root, theme, &mut tree, WidgetId::ROOT, false);
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
            Some(ctx) => {
                let (w, h) = fonts.measure(
                    &ctx.text,
                    &ctx.style,
                    wrap_width(known.width, available.width),
                );
                Size {
                    width: known.width.unwrap_or(w),
                    height: known.height.unwrap_or(h),
                }
            }
            None => Size::ZERO,
        },
    )
    .expect("taffy compute_layout");

    let canvas = Rect::new(0.0, 0.0, f64::from(size.0), f64::from(size.1));
    let mut scene = Scene::new();
    paint(&mut scene, fonts, &tree, &node, Point::ORIGIN, canvas);
    scene
}
