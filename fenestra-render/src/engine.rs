//! The pixel and stateful engine: render a description to PNG, drive it through
//! scripted interactions on the headless harness, and compare against a baseline.
//! Built on `fenestra-shell`, so these are the operations that need a GPU; the
//! structural ops (access tree, query, aria, a11y) come from `fenestra-describe`.

use fenestra_core::{Key, KeyInput, Query, Theme};
use fenestra_describe::dto::{A11yReport, AccessNodeDto, Bounds};
use fenestra_describe::error::DescribeError;
use fenestra_describe::format::Description;
use fenestra_describe::inspect::{self, Selector};
use fenestra_describe::parse::to_element;
use fenestra_describe::state::{Action, StateMap};
use fenestra_shell::{Harness, render_element};
use image::{Rgba, RgbaImage};
use serde::Deserialize;

use crate::described_app::DescribedApp;

/// An engine error: a description that did not parse, or an interaction step
/// whose target could not be resolved. Both carry enough to self-correct.
#[derive(Debug)]
pub enum EngineError {
    /// The description did not parse; the path-pointed problems.
    Parse(Vec<DescribeError>),
    /// An interaction step could not resolve its target (a miss or ambiguity).
    /// Carries the step index, the reason, and the current access tree.
    Step {
        /// Zero-based index of the failing step.
        index: usize,
        /// What went wrong.
        message: String,
        /// The accessibility tree at the point of failure, for self-correction.
        tree: String,
    },
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(errs) => {
                writeln!(f, "description did not parse:")?;
                for e in errs {
                    writeln!(f, "  {e}")?;
                }
                Ok(())
            }
            Self::Step {
                index,
                message,
                tree,
            } => write!(f, "step {index}: {message}\naccessibility tree:\n{tree}"),
        }
    }
}

impl std::error::Error for EngineError {}

/// What [`render`] produced.
pub struct RenderOut {
    /// The typed access tree — the agent reads this first.
    pub tree: AccessNodeDto,
    /// The rendered pixels.
    pub png: RgbaImage,
    /// Automatic accessibility warnings (contrast, labeling, per-node legibility).
    pub warnings: A11yReport,
}

/// Renders a description: the typed access tree (first), the pixels, and the
/// automatic accessibility report.
///
/// # Errors
/// [`EngineError::Parse`] when the description does not parse cleanly.
pub fn render(
    desc: &Description,
    theme: &Theme,
    size: (u32, u32),
) -> Result<RenderOut, EngineError> {
    let tree = inspect::access_tree(desc, theme, size).map_err(EngineError::Parse)?;
    let warnings = inspect::check_a11y(desc, theme, size).map_err(EngineError::Parse)?;
    let el = to_element(desc, theme).map_err(EngineError::Parse)?;
    let png = render_element(el, theme, size);
    Ok(RenderOut {
        tree,
        png,
        warnings,
    })
}

/// One interaction step. Targets are semantic selectors — never coordinates.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    /// Click the matched node.
    Click(Selector),
    /// Right-click the matched node.
    RightClick(Selector),
    /// Double-click the matched node.
    DoubleClick(Selector),
    /// Triple-click the matched node.
    TripleClick(Selector),
    /// Shift-click the matched node.
    ShiftClick(Selector),
    /// Move the pointer over the matched node.
    Hover(Selector),
    /// Commit text to the focused element.
    Type(String),
    /// Press a key chord, e.g. `"enter"`, `"cmd+z"`, `"ctrl+shift+a"`.
    Key(String),
    /// Tab forward `n` times.
    Tab(u32),
    /// Tab backward `n` times.
    ShiftTab(u32),
    /// Scroll the wheel over the matched node.
    Wheel {
        /// The node to scroll.
        target: Selector,
        /// Vertical delta (positive moves content down).
        dy: f32,
    },
    /// Drag from one node to another.
    Drag {
        /// Press here.
        from: Selector,
        /// Release here.
        to: Selector,
    },
    /// Advance the deterministic clock by `ms` milliseconds.
    PumpMs(f64),
}

/// What [`interact`] produced.
pub struct InteractOut {
    /// Intent strings emitted by handlers during the steps (the Elm-level signal).
    pub emitted: Vec<String>,
    /// The access tree after the steps (framework-owned state changes are visible).
    pub tree: AccessNodeDto,
    /// The rendered pixels after the steps, when requested.
    pub png: Option<RgbaImage>,
    /// The runtime state after the steps (bound widgets' values reflect here).
    pub state: StateMap,
}

/// Drives a description through scripted interactions on the headless harness,
/// then captures the emitted intents and the resulting access tree (and pixels,
/// when `want_png`). Selectors resolve strictly; a miss returns the access tree
/// so the caller self-corrects.
///
/// # Errors
/// [`EngineError::Parse`] on a parse error, or [`EngineError::Step`] when a step's
/// target does not resolve to exactly one node.
pub fn interact(
    desc: &Description,
    theme: &Theme,
    size: (u32, u32),
    steps: &[Step],
    want_png: bool,
) -> Result<InteractOut, EngineError> {
    // Validate first, so a parse error is reported before we drive anything.
    to_element(desc, theme).map_err(EngineError::Parse)?;
    let app = DescribedApp::new(desc.clone(), theme.clone());
    let mut h = Harness::new(app, theme.clone(), size);
    for (index, step) in steps.iter().enumerate() {
        apply_step(&mut h, step, index)?;
    }
    // Map the emitted actions: keep the inert author intents (the Elm-level
    // signal); the framework-owned Set actions are already applied to the state.
    let emitted = h
        .take_messages()
        .into_iter()
        .filter_map(|a| match a {
            Action::Intent(s) => Some(s),
            Action::SetBool(..) | Action::SetText(..) | Action::SetNumber(..) => None,
        })
        .collect();
    let state = h.app().state().clone();
    let tree = inspect::frame_access_tree(h.frame());
    let png = if want_png { Some(h.render()) } else { None };
    Ok(InteractOut {
        emitted,
        tree,
        png,
        state,
    })
}

/// Resolves a selector against the harness's current frame, returning a
/// self-explaining error (with the tree) on a miss or ambiguity.
fn resolve(h: &Harness<DescribedApp>, sel: &Selector, index: usize) -> Result<Query, EngineError> {
    let fail = |message: String| EngineError::Step {
        index,
        message,
        tree: h.frame().access_yaml(),
    };
    let q = sel.to_query().map_err(&fail)?;
    h.frame()
        .try_get(&q)
        .map_err(|e| fail(format!("target [{q}]: {e}")))?;
    Ok(q)
}

/// Applies one step to the harness.
fn apply_step(h: &mut Harness<DescribedApp>, step: &Step, index: usize) -> Result<(), EngineError> {
    match step {
        Step::Click(s) => {
            let q = resolve(h, s, index)?;
            h.click(&q);
        }
        Step::RightClick(s) => {
            let q = resolve(h, s, index)?;
            h.right_click(&q);
        }
        Step::DoubleClick(s) => {
            let q = resolve(h, s, index)?;
            h.double_click(&q);
        }
        Step::TripleClick(s) => {
            let q = resolve(h, s, index)?;
            h.triple_click(&q);
        }
        Step::ShiftClick(s) => {
            let q = resolve(h, s, index)?;
            h.shift_click(&q);
        }
        Step::Hover(s) => {
            let q = resolve(h, s, index)?;
            h.hover(&q);
        }
        Step::Type(text) => h.type_text(text.clone()),
        Step::Key(spec) => {
            let key = key_from_str(spec).map_err(|message| EngineError::Step {
                index,
                message,
                tree: h.frame().access_yaml(),
            })?;
            h.key(key);
        }
        Step::Tab(n) => {
            for _ in 0..*n {
                h.tab();
            }
        }
        Step::ShiftTab(n) => {
            for _ in 0..*n {
                h.shift_tab();
            }
        }
        Step::Wheel { target, dy } => {
            let q = resolve(h, target, index)?;
            h.wheel(&q, *dy);
        }
        Step::Drag { from, to } => {
            let from_q = resolve(h, from, index)?;
            let to_q = to.to_query().map_err(|message| EngineError::Step {
                index,
                message,
                tree: h.frame().access_yaml(),
            })?;
            h.drag(&from_q, &to_q);
        }
        Step::PumpMs(ms) => h.pump(*ms),
    }
    Ok(())
}

/// Parses a key chord like `"enter"`, `"cmd+z"`, or `"ctrl+shift+a"`.
fn key_from_str(spec: &str) -> Result<KeyInput, String> {
    let mut input = KeyInput::plain(Key::Enter);
    let mut key = None;
    for token in spec.split('+') {
        match token.trim().to_lowercase().as_str() {
            "shift" => input.shift = true,
            "ctrl" | "control" => input.ctrl = true,
            "alt" | "option" => input.alt = true,
            "cmd" | "meta" | "super" | "win" => input.meta = true,
            "enter" | "return" => key = Some(Key::Enter),
            "space" => key = Some(Key::Space),
            "escape" | "esc" => key = Some(Key::Escape),
            "left" | "arrowleft" => key = Some(Key::ArrowLeft),
            "right" | "arrowright" => key = Some(Key::ArrowRight),
            "up" | "arrowup" => key = Some(Key::ArrowUp),
            "down" | "arrowdown" => key = Some(Key::ArrowDown),
            "home" => key = Some(Key::Home),
            "end" => key = Some(Key::End),
            "backspace" => key = Some(Key::Backspace),
            "delete" => key = Some(Key::Delete),
            "pageup" => key = Some(Key::PageUp),
            "pagedown" => key = Some(Key::PageDown),
            other => {
                let mut chars = other.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => key = Some(Key::Char(c)),
                    _ => return Err(format!("unknown key token {token:?} in {spec:?}")),
                }
            }
        }
    }
    match key {
        Some(k) => {
            input.key = k;
            Ok(input)
        }
        None => Err(format!("no key in {spec:?} (only modifiers)")),
    }
}

/// What [`match_screenshot`] produced.
pub struct ScreenshotDiff {
    /// True when the differing-pixel fraction is within budget.
    pub ok: bool,
    /// Pixels exceeding the per-channel tolerance (masked pixels excluded).
    pub differing: u64,
    /// Total compared pixels.
    pub total: u64,
    /// Largest per-channel delta seen.
    pub max_delta: u8,
    /// Coordinate of the worst pixel.
    pub worst: (u32, u32),
    /// A diff image (offending pixels in red over the dimmed baseline), when not ok.
    pub diff_png: Option<RgbaImage>,
}

/// Renders the description and compares it to `baseline`, pixel by pixel,
/// allowing `channel_tol` per-channel delta and up to `budget` (a fraction) of
/// pixels to exceed it. Masked rectangles are ignored.
///
/// # Errors
/// [`EngineError::Parse`] when the description does not parse cleanly.
pub fn match_screenshot(
    desc: &Description,
    theme: &Theme,
    size: (u32, u32),
    baseline: &RgbaImage,
    channel_tol: u8,
    budget: f64,
    masks: &[Bounds],
) -> Result<ScreenshotDiff, EngineError> {
    let el = to_element(desc, theme).map_err(EngineError::Parse)?;
    let actual = render_element(el, theme, size);
    Ok(compare(baseline, &actual, channel_tol, budget, masks))
}

/// Whether `(x, y)` lies inside any mask rectangle.
fn masked(x: u32, y: u32, masks: &[Bounds]) -> bool {
    masks.iter().any(|m| {
        let (px, py) = (f64::from(x), f64::from(y));
        px >= m.x && px < m.x + m.w && py >= m.y && py < m.y + m.h
    })
}

/// Compares two images, producing the diff stats and (on failure) a diff image.
fn compare(
    golden: &RgbaImage,
    actual: &RgbaImage,
    channel_tol: u8,
    budget: f64,
    masks: &[Bounds],
) -> ScreenshotDiff {
    if golden.dimensions() != actual.dimensions() {
        let total = u64::from(actual.width()) * u64::from(actual.height());
        return ScreenshotDiff {
            ok: false,
            differing: total,
            total,
            max_delta: 255,
            worst: (0, 0),
            diff_png: None,
        };
    }
    let total = u64::from(golden.width()) * u64::from(golden.height());
    let mut differing = 0u64;
    let mut max_delta = 0u8;
    let mut worst = (0u32, 0u32);
    let mut diff = RgbaImage::from_pixel(golden.width(), golden.height(), Rgba([0, 0, 0, 255]));
    for (x, y, a) in actual.enumerate_pixels() {
        if masked(x, y, masks) {
            diff.put_pixel(x, y, Rgba([40, 40, 40, 255]));
            continue;
        }
        let g = golden.get_pixel(x, y);
        let mut exceeds = false;
        for c in 0..4 {
            let delta = g.0[c].abs_diff(a.0[c]);
            if delta > max_delta {
                max_delta = delta;
                worst = (x, y);
            }
            if delta > channel_tol {
                exceeds = true;
            }
        }
        if exceeds {
            differing += 1;
            diff.put_pixel(x, y, Rgba([255, 0, 0, 255]));
        } else {
            let p = g.0;
            diff.put_pixel(x, y, Rgba([p[0] / 3, p[1] / 3, p[2] / 3, 255]));
        }
    }
    #[expect(clippy::cast_precision_loss, reason = "image pixel counts are small")]
    let fraction = differing as f64 / total as f64;
    let ok = fraction <= budget;
    ScreenshotDiff {
        ok,
        differing,
        total,
        max_delta,
        worst,
        diff_png: if ok { None } else { Some(diff) },
    }
}
