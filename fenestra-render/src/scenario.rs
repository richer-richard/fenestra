//! The unified verify loop: a [`Scenario`] is one declarative bundle — a
//! description, optional interaction steps, and a set of expectations — that
//! [`verify`] runs in a single pass, returning one [`VerifyReport`] with a
//! per-check verdict and one overall `ok`.
//!
//! This closes two gaps the per-command engine left open:
//!
//! - **Scenario shot compares.** A screenshot expectation is diffed against the
//!   *post-interaction* pixels when the scenario has steps — so "after I click
//!   Submit, the screen looks like this baseline" is verifiable, not just the
//!   static render.
//! - **Unified verify.** One scenario replaces a hand-reconciled sequence of
//!   `render` / `check` / `match-aria` / `match-png`: every requested check runs
//!   against the same (static or post-interaction) frame and folds into a single
//!   pass/fail an agent — or a CI gate — reads in one shot.

use std::path::PathBuf;

use fenestra_core::Theme;
use fenestra_describe::dto::{A11yReport, AccessNodeDto, Bounds};
use fenestra_describe::format::Description;
use fenestra_describe::inspect::{self, AriaMode, Selector};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::engine::{self, EngineError, Step};
use crate::theme_input::resolve_theme;

/// The scenario schema tag, mirroring the description's `fenestra/1`.
const SCHEMA: &str = "fenestra/1";

/// A declarative verification bundle: a UI description, the interactions to drive
/// (optional), and the expectations to assert. Self-contained — it carries its
/// own theme and size — so one file is one reproducible verification.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    /// Schema tag; must be `fenestra/1`.
    #[serde(default = "default_schema")]
    pub schema: String,
    /// The UI under test.
    pub description: Description,
    /// Optional theme: a `ThemeSpec` object, or `{"preset":"dark"}`. Light when omitted.
    #[serde(default)]
    pub theme: Option<Value>,
    /// Window size as `WxH` (default `800x600`).
    #[serde(default)]
    pub size: Option<String>,
    /// Interactions to drive before asserting. When present, every check runs
    /// against the *post-interaction* frame; when empty, against the static render.
    #[serde(default)]
    pub steps: Vec<Step>,
    /// The expectations to assert. Each set field becomes one check; an empty
    /// `expect` is a smoke gate (the description parses and renders without error).
    #[serde(default)]
    pub expect: Expect,
}

/// The expectations a scenario asserts. Every field is optional; each one that is
/// set contributes a single check to the report.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    /// The exact list of author intent strings the steps should emit, in order.
    #[serde(default)]
    pub emitted: Option<Vec<String>>,
    /// Gate accessibility: the theme is legible and every interactive control is named.
    #[serde(default)]
    pub a11y: bool,
    /// Stricter accessibility: additionally require that every text node clears
    /// its strict per-node APCA floor (no `text_contrast_failures`). Implies the
    /// `a11y` checks too. Catches authored low-contrast text the relaxed theme
    /// contract permits for filled-control labels.
    #[serde(default)]
    pub a11y_strict: bool,
    /// Match an expected aria snapshot against the (post-interaction) tree.
    #[serde(default)]
    pub aria: Option<AriaExpect>,
    /// Compare the (post-interaction) render against a baseline PNG.
    #[serde(default)]
    pub screenshot: Option<ScreenshotExpect>,
    /// Assert how many nodes a selector matches in the (post-interaction) tree.
    #[serde(default)]
    pub queries: Vec<QueryExpect>,
    /// Assert the keyboard focus order: the refs a Tab cycle visits, in order.
    #[serde(default)]
    pub focus_order: Option<Vec<String>>,
    /// Gate layout geometry: no interactive targets below the minimum hit size,
    /// and nothing signal-bearing rendered off-screen.
    #[serde(default)]
    pub layout: bool,
}

/// An expected aria snapshot and how to match it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AriaExpect {
    /// The expected snapshot (Playwright `- role "name" [attr]` grammar).
    pub snapshot: String,
    /// `partial` (subset, default) | `strict` (exact) | `regex` (each line a pattern).
    #[serde(default)]
    pub mode: AriaMode,
}

/// An expected screenshot baseline and its comparison budget.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenshotExpect {
    /// Path to the baseline PNG on disk.
    pub baseline: PathBuf,
    /// Per-channel tolerance (0 = exact).
    #[serde(default)]
    pub tolerance: u8,
    /// Allowed differing-pixel fraction.
    #[serde(default)]
    pub budget: f64,
    /// Rectangles to ignore (volatile regions).
    #[serde(default)]
    pub masks: Vec<Bounds>,
}

/// An expected match count for a semantic selector.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryExpect {
    /// The selector to resolve against the access tree.
    pub selector: Selector,
    /// How many nodes it must match (default 1).
    #[serde(default = "default_one")]
    pub count: usize,
}

/// One check's verdict in the unified report.
#[derive(Debug, Clone, Serialize)]
pub struct CheckOutcome {
    /// What was checked: `emitted`, `a11y`, `aria`, `screenshot`, or `query: …`.
    pub name: String,
    /// Whether it passed.
    pub ok: bool,
    /// A human-readable explanation; empty when `ok`.
    pub detail: String,
}

/// The unified verification report: one verdict, the per-check breakdown, and the
/// post-interaction signal (emitted intents + access tree) for self-correction.
#[derive(Debug, Clone, Serialize)]
pub struct VerifyReport {
    /// True when every check passed (vacuously true when there are no checks).
    pub ok: bool,
    /// The per-check verdicts, in the order emitted/a11y/aria/screenshot/queries.
    pub checks: Vec<CheckOutcome>,
    /// The author intents emitted while driving the steps (empty without steps).
    pub emitted: Vec<String>,
    /// The access tree the checks ran against (post-interaction when there are steps).
    pub tree: AccessNodeDto,
}

/// What [`verify`] produced: the report, the final render, and (on a screenshot
/// mismatch) the diff image.
pub struct VerifyOut {
    /// The unified report.
    pub report: VerifyReport,
    /// The final (post-interaction) render — a preview to attach or save.
    pub png: RgbaImage,
    /// The screenshot diff image — present when a *same-size* screenshot check
    /// failed. A dimension mismatch has no per-pixel overlay, so this stays `None`
    /// there even though the check fails (the report's detail names the mismatch).
    pub diff_png: Option<RgbaImage>,
}

impl std::fmt::Debug for VerifyOut {
    /// Summarizes the images by dimensions rather than dumping every pixel.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dims = |img: &RgbaImage| format!("{}x{}", img.width(), img.height());
        f.debug_struct("VerifyOut")
            .field("report", &self.report)
            .field("png", &format_args!("{}", dims(&self.png)))
            .field("diff_png", &self.diff_png.as_ref().map(dims))
            .finish()
    }
}

/// Runs a scenario: drives its steps (if any), then asserts every requested
/// expectation against the resulting frame, folding the per-check verdicts into
/// one [`VerifyReport`]. A failed *check* is a normal report (`ok: false`), not
/// an error; an [`EngineError`] means the scenario could not run at all.
///
/// # Errors
/// [`EngineError::Parse`] when the description does not parse, [`EngineError::Step`]
/// when an interaction target does not resolve, or [`EngineError::Scenario`] for a
/// setup problem (bad schema/theme/size, an unreadable baseline, a bad pattern).
pub fn verify(scenario: &Scenario) -> Result<VerifyOut, EngineError> {
    let (theme, size) = scenario_env(scenario)?;
    let p = produce(scenario, &theme, size)?;
    let expect = &scenario.expect;
    let mut checks = Vec::new();
    let mut diff_png = None;

    if let Some(want) = &expect.emitted {
        let ok = &p.emitted == want;
        let detail = if ok {
            String::new()
        } else {
            format!("emitted {:?}, expected {want:?}", p.emitted)
        };
        checks.push(outcome("emitted", ok, detail));
    }

    if expect.a11y || expect.a11y_strict {
        let strict_ok = !expect.a11y_strict || p.a11y.text_contrast_failures.is_empty();
        let ok = p.a11y.legible && p.a11y.unlabeled.is_empty() && strict_ok;
        let detail = if ok {
            String::new()
        } else {
            format!(
                "legible: {}; {} unlabeled control(s); {} contrast violation(s); \
                 {} strict text-contrast failure(s)",
                p.a11y.legible,
                p.a11y.unlabeled.len(),
                p.a11y.contrast_violations.len(),
                p.a11y.text_contrast_failures.len(),
            )
        };
        checks.push(outcome("a11y", ok, detail));
    }

    if let Some(aria) = &expect.aria {
        let diff = inspect::match_aria_text(&p.aria, &aria.snapshot, aria.mode)
            .map_err(|e| EngineError::Scenario(format!("expect.aria: {e}")))?;
        let detail = if diff.ok { String::new() } else { diff.diff };
        checks.push(outcome("aria", diff.ok, detail));
    }

    if let Some(shot) = &expect.screenshot {
        let baseline = image::open(&shot.baseline)
            .map_err(|e| {
                EngineError::Scenario(format!("cannot read baseline {:?}: {e}", shot.baseline))
            })?
            .into_rgba8();
        let diff = engine::diff_images(&baseline, &p.png, shot.tolerance, shot.budget, &shot.masks);
        let detail = if diff.ok {
            String::new()
        } else if baseline.dimensions() != p.png.dimensions() {
            // A size mismatch short-circuits the pixel compare (no per-pixel diff
            // is possible), so say so plainly rather than report a saturated
            // all-pixels-differ result an author would mistake for a content change.
            let (bw, bh) = baseline.dimensions();
            let (aw, ah) = p.png.dimensions();
            format!("image size mismatch: baseline {bw}x{bh} vs rendered {aw}x{ah}")
        } else {
            format!(
                "{}/{} pixels differ (max channel delta {} at {:?})",
                diff.differing, diff.total, diff.max_delta, diff.worst,
            )
        };
        if !diff.ok {
            diff_png = diff.diff_png;
        }
        checks.push(outcome("screenshot", diff.ok, detail));
    }

    for q in &expect.queries {
        let res = inspect::query_tree(&p.tree, &q.selector)
            .map_err(|e| EngineError::Scenario(format!("expect.queries: {e}")))?;
        let got = res.matches.len();
        let ok = got == q.count;
        let detail = if ok {
            String::new()
        } else {
            query_miss_detail(got, q.count, &res.nearest)
        };
        checks.push(outcome(
            format!("query: {}", describe_selector(&q.selector)),
            ok,
            detail,
        ));
    }

    if let Some(want) = &expect.focus_order {
        let matched = &p.focus_order == want;
        let detail = if matched {
            String::new()
        } else {
            format!("focus order {:?}, expected {want:?}", p.focus_order)
        };
        checks.push(outcome("focus_order", matched, detail));
    }

    if expect.layout {
        let report = inspect::tree_layout_report(&p.tree, size);
        let matched = report.small_targets.is_empty() && report.offscreen.is_empty();
        let detail = if matched {
            String::new()
        } else {
            format!(
                "{} small target(s), {} off-screen node(s)",
                report.small_targets.len(),
                report.offscreen.len()
            )
        };
        checks.push(outcome("layout", matched, detail));
    }

    let ok = checks.iter().all(|c| c.ok);
    Ok(VerifyOut {
        report: VerifyReport {
            ok,
            checks,
            emitted: p.emitted,
            tree: p.tree,
        },
        png: p.png,
        diff_png,
    })
}

/// (Re)generates a scenario's screenshot baseline: renders the scenario's final
/// (post-interaction) frame and writes it to the `expect.screenshot.baseline`
/// path — the authoring affordance that lets you capture a baseline once, then
/// verify against it. Returns the path written.
///
/// # Errors
/// [`EngineError::Scenario`] when the scenario has no screenshot expectation to
/// bless or the baseline cannot be written, plus the same parse/step/setup errors
/// as [`verify`].
pub fn bless(scenario: &Scenario) -> Result<PathBuf, EngineError> {
    let Some(shot) = &scenario.expect.screenshot else {
        return Err(EngineError::Scenario(
            "nothing to bless: the scenario has no expect.screenshot baseline".into(),
        ));
    };
    let (theme, size) = scenario_env(scenario)?;
    let p = produce(scenario, &theme, size)?;
    p.png.save(&shot.baseline).map_err(|e| {
        EngineError::Scenario(format!("cannot write baseline {:?}: {e}", shot.baseline))
    })?;
    Ok(shot.baseline.clone())
}

// --------------------------------------------------------------- internals

/// The frame a scenario's checks run against: the access tree, the aria snapshot,
/// the a11y report, the rendered pixels, and the emitted intents — captured from
/// either the static render (no steps) or the post-interaction harness (steps).
struct Produced {
    tree: AccessNodeDto,
    aria: String,
    a11y: A11yReport,
    png: RgbaImage,
    emitted: Vec<String>,
    focus_order: Vec<String>,
}

/// Captures the [`Produced`] frame: static when the scenario has no steps,
/// post-interaction when it does (so every check sees the same driven state).
fn produce(scenario: &Scenario, theme: &Theme, size: (u32, u32)) -> Result<Produced, EngineError> {
    if scenario.steps.is_empty() {
        // Static: one render gives the tree, pixels, and a11y; aria is a cheap
        // second build of the same description.
        let out = engine::render(&scenario.description, theme, size)?;
        let aria = inspect::aria_snapshot(&scenario.description, theme, size)
            .map_err(EngineError::Parse)?;
        let focus_order = inspect::focus_order(&scenario.description, theme, size)
            .map_err(EngineError::Parse)?;
        Ok(Produced {
            tree: out.tree,
            aria,
            a11y: out.warnings,
            png: out.png,
            emitted: Vec::new(),
            focus_order,
        })
    } else {
        let mut h = engine::drive(&scenario.description, theme, size, &scenario.steps)?;
        let emitted = engine::emitted_intents(&mut h);
        let tree = inspect::frame_access_tree(h.frame());
        let aria = h.frame().access_yaml();
        let a11y = inspect::frame_a11y(h.frame(), theme);
        let focus_order = inspect::frame_focus_order(h.frame());
        let png = h.render();
        Ok(Produced {
            tree,
            aria,
            a11y,
            png,
            emitted,
            focus_order,
        })
    }
}

/// Resolves a scenario's schema, theme, and size — the setup that must succeed
/// before any check can run.
fn scenario_env(s: &Scenario) -> Result<(Theme, (u32, u32)), EngineError> {
    if s.schema != SCHEMA {
        return Err(EngineError::Scenario(format!(
            "unknown schema {:?}; expected {SCHEMA:?}",
            s.schema
        )));
    }
    let theme = resolve_theme(s.theme.as_ref()).map_err(EngineError::Scenario)?;
    let size = parse_size(s.size.as_deref())?;
    Ok((theme, size))
}

/// Parses a `WxH` size, defaulting to 800x600.
fn parse_size(s: Option<&str>) -> Result<(u32, u32), EngineError> {
    let Some(s) = s else {
        return Ok((800, 600));
    };
    if let Some((w, h)) = s.split_once(['x', 'X'])
        && let (Ok(w), Ok(h)) = (w.trim().parse(), h.trim().parse())
    {
        return Ok((w, h));
    }
    Err(EngineError::Scenario(format!(
        "invalid size {s:?}; expected WxH like 800x600"
    )))
}

/// Builds one check outcome.
fn outcome(name: impl Into<String>, ok: bool, detail: impl Into<String>) -> CheckOutcome {
    CheckOutcome {
        name: name.into(),
        ok,
        detail: detail.into(),
    }
}

/// A readable label for a selector, for the `query:` check name.
fn describe_selector(s: &Selector) -> String {
    let mut parts = Vec::new();
    if let Some(r) = &s.role {
        parts.push(format!("role={r}"));
    }
    if let Some(n) = s
        .name
        .as_ref()
        .or(s.name_contains.as_ref())
        .or(s.label.as_ref())
        .or(s.label_contains.as_ref())
    {
        parts.push(format!("name={n:?}"));
    }
    if let Some(v) = s.value.as_ref().or(s.value_contains.as_ref()) {
        parts.push(format!("value={v:?}"));
    }
    if let Some(id) = &s.id {
        parts.push(format!("id={id}"));
    }
    if parts.is_empty() {
        "<empty>".to_string()
    } else {
        parts.join(" ")
    }
}

/// The detail line for a query-count miss, naming the nearest candidates when the
/// selector matched nothing.
fn query_miss_detail(got: usize, want: usize, nearest: &[AccessNodeDto]) -> String {
    let mut detail = format!("found {got}, expected {want}");
    if got == 0 && !nearest.is_empty() {
        let names: Vec<String> = nearest
            .iter()
            .map(|n| match &n.name {
                Some(name) => format!("{} {name:?}", n.role),
                None => n.role.clone(),
            })
            .collect();
        detail.push_str(&format!("; nearest: {}", names.join(", ")));
    }
    detail
}

/// Returns the default schema tag for deserialization.
fn default_schema() -> String {
    SCHEMA.to_string()
}

/// Returns the default expected match count (1).
fn default_one() -> usize {
    1
}
