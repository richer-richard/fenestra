//! The fenestra MCP server: eight tools that render and verify a UI described as
//! `fenestra/1` JSON. Each tool leads with a typed structured result (the access
//! tree, a report, a diff); visual tools also attach a downscaled preview image.
//!
//! Error channel: a malformed description or selector is a protocol error
//! (`ErrorData::invalid_params`); a render/GPU failure surfaces as an internal
//! error; a *verification mismatch* (a failed aria/screenshot/a11y check) is a
//! normal successful result the agent reads — not an error.

use fenestra_cli::engine::{self, EngineError, Step};
use fenestra_cli::resolve_theme;
use fenestra_core::Theme;
use fenestra_describe as describe;
use fenestra_describe::error::DescribeError;
use fenestra_describe::format::Description;
use fenestra_describe::inspect::{self, AriaMode, Selector};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::{ErrorData, ServerHandler, model::CallToolResult, tool, tool_handler, tool_router};
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
        description = "Render a fenestra/1 UI description to a typed accessibility tree, a downscaled preview image, and automatic accessibility warnings (contrast, labeling, per-text-node legibility). Read the access tree first; the full-resolution PNG path is in the structured result."
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
        let full = content::save_full(&out.png);
        let structured = json!({
            "tree": out.tree,
            "warnings": out.warnings,
            "full_resolution_png": full,
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
    ) -> Result<CallToolResult, ErrorData> {
        let desc = parse_desc(&p.description)?;
        let theme = theme_of(p.theme.as_ref())?;
        let size = parse_size(p.size.as_deref())?;
        let selector: Selector = serde_json::from_value(p.selector.clone())
            .map_err(|e| ErrorData::invalid_params(format!("invalid selector: {e}"), None))?;
        let result = inspect::query(&desc, &theme, size, &selector).map_err(map_parse)?;
        let text = format!(
            "{} match(es), {} nearest candidate(s)",
            result.matches.len(),
            result.nearest.len()
        );
        Ok(content::ok(text, to_value(&result), None))
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
        let full = out.png.as_ref().and_then(content::save_full);
        let structured = json!({
            "emitted": out.emitted,
            "tree": out.tree,
            "full_resolution_png": full,
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
        description = "Check accessibility of a UI from the real render: theme contrast, labeling of every interactive control, and per-text-node APCA + WCAG 2 legibility. Returns a structured report; this is a normal result whether or not it passes."
    )]
    async fn check_a11y(
        &self,
        Parameters(p): Parameters<CheckParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let desc = parse_desc(&p.description)?;
        let theme = theme_of(p.theme.as_ref())?;
        let size = parse_size(p.size.as_deref())?;
        let report = inspect::check_a11y(&desc, &theme, size).map_err(map_parse)?;
        let text = format!(
            "legible: {} · unlabeled: {} · contrast violations: {} · text nodes measured: {}",
            report.legible,
            report.unlabeled.len(),
            report.contrast_violations.len(),
            report.node_legibility.len(),
        );
        Ok(content::ok(text, to_value(&report), None))
    }

    #[tool(
        name = "match_aria_snapshot",
        description = "Match an expected accessibility snapshot (Playwright `- role \"name\" [attr]` grammar) against a UI. mode: partial (subset, default) | strict (exact) | regex (each expected line is a pattern). Returns a pass/fail diff — a normal result."
    )]
    async fn match_aria_snapshot(
        &self,
        Parameters(p): Parameters<MatchAriaParams>,
    ) -> Result<CallToolResult, ErrorData> {
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
        let text = if diff.ok {
            "match".to_string()
        } else {
            format!("mismatch:\n{}", diff.diff)
        };
        Ok(content::ok(text, to_value(&diff), None))
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
        let diff_path = diff.diff_png.as_ref().and_then(content::save_full);
        let structured = json!({
            "ok": diff.ok,
            "differing": diff.differing,
            "total": diff.total,
            "max_delta": diff.max_delta,
            "worst": [diff.worst.0, diff.worst.1],
            "diff_png": diff_path,
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
    async fn describe_vocabulary(&self) -> Result<CallToolResult, ErrorData> {
        let vocab = describe::vocabulary::describe_vocabulary();
        let text = format!(
            "{} node types, {} color roles",
            vocab.nodes.len(),
            vocab.color_roles.len()
        );
        Ok(content::ok(text, to_value(&vocab), None))
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
             accessibility warnings; query_ui, check_a11y, match_aria_snapshot, and \
             match_screenshot to assert; interact to drive it; validate to check a \
             description without rendering."
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
    description: Value,
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    theme: Option<Value>,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
struct MatchAriaParams {
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
    description: Value,
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
        EngineError::Parse(_) | EngineError::Step { .. } => {
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

/// Serializes a DTO to a JSON value (DTOs always serialize).
fn to_value<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> Value {
        json!({ "schema": "fenestra/1", "root": { "button": { "label": "Go", "on_click": "go" } } })
    }

    #[test]
    fn tool_list_has_all_eight_with_schemas() {
        let tools = FenestraServer::tool_router().list_all();
        let names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
        for expected in [
            "render_ui",
            "query_ui",
            "interact",
            "check_a11y",
            "match_aria_snapshot",
            "match_screenshot",
            "describe_vocabulary",
            "validate",
        ] {
            assert!(
                names.iter().any(|n| n == expected),
                "missing {expected}; have {names:?}"
            );
        }
        assert_eq!(names.len(), 8, "exactly eight tools");
        // Every tool advertises an input schema object.
        for t in &tools {
            assert!(!t.input_schema.is_empty(), "{} has no input schema", t.name);
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
        assert!(r.content.len() >= 2, "a text block and a preview image");
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
}
