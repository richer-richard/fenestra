//! The frame pipeline: element tree -> ids -> style resolution -> taffy
//! layout -> vello scene. Pure: same inputs, same scene.

use kurbo::{Point, Rect};
use taffy::prelude::{NodeId, TaffyTree};
use vello::Scene;

use crate::element::{Element, Kind};
use crate::id::WidgetId;
use crate::layout;
use crate::painter;
use crate::style::{Display, Paint, Style};
use crate::theme::Theme;

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
    #[expect(dead_code, reason = "text painting lands in M2")]
    Text(String),
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
    tree: &mut TaffyTree<()>,
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
    let taffy = if children.is_empty() {
        tree.new_leaf(taffy_style).expect("taffy new_leaf")
    } else {
        let ids: Vec<NodeId> = children.iter().map(|c| c.taffy).collect();
        tree.new_with_children(taffy_style, &ids)
            .expect("taffy new_with_children")
    };
    let kind = match &el.kind {
        Kind::Text(s) => PaintKind::Text(s.clone()),
        Kind::Box | Kind::Divider => PaintKind::Box,
    };
    Node {
        taffy,
        id,
        kind,
        style,
        children,
    }
}

fn paint(scene: &mut Scene, tree: &TaffyTree<()>, node: &Node, origin: Point, canvas: Rect) {
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
    match &node.kind {
        PaintKind::Box => {}
        PaintKind::Text(_) => {
            // Glyph runs land in M2.
        }
    }
    for child in &node.children {
        paint(scene, tree, child, Point::new(x, y), canvas);
    }
    painter::pop_box(scene, layers);
}

/// Renders an element tree to a vello scene at the given logical size.
/// A root with `Auto` width/height is stretched to fill the canvas.
pub fn build_scene<Msg>(root: &Element<Msg>, theme: &Theme, size: (f32, f32)) -> Scene {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let mut node = build(root, theme, &mut tree, WidgetId::ROOT, false);
    if root.style.width == crate::style::Length::Auto {
        node.style.width = crate::style::Length::Px(size.0);
    }
    if root.style.height == crate::style::Length::Auto {
        node.style.height = crate::style::Length::Px(size.1);
    }
    tree.set_style(node.taffy, layout::to_taffy(&node.style, false))
        .expect("taffy set_style");
    layout::compute(&mut tree, node.taffy, size.0, size.1);

    let canvas = Rect::new(0.0, 0.0, f64::from(size.0), f64::from(size.1));
    let mut scene = Scene::new();
    paint(&mut scene, &tree, &node, Point::ORIGIN, canvas);
    scene
}
