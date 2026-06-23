//! Scenario scripts: drive a [`Harness`] from JSON — events, semantic
//! targets, assertions, and named screenshots — so an agent can verify
//! a UI without writing Rust for each probe.
//!
//! A scenario is `{"steps": [...]}` where each step is one verb:
//!
//! ```json
//! {"steps": [
//!   {"click":  {"role": "button", "name": "Add"}},
//!   {"type":   "buy milk"},
//!   {"key":    "enter"},
//!   {"assert": {"exists": {"label": "buy milk"}}},
//!   {"assert": {"count": {"target": {"role": "checkbox"}, "equals": 1}}},
//!   {"shot":   "after-add"}
//! ]}
//! ```
//!
//! Verbs: `click`, `right_click`, `double_click`, `triple_click`,
//! `shift_click`, `hover` (semantic
//! target inline); `type` (string); `key` (e.g. `"enter"`,
//! `"cmd+z"`, `"ctrl+shift+a"`); `tab` / `shift_tab` (count);
//! `wheel` `{target, dy}`; `drag` `{from, to}`; `drop_file`
//! `{target, path}`; `pump_ms` (advance the clock); `window` (activate
//! by key); `shot` (PNG into the scenario's shot directory); `assert`
//! with `exists` / `absent` / `count` / `value` `{target, equals}` /
//! `windows` (the open set). Targets use the query vocabulary:
//! `role`, `name`/`name_contains`, `label`/`label_contains`,
//! `value`/`value_contains`, `id`. Unknown fields are errors, not
//! typos silently ignored.

use std::path::{Path, PathBuf};

use fenestra_core::{App, Key, KeyInput, Query, Semantics, by};
use serde::Deserialize;

use crate::Harness;

/// A failed step (or a parse failure, `step: None`), with enough
/// context to fix the scenario without re-running it.
#[derive(Debug)]
pub struct ScenarioError {
    /// Zero-based index of the failing step; `None` for parse errors.
    pub step: Option<usize>,
    /// What went wrong, including the accessibility tree where useful.
    pub message: String,
}

impl std::fmt::Display for ScenarioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.step {
            Some(i) => write!(f, "scenario step {i}: {}", self.message),
            None => write!(f, "scenario: {}", self.message),
        }
    }
}

impl std::error::Error for ScenarioError {}

/// What a successful run did.
#[derive(Debug)]
pub struct ScenarioReport {
    /// Steps executed.
    pub steps_run: usize,
    /// Screenshots written, in step order.
    pub shots: Vec<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    steps: Vec<Step>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum Step {
    Click(QuerySpec),
    RightClick(QuerySpec),
    DoubleClick(QuerySpec),
    TripleClick(QuerySpec),
    ShiftClick(QuerySpec),
    Hover(QuerySpec),
    Type(String),
    Key(String),
    Tab(u32),
    ShiftTab(u32),
    Wheel {
        target: QuerySpec,
        #[serde(default)]
        dx: f32,
        dy: f32,
    },
    Drag {
        from: QuerySpec,
        to: QuerySpec,
    },
    DropFile {
        target: QuerySpec,
        path: String,
    },
    PumpMs(f64),
    Window(String),
    Shot(String),
    Assert(AssertSpec),
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum AssertSpec {
    Exists(QuerySpec),
    Absent(QuerySpec),
    Count { target: QuerySpec, equals: usize },
    Value { target: QuerySpec, equals: String },
    Windows(Vec<String>),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuerySpec {
    role: Option<String>,
    name: Option<String>,
    name_contains: Option<String>,
    label: Option<String>,
    label_contains: Option<String>,
    value: Option<String>,
    value_contains: Option<String>,
    id: Option<String>,
}

impl QuerySpec {
    fn to_query(&self) -> Result<Query, String> {
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
                        None => return Err("empty target: set role, label, value, or id".into()),
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
        other => {
            return Err(format!(
                "unknown role {other:?} (expected button/checkbox/switch/radio/slider/\
                 textbox/combobox/dialog/tab/alert/text/image)"
            ));
        }
    })
}

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

/// Runs a JSON scenario against the harness. Screenshots from `shot`
/// steps land in `shots_dir` as `<name>.png`.
///
/// # Errors
/// On JSON that does not parse, a target that matches zero or several
/// nodes, an unknown role/key, or a failed assertion — with the step
/// index and (for target failures) the accessibility tree.
pub fn run_scenario<A: App>(
    harness: &mut Harness<A>,
    json: &str,
    shots_dir: impl AsRef<Path>,
) -> Result<ScenarioReport, ScenarioError>
where
    A::Msg: Send,
{
    let scenario: Scenario = serde_json::from_str(json).map_err(|e| ScenarioError {
        step: None,
        message: format!("invalid scenario JSON: {e}"),
    })?;
    let shots_dir = shots_dir.as_ref();
    let mut shots = Vec::new();

    for (i, step) in scenario.steps.iter().enumerate() {
        let fail = |message: String| ScenarioError {
            step: Some(i),
            message,
        };
        // Resolves a target strictly, with the tree in the error.
        macro_rules! target {
            ($spec:expr) => {{
                let q = $spec.to_query().map_err(&fail)?;
                harness.frame().try_get(&q).map_err(|e| {
                    fail(format!(
                        "target [{q}]: {e}\naccessibility tree:\n{}",
                        harness.frame().access_yaml()
                    ))
                })?;
                q
            }};
        }
        match step {
            Step::Click(spec) => {
                let q = target!(spec);
                harness.click(&q);
            }
            Step::RightClick(spec) => {
                let q = target!(spec);
                harness.right_click(&q);
            }
            Step::DoubleClick(spec) => {
                let q = target!(spec);
                harness.double_click(&q);
            }
            Step::TripleClick(spec) => {
                let q = target!(spec);
                harness.triple_click(&q);
            }
            Step::ShiftClick(spec) => {
                let q = target!(spec);
                harness.shift_click(&q);
            }
            Step::Hover(spec) => {
                let q = target!(spec);
                harness.hover(&q);
            }
            Step::Type(text) => harness.type_text(text.clone()),
            Step::Key(spec) => {
                let key = key_from_str(spec).map_err(&fail)?;
                harness.key(key);
            }
            Step::Tab(count) => {
                for _ in 0..*count {
                    harness.tab();
                }
            }
            Step::ShiftTab(count) => {
                for _ in 0..*count {
                    harness.shift_tab();
                }
            }
            Step::Wheel { target, dx, dy } => {
                let q = target!(target);
                harness.wheel_xy(&q, *dx, *dy);
            }
            Step::Drag { from, to } => {
                let from = target!(from);
                let to = to.to_query().map_err(&fail)?;
                harness.drag(&from, &to);
            }
            Step::DropFile { target, path } => {
                let q = target!(target);
                harness.drop_file(&q, path.clone());
            }
            Step::PumpMs(ms) => harness.pump(*ms),
            Step::Window(key) => {
                if !harness.window_keys().iter().any(|k| k == key) {
                    return Err(fail(format!(
                        "no open window {key:?}; open windows: {:?}",
                        harness.window_keys()
                    )));
                }
                harness.activate_window(key);
            }
            Step::Shot(name) => {
                std::fs::create_dir_all(shots_dir)
                    .map_err(|e| fail(format!("create shots dir: {e}")))?;
                let path = shots_dir.join(format!("{name}.png"));
                let image = harness.render();
                image
                    .save(&path)
                    .map_err(|e| fail(format!("write {}: {e}", path.display())))?;
                shots.push(path);
            }
            Step::Assert(assert) => run_assert(harness, assert).map_err(&fail)?,
        }
    }
    Ok(ScenarioReport {
        steps_run: scenario.steps.len(),
        shots,
    })
}

fn run_assert<A: App>(harness: &Harness<A>, assert: &AssertSpec) -> Result<(), String>
where
    A::Msg: Send,
{
    let tree = || format!("\naccessibility tree:\n{}", harness.frame().access_yaml());
    match assert {
        AssertSpec::Exists(spec) => {
            let q = spec.to_query()?;
            harness
                .frame()
                .try_get(&q)
                .map_err(|e| format!("assert exists [{q}]: {e}{}", tree()))?;
        }
        AssertSpec::Absent(spec) => {
            let q = spec.to_query()?;
            if !harness.frame().get_all(&q).is_empty() {
                return Err(format!("assert absent [{q}]: it exists{}", tree()));
            }
        }
        AssertSpec::Count { target, equals } => {
            let q = target.to_query()?;
            let n = harness.frame().get_all(&q).len();
            if n != *equals {
                return Err(format!("assert count [{q}]: {n} != {equals}{}", tree()));
            }
        }
        AssertSpec::Value { target, equals } => {
            let q = target.to_query()?;
            let node = harness
                .frame()
                .try_get(&q)
                .map_err(|e| format!("assert value [{q}]: {e}{}", tree()))?;
            let value = node.value.as_deref().unwrap_or("");
            if value != equals {
                return Err(format!("assert value [{q}]: {value:?} != {equals:?}"));
            }
        }
        AssertSpec::Windows(expected) => {
            let open = harness.window_keys();
            if &open != expected {
                return Err(format!("assert windows: open {open:?} != {expected:?}"));
            }
        }
    }
    Ok(())
}
