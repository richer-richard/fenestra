//! The fenestra MCP server: ten tools that render and verify a UI described as
//! `fenestra/1` JSON. Each tool leads with a typed structured result (the access
//! tree, a report, a diff); visual tools also attach a downscaled preview image.
//!
//! Error channel: a malformed description or selector is a protocol error
//! (`ErrorData::invalid_params`); a render/GPU failure surfaces as an internal
//! error; a *verification mismatch* (a failed aria/screenshot/a11y check) is a
//! normal successful result the agent reads — not an error.

use fenestra_core::Theme;
use fenestra_describe as describe;
use fenestra_describe::dto::{A11yReport, AriaDiff, FocusOrder, QueryResult};
use fenestra_describe::error::DescribeError;
use fenestra_describe::format::Description;
use fenestra_describe::inspect::{self, AriaMode, Selector};
use fenestra_describe::vocabulary::Vocabulary;
use fenestra_render::engine::{self, EngineError, Step};
use fenestra_render::resolve_theme;
use fenestra_render::scenario;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::{
    ErrorData, Json, ServerHandler, model::CallToolResult, tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::content;

/// The stateless fenestra MCP server.
#[derive(Clone, Default)]
pub struct FenestraServer;

#[tool_router]
impl FenestraServer {
    /// A new server.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    #[tool(
        name = "render_ui",
        description = "Render a fenestra/1 UI description to a typed accessibility tree, a downscaled preview image, and automatic accessibility warnings (contrast, labeling, per-text-node legibility). Read the access tree first; the full-resolution PNG comes back as a resource_link."
    )]
    async fn render_ui(
        &self,
        Parameters(p): Parameters<RenderParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let desc = parse_desc(&p.description)?;
        let theme = theme_of(p.theme.as_ref())?;
        let size = parse_size(p.size.as_deref())?;
        let out = blocking(move || engine::render(&desc, &theme, size))
            .await?
            .map_err(engine_err)?;
        let structured = json!({
            "tree": out.tree,
            "warnings": out.warnings,
        });
        let text = format!(
            "{}\n\nlegible: {} · unlabeled controls: {} · contrast violations: {}",
            serde_json::to_string_pretty(&out.tree).unwrap_or_default(),
            out.warnings.legible,
            out.warnings.unlabeled.len(),
            out.warnings.contrast_violations.len(),
        );
        Ok(content::ok(text, structured, Some(&out.png)))
    }

    #[tool(
        name = "query_ui",
        description = "Find nodes in a UI by a semantic selector (role, name, value, or id). Returns matches with stable refs; on a miss, returns the nearest candidates to guide a retry."
    )]
    async fn query_ui(
        &self,
        Parameters(p): Parameters<QueryParams>,
    ) -> Result<Json<QueryResult>, ErrorData> {
        let desc = parse_desc(&p.description)?;
        let theme = theme_of(p.theme.as_ref())?;
        let size = parse_size(p.size.as_deref())?;
        let selector: Selector = serde_json::from_value(p.selector.clone())
            .map_err(|e| ErrorData::invalid_params(format!("invalid selector: {e}"), None))?;
        let result = inspect::query(&desc, &theme, size, &selector).map_err(map_parse)?;
        Ok(Json(result))
    }

    #[tool(
        name = "interact",
        description = "Drive a UI through scripted interactions (click, type, key, tab, hover, wheel, drag — by semantic selector, never coordinates). Returns the emitted intent messages and the access tree afterwards; set screenshot=true for a preview image."
    )]
    async fn interact(
        &self,
        Parameters(p): Parameters<InteractParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let desc = parse_desc(&p.description)?;
        let theme = theme_of(p.theme.as_ref())?;
        let size = parse_size(p.size.as_deref())?;
        let steps: Vec<Step> = serde_json::from_value(p.steps.clone())
            .map_err(|e| ErrorData::invalid_params(format!("invalid steps: {e}"), None))?;
        let want_png = p.screenshot;
        let out = blocking(move || engine::interact(&desc, &theme, size, &steps, want_png))
            .await?
            .map_err(engine_err)?;
        let structured = json!({
            "emitted": out.emitted,
            "tree": out.tree,
            "state": out.state,
        });
        let text = format!(
            "emitted: {:?}\n\n{}",
            out.emitted,
            serde_json::to_string_pretty(&out.tree).unwrap_or_default()
        );
        Ok(content::ok(text, structured, out.png.as_ref()))
    }

    #[tool(
        name = "check_a11y",
        description = "Check accessibility of a UI from the real render: theme contrast, labeling of every interactive control, and per-text-node APCA + WCAG 2 legibility. text_contrast_failures lists nodes failing the strict body-text floor even when the theme verdict is legible (catching authored low-contrast text). Returns a structured report; this is a normal result whether or not it passes."
    )]
    async fn check_a11y(
        &self,
        Parameters(p): Parameters<CheckParams>,
    ) -> Result<Json<A11yReport>, ErrorData> {
        let desc = parse_desc(&p.description)?;
        let theme = theme_of(p.theme.as_ref())?;
        let size = parse_size(p.size.as_deref())?;
        let report = inspect::check_a11y(&desc, &theme, size).map_err(map_parse)?;
        Ok(Json(report))
    }

    #[tool(
        name = "focus_order",
        description = "Return the keyboard focus order: the refs a Tab cycle visits, in order, honoring a modal focus trap (disabled controls excluded). Verifies reachability and tab sequence as typed data, without driving the UI."
    )]
    async fn focus_order(
        &self,
        Parameters(p): Parameters<CheckParams>,
    ) -> Result<Json<FocusOrder>, ErrorData> {
        let desc = parse_desc(&p.description)?;
        let theme = theme_of(p.theme.as_ref())?;
        let size = parse_size(p.size.as_deref())?;
        let order = inspect::focus_order(&desc, &theme, size).map_err(map_parse)?;
        Ok(Json(FocusOrder { order }))
    }

    #[tool(
        name = "match_aria_snapshot",
        description = "Match an expected accessibility snapshot (Playwright `- role \"name\" [attr]` grammar) against a UI. mode: partial (subset, default) | strict (exact) | regex (each expected line is a pattern). Returns a pass/fail diff — a normal result."
    )]
    async fn match_aria_snapshot(
        &self,
        Parameters(p): Parameters<MatchAriaParams>,
    ) -> Result<Json<AriaDiff>, ErrorData> {
        let desc = parse_desc(&p.description)?;
        let theme = theme_of(p.theme.as_ref())?;
        let size = parse_size(p.size.as_deref())?;
        let mode = match p.mode.as_str() {
            "partial" => AriaMode::Partial,
            "strict" => AriaMode::Strict,
            "regex" => AriaMode::Regex,
            other => {
                return Err(ErrorData::invalid_params(
                    format!("unknown mode {other:?}; expected partial|strict|regex"),
                    None,
                ));
            }
        };
        let diff =
            inspect::match_aria(&desc, &theme, size, &p.expected, mode).map_err(map_parse)?;
        Ok(Json(diff))
    }

    #[tool(
        name = "match_screenshot",
        description = "Compare a UI's render against a baseline PNG (path on disk), pixel by pixel, with an optional per-channel tolerance and differing-pixel budget. Returns the diff stats and a diff-image preview on mismatch — a normal result."
    )]
    async fn match_screenshot(
        &self,
        Parameters(p): Parameters<MatchScreenshotParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let desc = parse_desc(&p.description)?;
        let theme = theme_of(p.theme.as_ref())?;
        let size = parse_size(p.size.as_deref())?;
        let baseline = image::open(&p.baseline_path)
            .map_err(|e| {
                ErrorData::invalid_params(
                    format!("cannot read baseline {:?}: {e}", p.baseline_path),
                    None,
                )
            })?
            .into_rgba8();
        let (tol, budget) = (p.tolerance, p.budget);
        let diff = blocking(move || {
            engine::match_screenshot(&desc, &theme, size, &baseline, tol, budget, &[])
        })
        .await?
        .map_err(engine_err)?;
        let structured = json!({
            "ok": diff.ok,
            "differing": diff.differing,
            "total": diff.total,
            "max_delta": diff.max_delta,
            "worst": [diff.worst.0, diff.worst.1],
        });
        let text = format!(
            "{}: {}/{} pixels differ (max channel delta {})",
            if diff.ok { "match" } else { "mismatch" },
            diff.differing,
            diff.total,
            diff.max_delta,
        );
        Ok(content::ok(text, structured, diff.diff_png.as_ref()))
    }

    #[tool(
        name = "describe_vocabulary",
        description = "Return the description grammar: every node type with a minimal example, and the theme color roles a color may name. Call this first to learn how to author a fenestra/1 description."
    )]
    async fn describe_vocabulary(&self) -> Json<Vocabulary> {
        Json(describe::vocabulary::describe_vocabulary())
    }

    #[tool(
        name = "validate",
        description = "Validate a fenestra/1 description without rendering. Structural problems (unknown fields, bad node tags) and semantic ones (an unknown color role) come back path-pointed. Returns isError when the description is invalid."
    )]
    async fn validate(
        &self,
        Parameters(p): Parameters<ValidateParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = serde_json::to_string(&p.description).map_err(|e| {
            ErrorData::invalid_params(format!("description is not serializable: {e}"), None)
        })?;
        match describe::parse::validate(&text) {
            Ok(()) => Ok(content::ok(
                "valid".to_string(),
                json!({ "valid": true, "errors": [] }),
                None,
            )),
            Err(errs) => Ok(content::error(
                format!(
                    "{} problem(s):\n{}",
                    errs.len(),
                    errs.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
                json!({ "valid": false, "errors": errs_to_value(&errs) }),
            )),
        }
    }

    #[tool(
        name = "run_scenario",
        description = "Run a verification scenario in one pass: a fenestra/1 description, optional interaction steps, and a bundle of expectations (emitted intents, a11y, an aria snapshot, a screenshot baseline, query match-counts). Drives the steps, then asserts every expectation against the resulting frame — the screenshot check compares the POST-interaction pixels. Returns a unified report (one `ok` plus a per-check breakdown) and a preview: the diff image on a screenshot mismatch, else the final render. A failed check is a normal result, not an error."
    )]
    async fn run_scenario(
        &self,
        Parameters(p): Parameters<RunScenarioParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let scenario: scenario::Scenario = serde_json::from_value(p.scenario.clone())
            .map_err(|e| ErrorData::invalid_params(format!("invalid scenario: {e}"), None))?;
        let out = blocking(move || scenario::verify(&scenario))
            .await?
            .map_err(engine_err)?;
        let scenario::VerifyOut {
            report,
            png,
            diff_png,
        } = out;
        let structured = serde_json::to_value(&report).map_err(|e| {
            ErrorData::internal_error(format!("cannot serialize report: {e}"), None)
        })?;
        let passed = report.checks.iter().filter(|c| c.ok).count();
        let mut text = format!(
            "verify {}: {passed}/{} check(s) passed",
            if report.ok { "PASS" } else { "FAIL" },
            report.checks.len(),
        );
        for c in report.checks.iter().filter(|c| !c.ok) {
            text.push_str(&format!("\n  ✗ {}: {}", c.name, c.detail));
        }
        // Lead the preview with the diff on a screenshot miss, else the final render.
        let image = diff_png.as_ref().or(Some(&png));
        Ok(content::ok(text, structured, image))
    }
}

#[tool_handler]
impl ServerHandler for FenestraServer {
    fn get_info(&self) -> ServerInfo {
        // ServerInfo and Implementation are `#[non_exhaustive]`, so they cannot be
        // built with a struct literal from here; mutate a default instead. The
        // default `server_info` is `from_build_env`, which captures *rmcp's* name
        // and version (not ours), so set our own identity explicitly.
        let mut info = ServerInfo::default();
        info.server_info.name = "fenestra-mcp".to_string();
        info.server_info.version = env!("CARGO_PKG_VERSION").to_string();
        info.instructions = Some(
            "fenestra renders and verifies native UIs described as fenestra/1 JSON. Call \
             describe_vocabulary first to learn the format; render_ui to see a UI and its \
             accessibility warnings; query_ui, check_a11y, focus_order, match_aria_snapshot, and \
             match_screenshot to assert; interact to drive it; run_scenario to drive steps \
             and assert a whole bundle of expectations in one pass (the screenshot check \
             compares the post-interaction pixels); validate to check a description without \
             rendering."
                .to_string(),
        );
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }
}

// ---------------------------------------------------------------- params

#[derive(Debug, Deserialize, JsonSchema, Default)]
struct RenderParams {
    /// The UI description: a `fenestra/1` JSON object.
    description: Value,
    /// Window size as `WxH` (default `800x600`).
    #[serde(default)]
    size: Option<String>,
    /// Optional theme: a `ThemeSpec` object, or `{"preset":"dark"}`.
    #[serde(default)]
    theme: Option<Value>,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
struct QueryParams {
    /// The UI description: a `fenestra/1` JSON object.
    description: Value,
    /// Selector: `{"role":"button","name":"Add"}` (role/name/value/id).
    selector: Value,
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    theme: Option<Value>,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
struct InteractParams {
    /// The UI description: a `fenestra/1` JSON object.
    description: Value,
    /// An array of interaction steps, e.g. `[{"click":{"role":"button","name":"Add"}}]`.
    steps: Value,
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    theme: Option<Value>,
    /// Attach a preview image of the UI after the steps.
    #[serde(default)]
    screenshot: bool,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
struct CheckParams {
    /// The UI description: a `fenestra/1` JSON object.
    description: Value,
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    theme: Option<Value>,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
struct MatchAriaParams {
    /// The UI description: a `fenestra/1` JSON object.
    description: Value,
    /// The expected aria snapshot.
    expected: String,
    /// `partial` (default) | `strict` | `regex`.
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    theme: Option<Value>,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
struct MatchScreenshotParams {
    /// The UI description: a `fenestra/1` JSON object.
    description: Value,
    /// Path to the baseline PNG on disk.
    baseline_path: String,
    /// Per-channel tolerance (0 = exact).
    #[serde(default)]
    tolerance: u8,
    /// Allowed differing-pixel fraction.
    #[serde(default)]
    budget: f64,
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    theme: Option<Value>,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
struct ValidateParams {
    /// The UI description: a `fenestra/1` JSON object.
    description: Value,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
struct RunScenarioParams {
    /// The scenario: a `fenestra/1` description, optional `steps`, and an `expect`
    /// bundle (emitted/a11y/aria/screenshot/queries).
    scenario: Value,
}

fn default_mode() -> String {
    "partial".to_string()
}

// --------------------------------------------------------------- helpers

/// Parses the description value into a `Description`.
fn parse_desc(value: &Value) -> Result<Description, ErrorData> {
    serde_json::from_value(value.clone())
        .map_err(|e| ErrorData::invalid_params(format!("invalid description: {e}"), None))
}

/// Resolves the optional theme value.
fn theme_of(value: Option<&Value>) -> Result<Theme, ErrorData> {
    resolve_theme(value).map_err(|m| ErrorData::invalid_params(m, None))
}

/// Parses a `WxH` size, defaulting to 800x600.
fn parse_size(s: Option<&str>) -> Result<(u32, u32), ErrorData> {
    let Some(s) = s else {
        return Ok((800, 600));
    };
    s.split_once(['x', 'X'])
        .and_then(|(w, h)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?)))
        .ok_or_else(|| {
            ErrorData::invalid_params(
                format!("invalid size {s:?}; expected WxH like 800x600"),
                None,
            )
        })
}

/// Runs blocking (GPU) work off the async runtime.
async fn blocking<F, T>(f: F) -> Result<T, ErrorData>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| ErrorData::internal_error(format!("render task failed: {e}"), None))
}

/// Maps an engine error to a protocol error (the message is self-explaining and
/// includes the access tree for selector misses).
fn engine_err(e: EngineError) -> ErrorData {
    match e {
        EngineError::Parse(_) | EngineError::Step { .. } | EngineError::Scenario(_) => {
            ErrorData::invalid_params(e.to_string(), None)
        }
    }
}

/// Maps describe parse errors to a protocol error with a structured `data`.
fn map_parse(errs: Vec<DescribeError>) -> ErrorData {
    let message = errs
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ");
    ErrorData::invalid_params(message, Some(errs_to_value(&errs)))
}

/// Serializes describe errors to a JSON array of `{path, message}`.
fn errs_to_value(errs: &[DescribeError]) -> Value {
    Value::Array(
        errs.iter()
            .map(|e| json!({ "path": e.path, "message": e.message }))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> Value {
        json!({ "schema": "fenestra/1", "root": { "button": { "label": "Go", "on_click": "go" } } })
    }

    #[test]
    fn tool_list_has_all_ten_with_schemas() {
        let tools = FenestraServer::tool_router().list_all();
        let names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
        for expected in [
            "render_ui",
            "query_ui",
            "interact",
            "check_a11y",
            "focus_order",
            "match_aria_snapshot",
            "match_screenshot",
            "describe_vocabulary",
            "validate",
            "run_scenario",
        ] {
            assert!(
                names.iter().any(|n| n == expected),
                "missing {expected}; have {names:?}"
            );
        }
        assert_eq!(names.len(), 10, "exactly ten tools");
        // Every tool advertises an input schema object.
        for t in &tools {
            assert!(!t.input_schema.is_empty(), "{} has no input schema", t.name);
        }
        // The typed (Json<T>) tools advertise an output schema, so a client knows
        // the shape of the structured result before calling.
        let with_output: Vec<&str> = tools
            .iter()
            .filter(|t| t.output_schema.is_some())
            .map(|t| t.name.as_ref())
            .collect();
        for expected in [
            "query_ui",
            "check_a11y",
            "focus_order",
            "match_aria_snapshot",
            "describe_vocabulary",
        ] {
            assert!(
                with_output.contains(&expected),
                "{expected} should advertise an output schema; have {with_output:?}"
            );
        }
    }

    #[tokio::test]
    async fn validate_discriminates_good_and_bad() {
        let s = FenestraServer::new();
        let ok = s
            .validate(Parameters(ValidateParams {
                description: good(),
            }))
            .await
            .unwrap();
        assert_ne!(
            ok.is_error,
            Some(true),
            "a valid description is not an error"
        );

        let bad = json!({ "schema": "fenestra/1", "root": { "col": { "kids": [] } } });
        let bad = s
            .validate(Parameters(ValidateParams { description: bad }))
            .await
            .unwrap();
        assert_eq!(
            bad.is_error,
            Some(true),
            "an invalid description is isError"
        );
    }

    #[tokio::test]
    async fn render_ui_leads_with_tree_then_image() {
        let s = FenestraServer::new();
        let r = s
            .render_ui(Parameters(RenderParams {
                description: good(),
                size: Some("300x120".to_string()),
                theme: None,
            }))
            .await
            .unwrap();
        assert_ne!(r.is_error, Some(true));
        let structured = r.structured_content.as_ref().expect("structured content");
        assert!(
            structured.get("tree").is_some(),
            "tree is in the structured result"
        );
        assert!(
            r.content.len() >= 3,
            "a text block, an inline preview, and a full-res resource_link"
        );
        // The full-resolution render comes back as a resource_link, not base64.
        let link = r
            .content
            .iter()
            .find_map(|c| c.as_resource_link())
            .expect("a resource_link to the full-resolution PNG");
        assert!(link.uri.starts_with("file://"), "uri: {}", link.uri);
        assert_eq!(link.mime_type.as_deref(), Some("image/png"));
    }

    #[tokio::test]
    async fn malformed_description_is_a_protocol_error() {
        let s = FenestraServer::new();
        let r = s
            .render_ui(Parameters(RenderParams {
                description: json!({ "nope": 1 }),
                size: None,
                theme: None,
            }))
            .await;
        assert!(
            r.is_err(),
            "a malformed description is an ErrorData, not a result"
        );
    }

    #[test]
    fn server_identifies_as_fenestra() {
        let info = FenestraServer::new().get_info();
        assert_eq!(info.server_info.name, "fenestra-mcp");
    }

    #[tokio::test]
    async fn run_scenario_passes_and_fails_as_normal_results() {
        let s = FenestraServer::new();

        // A passing scenario: a labeled button clears a11y and the button query.
        let pass = json!({
            "schema": "fenestra/1",
            "description": good(),
            "size": "300x120",
            "expect": { "a11y": true, "queries": [ { "selector": { "role": "button" }, "count": 1 } ] }
        });
        let r = s
            .run_scenario(Parameters(RunScenarioParams { scenario: pass }))
            .await
            .unwrap();
        assert_ne!(r.is_error, Some(true), "a passing scenario is not an error");
        let structured = r.structured_content.as_ref().expect("structured content");
        assert_eq!(
            structured.get("ok").and_then(serde_json::Value::as_bool),
            Some(true),
            "{structured}"
        );

        // A failing check (wrong query count) is a normal result, not isError.
        let fail = json!({
            "schema": "fenestra/1",
            "description": good(),
            "size": "300x120",
            "expect": { "queries": [ { "selector": { "role": "button" }, "count": 5 } ] }
        });
        let r = s
            .run_scenario(Parameters(RunScenarioParams { scenario: fail }))
            .await
            .unwrap();
        assert_ne!(
            r.is_error,
            Some(true),
            "a failed check is a normal result, not a protocol error"
        );
        let structured = r.structured_content.as_ref().expect("structured content");
        assert_eq!(
            structured.get("ok").and_then(serde_json::Value::as_bool),
            Some(false),
            "{structured}"
        );
    }
}
