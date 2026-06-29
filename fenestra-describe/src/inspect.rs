//! The structural verification engine: build a frame from a `Description` (via
//! `fenestra_core::build_frame` — no GPU), then read it the way assistive
//! technology and tests do. The typed access tree, semantic queries, aria
//! snapshots, and the accessibility report all live here; only pixels need the
//! shell, one layer up.

use std::collections::HashMap;

use fenestra_core::{
    AccessNode, Color, Fonts, Frame, FrameState, Query, Semantics, Theme, WidgetId, build_frame, by,
};
use regex::Regex;
use serde::Deserialize;

use crate::dto::{
    A11yReport, AccessNodeDto, AriaDiff, Bounds, ContrastDto, LayoutFinding, LayoutReport,
    LegibilityDto, QueryResult,
};
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
    Ok(frame_access_tree(&build(desc, theme, size)?))
}

/// Converts a built frame's access tree to the typed DTO (with stable refs) —
/// the frame-level primitive the cli engine uses to capture a tree after driving
/// interactions on a live harness.
#[must_use]
pub fn frame_access_tree(frame: &Frame) -> AccessNodeDto {
    node_to_dto(&frame.access_tree(), &[])
}

/// The keyboard focus order: the refs a Tab cycle visits, in order, honoring a
/// modal focus trap (disabled controls excluded). The agent-verifiable answer to
/// "what does Tab reach, and in what order" — refs use the same scheme as the
/// access tree (the node's key, else its structural path).
///
/// # Errors
/// The parse errors when the description does not parse cleanly.
pub fn focus_order(
    desc: &Description,
    theme: &Theme,
    size: (u32, u32),
) -> Result<Vec<String>, Vec<DescribeError>> {
    Ok(frame_focus_order(&build(desc, theme, size)?))
}

/// Focus order read from an already-built [`Frame`] — the primitive
/// [`focus_order`] builds on, for a frame the caller already holds (e.g. the
/// post-interaction frame of a driven scenario).
#[must_use]
pub fn frame_focus_order(frame: &Frame) -> Vec<String> {
    fn index(node: &AccessNode, path: &[usize], by_id: &mut HashMap<WidgetId, String>) {
        let ref_ = node.key.clone().unwrap_or_else(|| {
            let joined = path
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join("/");
            format!("/{joined}")
        });
        by_id.insert(node.id, ref_);
        for (i, child) in node.children.iter().enumerate() {
            let mut p = path.to_vec();
            p.push(i);
            index(child, &p, by_id);
        }
    }
    let mut by_id = HashMap::new();
    index(&frame.access_tree(), &[], &mut by_id);
    frame
        .focusables()
        .into_iter()
        .filter_map(|id| by_id.get(&id).cloned())
        .collect()
}

/// The minimum interactive hit-target size in logical px (WCAG 2.5.8 minimum).
const MIN_HIT_TARGET: f64 = 24.0;

/// Layout problems found from frame geometry: interactive targets below the
/// minimum hit size, and signal-bearing nodes that fall outside the window. The
/// "is it big enough to tap / does it fit on screen" check an agent would
/// otherwise need eyes for.
///
/// # Errors
/// The parse errors when the description does not parse cleanly.
pub fn layout_report(
    desc: &Description,
    theme: &Theme,
    size: (u32, u32),
) -> Result<LayoutReport, Vec<DescribeError>> {
    Ok(tree_layout_report(&access_tree(desc, theme, size)?, size))
}

/// Layout problems read from an already-captured access `tree` (e.g. a driven
/// scenario's after-tree) against a window `size` — the primitive [`layout_report`]
/// builds on.
#[must_use]
pub fn tree_layout_report(tree: &AccessNodeDto, size: (u32, u32)) -> LayoutReport {
    fn finding(node: &AccessNodeDto, detail: String) -> LayoutFinding {
        LayoutFinding {
            ref_: node.ref_.clone(),
            role: node.role.clone(),
            name: node.name.clone(),
            bounds: node.bounds,
            detail,
        }
    }
    fn walk(
        node: &AccessNodeDto,
        w: f64,
        h: f64,
        small: &mut Vec<LayoutFinding>,
        off: &mut Vec<LayoutFinding>,
    ) {
        let b = node.bounds;
        // Interactive targets must be at least the minimum hit size.
        if node.focusable && b.w.min(b.h) < MIN_HIT_TARGET {
            small.push(finding(
                node,
                format!(
                    "{:.0}x{:.0} below the {MIN_HIT_TARGET:.0}px minimum hit target",
                    b.w, b.h
                ),
            ));
        }
        // A signal-bearing node outside the window is clipped / unreachable. A
        // half-pixel slack avoids flagging a node sitting exactly on the edge.
        let signal = node.focusable || node.name.is_some() || node.role != "generic";
        let eps = 0.5;
        if signal && (b.x < -eps || b.y < -eps || b.x + b.w > w + eps || b.y + b.h > h + eps) {
            off.push(finding(
                node,
                format!(
                    "rect ({:.0},{:.0} {:.0}x{:.0}) extends outside the {w:.0}x{h:.0} window",
                    b.x, b.y, b.w, b.h
                ),
            ));
        }
        for child in &node.children {
            walk(child, w, h, small, off);
        }
    }
    let (w, h) = (f64::from(size.0), f64::from(size.1));
    let mut small_targets = Vec::new();
    let mut offscreen = Vec::new();
    walk(tree, w, h, &mut small_targets, &mut offscreen);
    LayoutReport {
        small_targets,
        offscreen,
    }
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
    /// Checkbox/switch checked state.
    #[serde(default)]
    pub checked: Option<bool>,
    /// Radio/tab selected state.
    #[serde(default)]
    pub selected: Option<bool>,
    /// `aria-invalid` state.
    #[serde(default)]
    pub invalid: Option<bool>,
    /// Range value at least this (slider/spinbutton/meter/progressbar).
    #[serde(default)]
    pub value_gte: Option<f64>,
    /// Range value at most this.
    #[serde(default)]
    pub value_lte: Option<f64>,
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
            && self.checked.is_none()
            && self.selected.is_none()
            && self.invalid.is_none()
            && self.value_gte.is_none()
            && self.value_lte.is_none()
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
        if let Some(c) = self.checked
            && dto.checked != Some(c)
        {
            return false;
        }
        if let Some(s) = self.selected
            && dto.selected != Some(s)
        {
            return false;
        }
        if let Some(iv) = self.invalid
            && dto.invalid != iv
        {
            return false;
        }
        if let Some(lo) = self.value_gte
            && !dto.value_now.is_some_and(|v| v >= lo)
        {
            return false;
        }
        if let Some(hi) = self.value_lte
            && !dto.value_now.is_some_and(|v| v <= hi)
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

    /// Builds a frame [`Query`] from this selector, for driving the harness.
    /// Mirrors the harness query vocabulary: `role` (+ `name`) first, then
    /// `label`, then `value`, then `id`.
    ///
    /// # Errors
    /// An unknown role word, or an empty selector.
    pub fn to_query(&self) -> Result<Query, String> {
        let mut q = match self.role.as_deref() {
            Some(role) => by::role(role_from_str(role)?),
            None => match (&self.label, &self.label_contains) {
                (Some(l), _) => by::label(l),
                (None, Some(l)) => by::label_contains(l),
                (None, None) => match (&self.value, &self.value_contains) {
                    (Some(v), _) => by::value(v),
                    (None, Some(v)) => by::value_contains(v),
                    (None, None) => match &self.id {
                        Some(id) => by::id(id),
                        None => {
                            return Err("empty selector: set role, label, value, or id".into());
                        }
                    },
                },
            },
        };
        if self.role.is_some() {
            if let Some(l) = &self.label {
                q = q.name(l);
            } else if let Some(l) = &self.label_contains {
                q = q.name_contains(l);
            }
        }
        if let Some(n) = &self.name {
            q = q.name(n);
        } else if let Some(n) = &self.name_contains {
            q = q.name_contains(n);
        }
        Ok(q)
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
    query_tree(&tree, selector).map_err(|e| vec![e])
}

/// Finds nodes matching the selector in an already-captured access `tree` — the
/// primitive [`query`] builds on, exposed so a caller that already holds a tree
/// (e.g. a driven scenario's post-interaction after-tree) can query it without
/// re-rendering. On a miss, returns the nearest candidates.
///
/// # Errors
/// An empty selector (set at least one of role, name, value, or id).
pub fn query_tree(tree: &AccessNodeDto, selector: &Selector) -> Result<QueryResult, DescribeError> {
    if selector.is_empty() {
        return Err(DescribeError::new(
            "selector",
            "empty selector: set role, name, value, or id",
        ));
    }
    let mut matches = Vec::new();
    collect_matching(tree, selector, &mut matches);
    let nearest = if matches.is_empty() {
        nearest_candidates(tree, selector, 5)
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
        Some(Semantics::Checkbox { checked, .. }) => (Some(*checked), None),
        Some(Semantics::Switch { on }) => (Some(*on), None),
        Some(Semantics::Radio { selected }) | Some(Semantics::Tab { selected }) => {
            (None, Some(*selected))
        }
        _ => (None, None),
    };
    let (value_now, value_min, value_max) = match &node.semantics {
        Some(
            Semantics::Slider { value, min, max }
            | Semantics::Spinbutton { value, min, max }
            | Semantics::Meter { value, min, max },
        ) => (
            Some(f64::from(*value)),
            Some(f64::from(*min)),
            Some(f64::from(*max)),
        ),
        Some(Semantics::ProgressBar { value }) => (value.map(f64::from), Some(0.0), Some(1.0)),
        _ => (None, None, None),
    };
    let mixed = match &node.semantics {
        Some(Semantics::Checkbox { mixed, .. }) => mixed.then_some(true),
        _ => None,
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
        value_now,
        value_min,
        value_max,
        mixed,
        focusable: node.focusable,
        invalid: node.invalid,
        live: node.live,
        selection: node.selection.map(|(s, e)| [s, e]),
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

// ----------------------------------------------------------- aria snapshots

/// The aria snapshot of a description — Playwright's `- role "name" [attr]`
/// grammar, deterministic and signal-dense. Lock it with a snapshot test, or
/// match an expected shape against it with [`match_aria`].
///
/// # Errors
/// The parse errors when the description does not parse cleanly.
pub fn aria_snapshot(
    desc: &Description,
    theme: &Theme,
    size: (u32, u32),
) -> Result<String, Vec<DescribeError>> {
    Ok(build(desc, theme, size)?.access_yaml())
}

/// How an expected aria snapshot is matched against the actual one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AriaMode {
    /// Every expected line appears in the actual snapshot, in order; extra
    /// actual lines are ignored. The default — robust to unrelated changes.
    #[default]
    Partial,
    /// The snapshots are identical, line for line (indentation included).
    Strict,
    /// Each expected line is a regular expression, matched in order against the
    /// actual lines. The aria grammar's `[...]` and `"..."` are literal text, so
    /// escape brackets (`\[`, `\]`) when matching attributes.
    Regex,
}

/// Matches an expected aria snapshot against the description's actual one.
///
/// # Errors
/// The parse errors when the description does not parse, or (in [`AriaMode::Regex`])
/// an invalid regular expression in `expected`.
pub fn match_aria(
    desc: &Description,
    theme: &Theme,
    size: (u32, u32),
    expected: &str,
    mode: AriaMode,
) -> Result<AriaDiff, Vec<DescribeError>> {
    let actual = aria_snapshot(desc, theme, size)?;
    match_aria_text(&actual, expected, mode).map_err(|e| vec![e])
}

/// Matches an expected aria snapshot against an `actual` snapshot string — the
/// frame-level primitive [`match_aria`] builds on, exposed so a caller that
/// already holds a snapshot (e.g. a driven scenario's post-interaction
/// `Frame::access_yaml`) can match without re-rendering.
///
/// # Errors
/// In [`AriaMode::Regex`], an invalid regular expression in `expected`.
pub fn match_aria_text(
    actual: &str,
    expected: &str,
    mode: AriaMode,
) -> Result<AriaDiff, DescribeError> {
    match mode {
        AriaMode::Strict => Ok(strict_diff(expected, actual)),
        AriaMode::Partial => Ok(literal_subsequence_diff(expected, actual)),
        AriaMode::Regex => regex_subsequence_diff(expected, actual),
    }
}

/// Non-empty lines with trailing whitespace trimmed (leading indent kept).
fn snapshot_lines(s: &str) -> Vec<&str> {
    s.lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .collect()
}

/// Strict line-for-line comparison (indentation significant).
fn strict_diff(expected: &str, actual: &str) -> AriaDiff {
    let exp = snapshot_lines(expected);
    let act = snapshot_lines(actual);
    if exp == act {
        return AriaDiff {
            ok: true,
            diff: String::new(),
        };
    }
    let mut diff = String::new();
    for i in 0..exp.len().max(act.len()) {
        let e = exp.get(i).copied().unwrap_or_default();
        let a = act.get(i).copied().unwrap_or_default();
        if e == a {
            diff.push_str(&format!("  {a}\n"));
        } else {
            if !e.is_empty() {
                diff.push_str(&format!("- {e}\n"));
            }
            if !a.is_empty() {
                diff.push_str(&format!("+ {a}\n"));
            }
        }
    }
    AriaDiff { ok: false, diff }
}

/// Subsequence match with literal line equality.
fn literal_subsequence_diff(expected: &str, actual: &str) -> AriaDiff {
    let exp: Vec<&str> = snapshot_lines(expected)
        .into_iter()
        .map(str::trim)
        .collect();
    let act: Vec<&str> = snapshot_lines(actual).into_iter().map(str::trim).collect();
    let mut cursor = 0;
    let mut missing = Vec::new();
    for e in &exp {
        match act[cursor..].iter().position(|a| a == e) {
            Some(off) => cursor += off + 1,
            None => missing.push((*e).to_string()),
        }
    }
    finish_subsequence(&missing, &act, "lines not found")
}

/// Subsequence match where each expected line is a regular expression.
fn regex_subsequence_diff(expected: &str, actual: &str) -> Result<AriaDiff, DescribeError> {
    let exp: Vec<&str> = snapshot_lines(expected)
        .into_iter()
        .map(str::trim)
        .collect();
    let act: Vec<&str> = snapshot_lines(actual).into_iter().map(str::trim).collect();
    let mut cursor = 0;
    let mut missing = Vec::new();
    for e in &exp {
        let re = Regex::new(e)
            .map_err(|err| DescribeError::new("expected", format!("invalid regex {e:?}: {err}")))?;
        match act[cursor..].iter().position(|a| re.is_match(a)) {
            Some(off) => cursor += off + 1,
            None => missing.push((*e).to_string()),
        }
    }
    Ok(finish_subsequence(&missing, &act, "patterns not matched"))
}

/// Builds the `AriaDiff` from a subsequence match's missing lines.
fn finish_subsequence(missing: &[String], act: &[&str], kind: &str) -> AriaDiff {
    if missing.is_empty() {
        return AriaDiff {
            ok: true,
            diff: String::new(),
        };
    }
    let mut diff = format!("expected {kind} (in order):\n");
    for m in missing {
        diff.push_str(&format!("- {m}\n"));
    }
    diff.push_str("\nactual:\n");
    for a in act {
        diff.push_str(&format!("  {a}\n"));
    }
    AriaDiff { ok: false, diff }
}

// ------------------------------------------------------------- a11y report

/// Interactive roles that must carry an accessible name.
const INTERACTIVE_ROLES: &[&str] = &[
    "button", "checkbox", "switch", "radio", "slider", "textbox", "combobox", "tab",
];

/// The accessibility report: theme contrast, labeling, and per-node legibility,
/// all from the real resolved render — the evidence that a screen is readable
/// and every control is named, with no pixels required.
///
/// # Errors
/// The parse errors when the description does not parse cleanly.
pub fn check_a11y(
    desc: &Description,
    theme: &Theme,
    size: (u32, u32),
) -> Result<A11yReport, Vec<DescribeError>> {
    Ok(frame_a11y(&build(desc, theme, size)?, theme))
}

/// The accessibility report read from an already-built [`Frame`] — the same
/// evidence as [`check_a11y`], but for a frame the caller already has (e.g. the
/// *post-interaction* frame of a driven scenario, whose state differs from the
/// static description). `theme` supplies the contrast contract and window
/// background the per-node legibility measures against.
#[must_use]
pub fn frame_a11y(frame: &Frame, theme: &Theme) -> A11yReport {
    let tree = node_to_dto(&frame.access_tree(), &[]);

    let contrast_violations: Vec<ContrastDto> = theme
        .contrast_report()
        .iter()
        .map(|v| ContrastDto {
            pair: v.pair.clone(),
            measured_lc: v.measured_lc,
            required_lc: v.required_lc,
        })
        .collect();

    let mut unlabeled = Vec::new();
    collect_unlabeled(&tree, &mut unlabeled);

    let node_legibility: Vec<LegibilityDto> = frame
        .legibility(theme.bg)
        .iter()
        .map(|l| LegibilityDto {
            text: l.text.clone(),
            fg: hex(l.fg),
            bg: hex(l.bg),
            bg_uniform: l.bg_uniform,
            size_px: l.size_px,
            weight: l.weight,
            lc: l.lc,
            required_lc: l.required_lc,
            wcag2: l.wcag2,
            passes_apca: l.passes_apca,
            passes_wcag2: l.passes_wcag2,
            bounds: Bounds {
                x: l.rect.x0,
                y: l.rect.y0,
                w: l.rect.width(),
                h: l.rect.height(),
            },
        })
        .collect();

    // The overall verdict follows the theme's calibrated contrast contract — the
    // same `validate_contrast` proof shipped Looks assert, which uses a relaxed
    // floor for filled-control labels. `node_legibility` below carries the strict
    // per-node APCA/WCAG2 detail (body-text floor) for finer inspection, so a
    // marginal control-label miss does not flip the whole screen to illegible.
    let legible = contrast_violations.is_empty();

    // The strict per-node failures (body-text floor) — surfaced regardless of the
    // relaxed theme verdict so an authored low-contrast run is never silent.
    let text_contrast_failures = node_legibility
        .iter()
        .filter(|l| !l.passes_apca)
        .cloned()
        .collect();

    A11yReport {
        legible,
        contrast_violations,
        unlabeled,
        node_legibility,
        text_contrast_failures,
    }
}

/// Collects interactive nodes that lack an accessible name.
fn collect_unlabeled(node: &AccessNodeDto, out: &mut Vec<AccessNodeDto>) {
    let interactive = INTERACTIVE_ROLES.contains(&node.role.as_str());
    let named = node.name.as_deref().is_some_and(|n| !n.trim().is_empty());
    if interactive && !named {
        out.push(flat(node));
    }
    for child in &node.children {
        collect_unlabeled(child, out);
    }
}

/// Formats a color as `#rrggbb`.
fn hex(c: Color) -> String {
    let p = c.to_rgba8();
    format!("#{:02x}{:02x}{:02x}", p.r, p.g, p.b)
}

/// Maps an ARIA role word to a `Semantics` (state-less; the query matches by
/// role discriminant). Unknown words are reported, not silently ignored.
fn role_from_str(role: &str) -> Result<Semantics, String> {
    Ok(match role {
        "button" => Semantics::Button,
        "checkbox" => Semantics::Checkbox {
            checked: false,
            mixed: false,
        },
        "switch" => Semantics::Switch { on: false },
        "radio" => Semantics::Radio { selected: false },
        "slider" => Semantics::Slider {
            value: 0.0,
            min: 0.0,
            max: 1.0,
        },
        "textbox" => Semantics::TextInput { multiline: false },
        "combobox" => Semantics::ComboBox,
        "dialog" => Semantics::Dialog,
        "tab" => Semantics::Tab { selected: false },
        "alert" => Semantics::Alert,
        "text" => Semantics::Label,
        "image" => Semantics::Image,
        "spinbutton" => Semantics::Spinbutton {
            value: 0.0,
            min: 0.0,
            max: 1.0,
        },
        "meter" => Semantics::Meter {
            value: 0.0,
            min: 0.0,
            max: 1.0,
        },
        "progressbar" => Semantics::ProgressBar { value: None },
        other => {
            return Err(format!(
                "unknown role {other:?} (expected button/checkbox/switch/radio/slider/\
                 textbox/combobox/dialog/tab/alert/text/image/spinbutton/meter/progressbar)"
            ));
        }
    })
}
