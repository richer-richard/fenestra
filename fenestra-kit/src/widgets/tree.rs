//! Tree view: nested nodes with disclosure, app-owned expansion and
//! selection (Elm-pure).

use fenestra_core::{
    Cursor, Element, Key, R_MD, SP1, SP2, Semantics, TextSize, Theme, Transition, col, row, text,
};

/// One node of a [`tree_view`]. Build nested structures with
/// [`TreeNode::new`] + [`TreeNode::children`].
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Stable id (expansion and selection key off it).
    pub id: String,
    /// Visible label.
    pub label: String,
    /// Child nodes (empty = leaf).
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    /// A node.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children: Vec::new(),
        }
    }

    /// Adds children.
    #[must_use]
    pub fn children(mut self, children: impl IntoIterator<Item = TreeNode>) -> Self {
        self.children.extend(children);
        self
    }
}

/// Shared id-to-message mapping for tree callbacks.
type IdFn<Msg> = std::rc::Rc<dyn Fn(&str) -> Msg>;

/// A tree view under construction; converts into an [`Element`].
pub struct TreeView<Msg> {
    roots: Vec<TreeNode>,
    expanded: Vec<String>,
    selected: Option<String>,
    on_toggle: Option<IdFn<Msg>>,
    on_select: Option<IdFn<Msg>>,
}

/// A nested tree. The app owns the expanded set and the selection:
/// clicking a branch emits `on_toggle(id)`, clicking a leaf emits
/// `on_select(id)`; focused rows also toggle with Left/Right arrows.
pub fn tree_view<Msg>(roots: impl IntoIterator<Item = TreeNode>) -> TreeView<Msg> {
    TreeView {
        roots: roots.into_iter().collect(),
        expanded: Vec::new(),
        selected: None,
        on_toggle: None,
        on_select: None,
    }
}

impl<Msg> TreeView<Msg> {
    /// The ids currently expanded (the app's set).
    #[must_use]
    pub fn expanded(mut self, ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.expanded = ids.into_iter().map(Into::into).collect();
        self
    }

    /// The selected id, highlighted.
    #[must_use]
    pub fn selected(mut self, id: Option<impl Into<String>>) -> Self {
        self.selected = id.map(Into::into);
        self
    }

    /// Maps a toggled branch id to a message.
    pub fn on_toggle(mut self, f: impl Fn(&str) -> Msg + 'static) -> Self {
        self.on_toggle = Some(std::rc::Rc::new(f));
        self
    }

    /// Maps a selected leaf id to a message.
    pub fn on_select(mut self, f: impl Fn(&str) -> Msg + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(f));
        self
    }
}

fn render_node<Msg: Clone + 'static>(
    tree: &TreeView<Msg>,
    node: &TreeNode,
    depth: f32,
) -> Element<Msg> {
    let is_branch = !node.children.is_empty();
    let is_open = tree.expanded.iter().any(|e| e == &node.id);
    let is_selected = tree.selected.as_deref() == Some(node.id.as_str());

    let mut item = row()
        .items_center()
        .gap(SP1)
        .h(28.0)
        .pl(SP2 + depth * 16.0)
        .pr(SP2)
        .rounded(R_MD - 4.0)
        .shrink0()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .id(&format!("tree-{}", node.id))
        .semantics(Semantics::Button)
        .label(node.label.clone())
        .transition(Transition::colors())
        .hover_themed(|t, s| s.bg(t.element))
        .children((
            text(if is_branch {
                if is_open { "▾" } else { "▸" }
            } else {
                " "
            })
            .size(TextSize::Xs),
            text(node.label.clone()).size(TextSize::Sm),
        ));
    if is_selected {
        item = item.themed(|t: &Theme, s| s.bg(t.accent_bg));
    }

    // Click toggles branches and selects leaves; arrows mirror it for
    // the focused row.
    if is_branch {
        if let Some(f) = &tree.on_toggle {
            let id = node.id.clone();
            item = item.on_click(f(&id));
            let f = std::rc::Rc::clone(f);
            let id = node.id.clone();
            item = item.on_key(move |k| {
                let wants = matches!(
                    (k.key, is_open),
                    (Key::ArrowRight, false) | (Key::ArrowLeft, true)
                );
                wants.then(|| f(&id))
            });
        }
    } else if let Some(f) = &tree.on_select {
        item = item.on_click(f(&node.id));
    }

    if is_branch && is_open {
        col().shrink0().child(item).children(
            node.children
                .iter()
                .map(|child| render_node(tree, child, depth + 1.0)),
        )
    } else {
        item
    }
}

impl<Msg: Clone + 'static> From<TreeView<Msg>> for Element<Msg> {
    fn from(tree: TreeView<Msg>) -> Self {
        col()
            .gap(2.0)
            .children(tree.roots.iter().map(|node| render_node(&tree, node, 0.0)))
    }
}
