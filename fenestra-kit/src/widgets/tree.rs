//! Tree view: nested nodes with disclosure, app-owned expansion and
//! selection (Elm-pure). The whole tree is a single tab stop driven by the
//! keyboard like a WAI-ARIA tree: arrows move the selection, Right/Left
//! expand/collapse (or step in/out), Home/End jump, and typing jumps to a
//! matching node.

use fenestra_core::{
    Cursor, Element, Key, SP1, SP2, Semantics, TextSize, Theme, Transition, col, row, text,
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

/// A nested tree. The app owns the expanded set and the selection: clicking a
/// branch emits `on_toggle(id)`, clicking a leaf emits `on_select(id)`. The
/// tree is one tab stop; arrows then navigate it (see the module docs).
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

    /// Maps a selected node id to a message (fired by click on a leaf, or by
    /// keyboard navigation onto any node).
    pub fn on_select(mut self, f: impl Fn(&str) -> Msg + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(f));
        self
    }
}

/// One row in the flattened visible order, used for keyboard navigation.
#[derive(Clone)]
struct VisNode {
    id: String,
    label: String,
    is_branch: bool,
    is_open: bool,
    depth: usize,
}

/// Flattens the visible nodes (respecting the expanded set) in display order.
fn flatten<Msg>(tree: &TreeView<Msg>, node: &TreeNode, depth: usize, out: &mut Vec<VisNode>) {
    let is_branch = !node.children.is_empty();
    let is_open = tree.expanded.iter().any(|e| e == &node.id);
    out.push(VisNode {
        id: node.id.clone(),
        label: node.label.clone(),
        is_branch,
        is_open,
        depth,
    });
    if is_branch && is_open {
        for child in &node.children {
            flatten(tree, child, depth + 1, out);
        }
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
        .themed(|t: &Theme, s| s.rounded((t.radius.md - 4.0).max(0.0)))
        .shrink0()
        .cursor(Cursor::Pointer)
        .id(&format!("tree-{}", node.id))
        .semantics(Semantics::Button)
        .label(node.label.clone())
        .transition(Transition::colors())
        .state_layer(|t| t.text)
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

    // Click toggles branches and selects leaves; keyboard nav lives on the
    // tree container (one tab stop), so the rows are pointer targets but not
    // individual tab stops (`on_click` auto-focuses, so opt back out).
    if is_branch {
        if let Some(f) = &tree.on_toggle {
            item = item.on_click(f(&node.id)).focusable(false);
        }
    } else if let Some(f) = &tree.on_select {
        item = item.on_click(f(&node.id)).focusable(false);
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
        let body = col()
            .gap(2.0)
            .children(tree.roots.iter().map(|node| render_node(&tree, node, 0.0)));

        let mut vis = Vec::new();
        for root in &tree.roots {
            flatten(&tree, root, 0, &mut vis);
        }
        if vis.is_empty() || (tree.on_select.is_none() && tree.on_toggle.is_none()) {
            return body;
        }

        let sel = tree.selected.clone();
        let on_select = tree.on_select.clone();
        let on_toggle = tree.on_toggle.clone();
        let vis_ta = vis.clone();
        let sel_ta = on_select.clone();

        body.focusable(true)
            .on_key(move |k| {
                let cur = sel
                    .as_deref()
                    .and_then(|s| vis.iter().position(|v| v.id == s));
                match k.key {
                    Key::ArrowDown => {
                        let i = cur.map_or(0, |i| (i + 1).min(vis.len() - 1));
                        on_select.as_ref().map(|f| f(&vis[i].id))
                    }
                    Key::ArrowUp => {
                        let i = cur.map_or(vis.len() - 1, |i| i.saturating_sub(1));
                        on_select.as_ref().map(|f| f(&vis[i].id))
                    }
                    Key::Home => on_select.as_ref().map(|f| f(&vis[0].id)),
                    Key::End => on_select.as_ref().map(|f| f(&vis[vis.len() - 1].id)),
                    Key::ArrowRight => {
                        let i = cur?;
                        let v = &vis[i];
                        if v.is_branch && !v.is_open {
                            on_toggle.as_ref().map(|f| f(&v.id)) // expand
                        } else if v.is_branch && v.is_open && i + 1 < vis.len() {
                            on_select.as_ref().map(|f| f(&vis[i + 1].id)) // into first child
                        } else {
                            None
                        }
                    }
                    Key::ArrowLeft => {
                        let i = cur?;
                        let v = &vis[i];
                        if v.is_branch && v.is_open {
                            on_toggle.as_ref().map(|f| f(&v.id)) // collapse
                        } else {
                            // Step out to the parent (nearest shallower row).
                            let d = v.depth;
                            vis[..i]
                                .iter()
                                .rev()
                                .find(|p| p.depth < d)
                                .and_then(|p| on_select.as_ref().map(|f| f(&p.id)))
                        }
                    }
                    _ => None,
                }
            })
            .on_type_ahead(move |buf| {
                let needle = buf.to_lowercase();
                vis_ta
                    .iter()
                    .find(|v| v.label.to_lowercase().starts_with(&needle))
                    .and_then(|v| sel_ta.as_ref().map(|f| f(&v.id)))
            })
    }
}
