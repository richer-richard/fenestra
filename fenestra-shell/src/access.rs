//! Maps a frame's accessibility projection ([`Frame::access_tree`]) to an
//! AccessKit tree update for the platform adapter.

use accesskit::{Action, Node, NodeId, Role, Toggled, Tree, TreeId, TreeUpdate};
use fenestra_core::{AccessNode, Frame, Semantics, WidgetId};

/// Builds a full tree update for the current frame. `focus` falls back to
/// the root (AccessKit requires a focus target); `scale` maps the logical
/// rects to physical pixels via a root transform.
pub(crate) fn tree_update(frame: &Frame, focus: Option<WidgetId>, scale: f64) -> TreeUpdate {
    let root = frame.access_tree();
    let root_id = NodeId(root.id.0);
    let mut nodes = Vec::new();
    push_node(&mut nodes, &root, true, scale);
    TreeUpdate {
        nodes,
        tree: Some(Tree::new(root_id)),
        tree_id: TreeId::ROOT,
        focus: NodeId(focus.map_or(root.id.0, |f| f.0)),
    }
}

fn push_node(nodes: &mut Vec<(NodeId, Node)>, an: &AccessNode, is_root: bool, scale: f64) {
    let mut node = Node::new(if is_root { Role::Window } else { role_of(an) });
    if is_root && scale != 1.0 {
        node.set_transform(accesskit::Affine::scale(scale));
    }
    node.set_bounds(accesskit::Rect {
        x0: an.rect.x0,
        y0: an.rect.y0,
        x1: an.rect.x1,
        y1: an.rect.y1,
    });
    if let Some(label) = &an.label {
        node.set_label(label.clone());
    }
    if let Some(value) = &an.value {
        node.set_value(value.clone());
    }
    if an.live {
        node.set_live(accesskit::Live::Polite);
    }
    match an.semantics {
        Some(Semantics::Checkbox { checked, mixed }) => node.set_toggled(if mixed {
            accesskit::Toggled::Mixed
        } else {
            toggled(checked)
        }),
        Some(Semantics::Switch { on }) => node.set_toggled(toggled(on)),
        Some(Semantics::Radio { selected }) => node.set_toggled(toggled(selected)),
        Some(Semantics::Tab { selected }) => node.set_selected(selected),
        Some(Semantics::Slider { value, min, max })
        | Some(Semantics::Spinbutton { value, min, max })
        | Some(Semantics::Meter { value, min, max }) => {
            node.set_numeric_value(f64::from(value));
            node.set_min_numeric_value(f64::from(min));
            node.set_max_numeric_value(f64::from(max));
        }
        Some(Semantics::ProgressBar { value: Some(v) }) => {
            node.set_numeric_value(f64::from(v));
            node.set_min_numeric_value(0.0);
            node.set_max_numeric_value(1.0);
        }
        _ => {}
    }
    if an.focusable {
        node.add_action(Action::Focus);
    }
    if matches!(
        an.semantics,
        Some(
            Semantics::Button
                | Semantics::Checkbox { .. }
                | Semantics::Switch { .. }
                | Semantics::Radio { .. }
                | Semantics::Tab { .. }
                | Semantics::ComboBox
        )
    ) {
        node.add_action(Action::Click);
    }
    node.set_children(
        an.children
            .iter()
            .map(|c| NodeId(c.id.0))
            .collect::<Vec<_>>(),
    );
    nodes.push((NodeId(an.id.0), node));
    for child in &an.children {
        push_node(nodes, child, false, scale);
    }
}

fn role_of(an: &AccessNode) -> Role {
    match an.semantics {
        Some(Semantics::Button) => Role::Button,
        Some(Semantics::Checkbox { .. }) => Role::CheckBox,
        Some(Semantics::Switch { .. }) => Role::Switch,
        Some(Semantics::Radio { .. }) => Role::RadioButton,
        Some(Semantics::Slider { .. }) => Role::Slider,
        Some(Semantics::TextInput { multiline: false }) => Role::TextInput,
        Some(Semantics::TextInput { multiline: true }) => Role::MultilineTextInput,
        Some(Semantics::ComboBox) => Role::ComboBox,
        Some(Semantics::Dialog) => Role::Dialog,
        Some(Semantics::Tab { .. }) => Role::Tab,
        Some(Semantics::Alert) => Role::Alert,
        Some(Semantics::Label) => Role::Label,
        Some(Semantics::Image) => Role::Image,
        Some(Semantics::Spinbutton { .. }) => Role::SpinButton,
        Some(Semantics::Meter { .. }) => Role::Meter,
        Some(Semantics::ProgressBar { .. }) => Role::ProgressIndicator,
        None => Role::GenericContainer,
    }
}

fn toggled(on: bool) -> Toggled {
    if on { Toggled::True } else { Toggled::False }
}
