//! The structural verification engine: build a frame from a `Description` (via
//! `fenestra_core::build_frame` — no GPU), then read it the way assistive
//! technology and tests do. The typed access tree, semantic queries, aria
//! snapshots, and the accessibility report all live here; only pixels need the
//! shell, one layer up.

use fenestra_core::{AccessNode, Fonts, Frame, FrameState, Semantics, Theme, build_frame};
use serde::Deserialize;

use crate::dto::{AccessNodeDto, Bounds, QueryResult};
use crate::error::DescribeError;
use crate::format::Description;
use crate::parse::to_element;

/// Builds a frame from a description headlessly: embedded fonts, scale 1.0,
/// reduced motion — the determinism contract. Strict: any parse error (a bad
/// schema tag, an unresolvable color) returns the path-pointed problems so the
/// agent fixes the description first.
///
/// # Errors
/// The accumulated [`DescribeError`]s when the description does not parse cleanly.
pub fn build(
    desc: &Description,
    theme: &Theme,
    size: (u32, u32),
) -> Result<Frame, Vec<DescribeError>> {
    let el = to_element(desc, theme)?;
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;
    #[expect(clippy::cast_precision_loss, reason = "window sizes fit in f32")]
    let logical = (size.0 as f32, size.1 as f32);
    Ok(build_frame(
        &el, theme, &mut fonts, &mut state, logical, 1.0,
    ))
}

/// The typed access tree of a description — the agent's primary, deterministic
/// view of a UI (roles, names, refs, bounds), with no pixels.
///
/// # Errors
/// The parse errors when the description does not parse cleanly.
pub fn access_tree(
    desc: &Description,
    theme: &Theme,
    size: (u32, u32),
) -> Result<AccessNodeDto, Vec<DescribeError>> {
    let frame = build(desc, theme, size)?;
    Ok(node_to_dto(&frame.access_tree(), &[]))
}

/// A semantic selector, mirroring the harness query vocabulary. All set
/// criteria must match (AND). Prefer `role` (+ `name`), then `label`, then
/// `value`; reach for `id` (the stable-key escape hatch) last.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selector {
    /// ARIA role word (`button`, `textbox`, …).
    #[serde(default)]
    pub role: Option<String>,
    /// Accessible name, exact.
    #[serde(default)]
    pub name: Option<String>,
    /// Accessible name, substring.
    #[serde(default)]
    pub name_contains: Option<String>,
    /// Accessible name, exact (alias of `name`).
    #[serde(default)]
    pub label: Option<String>,
    /// Accessible name, substring (alias of `name_contains`).
    #[serde(default)]
    pub label_contains: Option<String>,
    /// Value, exact (text fields).
    #[serde(default)]
    pub value: Option<String>,
    /// Value, substring.
    #[serde(default)]
    pub value_contains: Option<String>,
    /// Stable key / ref.
    #[serde(default)]
    pub id: Option<String>,
}

impl Selector {
    /// True when no criterion is set (matches everything — rejected by `query`).
    fn is_empty(&self) -> bool {
        self.role.is_none()
            && self.name.is_none()
            && self.name_contains.is_none()
            && self.label.is_none()
            && self.label_contains.is_none()
            && self.value.is_none()
            && self.value_contains.is_none()
            && self.id.is_none()
    }

    /// Whether `dto` satisfies every set criterion.
    fn matches(&self, dto: &AccessNodeDto) -> bool {
        if let Some(role) = &self.role
            && &dto.role != role
        {
            return false;
        }
        if let Some(n) = self.name.as_ref().or(self.label.as_ref())
            && dto.name.as_deref() != Some(n.as_str())
        {
            return false;
        }
        if let Some(n) = self.name_contains.as_ref().or(self.label_contains.as_ref())
            && !dto.name.as_deref().is_some_and(|x| x.contains(n.as_str()))
        {
            return false;
        }
        if let Some(v) = &self.value
            && dto.value.as_deref() != Some(v.as_str())
        {
            return false;
        }
        if let Some(v) = &self.value_contains
            && !dto.value.as_deref().is_some_and(|x| x.contains(v.as_str()))
        {
            return false;
        }
        if let Some(id) = &self.id
            && &dto.ref_ != id
        {
            return false;
        }
        true
    }

    /// The accessible name this selector is looking for, lowercased, if any.
    fn wanted_name(&self) -> Option<String> {
        self.name
            .as_ref()
            .or(self.label.as_ref())
            .or(self.name_contains.as_ref())
            .or(self.label_contains.as_ref())
            .map(|s| s.to_lowercase())
    }
}

/// Finds nodes matching the selector. On a miss, returns the few nearest
/// candidates (not the whole tree) so the agent can correct its selector.
///
/// # Errors
/// An empty selector, or the parse errors when the description does not parse.
pub fn query(
    desc: &Description,
    theme: &Theme,
    size: (u32, u32),
    selector: &Selector,
) -> Result<QueryResult, Vec<DescribeError>> {
    if selector.is_empty() {
        return Err(vec![DescribeError::new(
            "selector",
            "empty selector: set role, name, value, or id",
        )]);
    }
    let tree = access_tree(desc, theme, size)?;
    let mut matches = Vec::new();
    collect_matching(&tree, selector, &mut matches);
    let nearest = if matches.is_empty() {
        nearest_candidates(&tree, selector, 5)
    } else {
        Vec::new()
    };
    Ok(QueryResult { matches, nearest })
}

/// Converts a core `AccessNode` to the serializable DTO, deriving a stable
/// `ref`: the node's user key when set, else a structural path like `/0/2/1`.
fn node_to_dto(node: &AccessNode, path: &[usize]) -> AccessNodeDto {
    let ref_ = node.key.clone().unwrap_or_else(|| {
        let joined = path
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join("/");
        format!("/{joined}")
    });
    let role = node
        .semantics
        .as_ref()
        .map_or("generic", Semantics::aria_role)
        .to_string();
    let (checked, selected) = match &node.semantics {
        Some(Semantics::Checkbox { checked }) => (Some(*checked), None),
        Some(Semantics::Switch { on }) => (Some(*on), None),
        Some(Semantics::Radio { selected }) | Some(Semantics::Tab { selected }) => {
            (None, Some(*selected))
        }
        _ => (None, None),
    };
    let children = node
        .children
        .iter()
        .enumerate()
        .map(|(i, child)| {
            let mut p = path.to_vec();
            p.push(i);
            node_to_dto(child, &p)
        })
        .collect();
    AccessNodeDto {
        ref_,
        role,
        name: node.label.clone(),
        value: node.value.clone(),
        checked,
        selected,
        focusable: node.focusable,
        bounds: Bounds {
            x: node.rect.x0,
            y: node.rect.y0,
            w: node.rect.width(),
            h: node.rect.height(),
        },
        children,
    }
}

/// Pushes every node matching `selector` (flattened, children cleared) into `out`.
fn collect_matching(node: &AccessNodeDto, selector: &Selector, out: &mut Vec<AccessNodeDto>) {
    if selector.matches(node) {
        out.push(flat(node));
    }
    for child in &node.children {
        collect_matching(child, selector, out);
    }
}

/// The few nodes most similar to the selector, for a miss. Signal-bearing nodes
/// only (a role or a name), ranked by role match then name overlap.
fn nearest_candidates(tree: &AccessNodeDto, selector: &Selector, k: usize) -> Vec<AccessNodeDto> {
    let mut all = Vec::new();
    flatten(tree, &mut all);
    all.retain(|d| d.role != "generic" || d.name.is_some());
    let want_role = selector.role.as_deref();
    let want_name = selector.wanted_name();
    all.sort_by(|a, b| {
        score(b, want_role, want_name.as_deref()).cmp(&score(a, want_role, want_name.as_deref()))
    });
    all.truncate(k);
    all
}

/// Relevance score of a node to the wanted role/name (higher is closer).
fn score(d: &AccessNodeDto, want_role: Option<&str>, want_name: Option<&str>) -> i32 {
    let mut s = 0;
    if let Some(r) = want_role
        && d.role == r
    {
        s += 2;
    }
    if let Some(n) = want_name
        && d.name
            .as_deref()
            .is_some_and(|x| x.to_lowercase().contains(n))
    {
        s += 1;
    }
    s
}

/// A copy of `node` with its children cleared (a flat list entry).
fn flat(node: &AccessNodeDto) -> AccessNodeDto {
    let mut leaf = node.clone();
    leaf.children = Vec::new();
    leaf
}

/// Flattens the tree into `out` as childless nodes, in paint order.
fn flatten(node: &AccessNodeDto, out: &mut Vec<AccessNodeDto>) {
    out.push(flat(node));
    for child in &node.children {
        flatten(child, out);
    }
}
