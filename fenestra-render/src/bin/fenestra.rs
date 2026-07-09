//! The `fenestra` command-line front door: render and verify a UI described as
//! JSON. Each subcommand reads a description from a path (or stdin with `-`),
//! writes its result as JSON to stdout, any image to `--out`, and signals the
//! outcome through the exit code: `0` ok, `1` a verification failed, `3` a parse
//! or IO error (clap uses `2` for usage errors). The exception is `preview`,
//! which opens a live-reload window against a real file path — there's
//! nothing to reload from stdin, so it doesn't follow that convention.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use fenestra_core::Theme;
use fenestra_describe::dto::Bounds;
use fenestra_describe::format::{Description, description_schema};
use fenestra_describe::inspect::{
    AriaMode, Selector, check_a11y, focus_order, layout_report, match_aria, query,
};
use fenestra_describe::parse::validate;
use fenestra_describe::vocabulary::describe_vocabulary;
use fenestra_render::engine::{Step, interact, match_screenshot, render, validate_masks};
use fenestra_render::scenario::{Scenario, bless, verify};
use fenestra_render::{PreviewApp, resolve_theme};
use fenestra_shell::{WindowOptions, run_app};
use serde_json::json;

#[derive(Parser)]
#[command(
    name = "fenestra",
    version,
    about = "Render and verify fenestra UIs described as JSON"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Render to an access tree, pixels, and accessibility warnings.
    Render {
        /// Description path, or `-`/omitted for stdin.
        desc: Option<PathBuf>,
        /// Window size, `WxH`.
        #[arg(long, default_value = "800x600")]
        size: String,
        /// Theme JSON path (`ThemeSpec` or `{"preset":"dark"}`).
        #[arg(long)]
        theme: Option<PathBuf>,
        /// Write the rendered PNG here.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Query the access tree by a semantic selector.
    Query {
        desc: Option<PathBuf>,
        /// Selector JSON, e.g. `{"role":"button","name":"Add"}`.
        #[arg(long)]
        selector: String,
        #[arg(long, default_value = "800x600")]
        size: String,
        #[arg(long)]
        theme: Option<PathBuf>,
    },
    /// Drive scripted interactions; report emitted intents and the after-tree.
    Interact {
        desc: Option<PathBuf>,
        /// Steps JSON: an array of interaction steps.
        #[arg(long)]
        steps: PathBuf,
        #[arg(long, default_value = "800x600")]
        size: String,
        #[arg(long)]
        theme: Option<PathBuf>,
        /// Write the after-interaction PNG here.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Check accessibility: contrast, labeling, and per-node legibility.
    Check {
        desc: Option<PathBuf>,
        #[arg(long, default_value = "800x600")]
        size: String,
        #[arg(long)]
        theme: Option<PathBuf>,
        /// Also fail on any strict per-node text-contrast failure (body-text floor).
        #[arg(long)]
        strict: bool,
    },
    /// List the keyboard focus order: the refs a Tab cycle visits, in order.
    FocusOrder {
        desc: Option<PathBuf>,
        #[arg(long, default_value = "800x600")]
        size: String,
        #[arg(long)]
        theme: Option<PathBuf>,
    },
    /// Report layout problems: small hit targets and off-screen nodes.
    Layout {
        desc: Option<PathBuf>,
        #[arg(long, default_value = "800x600")]
        size: String,
        #[arg(long)]
        theme: Option<PathBuf>,
    },
    /// Match an expected aria snapshot.
    MatchAria {
        desc: Option<PathBuf>,
        /// Expected snapshot path.
        #[arg(long)]
        expected: PathBuf,
        /// `partial` | `strict` | `regex`.
        #[arg(long, default_value = "partial")]
        mode: String,
        #[arg(long, default_value = "800x600")]
        size: String,
        #[arg(long)]
        theme: Option<PathBuf>,
    },
    /// Match a baseline screenshot.
    MatchPng {
        desc: Option<PathBuf>,
        /// Baseline PNG path.
        #[arg(long)]
        baseline: PathBuf,
        /// Per-channel tolerance (0 = exact).
        #[arg(long, default_value_t = 0)]
        tolerance: u8,
        /// Allowed differing-pixel fraction.
        #[arg(long, default_value_t = 0.0)]
        budget: f64,
        /// Ignore this rectangle when comparing (repeatable): `x,y,w,h` in
        /// logical pixels, e.g. `--mask 10,10,80,20`.
        #[arg(long = "mask", value_name = "X,Y,W,H")]
        masks: Vec<String>,
        #[arg(long, default_value = "800x600")]
        size: String,
        #[arg(long)]
        theme: Option<PathBuf>,
        /// Write the diff PNG here (on mismatch).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Print the description grammar (node types, color roles).
    Vocabulary,
    /// Print the JSON Schema for the fenestra/1 description format.
    Schema,
    /// Validate a description without rendering.
    Validate { desc: Option<PathBuf> },
    /// Run a scenario: drive its steps, assert every expectation, one verdict.
    /// The screenshot check compares the *post-interaction* pixels — the closed
    /// verify loop.
    Verify {
        /// Scenario path, or `-`/omitted for stdin.
        scenario: Option<PathBuf>,
        /// Write the diff PNG here (on a screenshot mismatch).
        #[arg(long)]
        out: Option<PathBuf>,
        /// (Re)write the scenario's screenshot baseline from the current render
        /// instead of verifying against it.
        #[arg(long)]
        bless: bool,
    },
    /// Open a live-reload preview window for a description file: edit and
    /// save, and the window updates. A broken edit shows a themed error
    /// panel over the last good view instead of crashing or going blank.
    Preview {
        /// Description path to preview (a real file, not stdin — there's
        /// nothing to reload from a pipe).
        desc: PathBuf,
        /// Initial window size, `WxH`.
        #[arg(long, default_value = "800x600")]
        size: String,
        /// Theme JSON path (`ThemeSpec` or `{"preset":"dark"}`).
        #[arg(long)]
        theme: Option<PathBuf>,
    },
}

const EXIT_VERIFY_FAILED: u8 = 1;
const EXIT_ERROR: u8 = 3;

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Render {
            desc,
            size,
            theme,
            out,
        } => cmd_render(desc, &size, theme, out),
        Command::Query {
            desc,
            selector,
            size,
            theme,
        } => cmd_query(desc, &selector, &size, theme),
        Command::Interact {
            desc,
            steps,
            size,
            theme,
            out,
        } => cmd_interact(desc, &steps, &size, theme, out),
        Command::Check {
            desc,
            size,
            theme,
            strict,
        } => cmd_check(desc, &size, theme, strict),
        Command::FocusOrder { desc, size, theme } => cmd_focus_order(desc, &size, theme),
        Command::Layout { desc, size, theme } => cmd_layout(desc, &size, theme),
        Command::MatchAria {
            desc,
            expected,
            mode,
            size,
            theme,
        } => cmd_match_aria(desc, &expected, &mode, &size, theme),
        Command::MatchPng {
            desc,
            baseline,
            tolerance,
            budget,
            masks,
            size,
            theme,
            out,
        } => cmd_match_png(
            desc,
            &baseline,
            (tolerance, budget, &masks),
            &size,
            theme,
            out,
        ),
        Command::Vocabulary => cmd_vocabulary(),
        Command::Schema => cmd_schema(),
        Command::Validate { desc } => cmd_validate(desc),
        Command::Verify {
            scenario,
            out,
            bless,
        } => cmd_verify(scenario, out.as_deref(), bless),
        Command::Preview { desc, size, theme } => cmd_preview(desc, &size, theme),
    }
}

// --------------------------------------------------------------- commands

fn cmd_render(
    desc: Option<PathBuf>,
    size: &str,
    theme: Option<PathBuf>,
    out: Option<PathBuf>,
) -> ExitCode {
    let (desc, theme, size) = match common(desc, size, theme) {
        Ok(v) => v,
        Err(c) => return c,
    };
    match render(&desc, &theme, size) {
        Ok(r) => {
            print_json(&json!({ "tree": r.tree, "warnings": r.warnings }));
            save_png_opt(&r.png, out.as_deref())
        }
        Err(e) => fail(&e),
    }
}

fn cmd_query(
    desc: Option<PathBuf>,
    selector: &str,
    size: &str,
    theme: Option<PathBuf>,
) -> ExitCode {
    let (desc, theme, size) = match common(desc, size, theme) {
        Ok(v) => v,
        Err(c) => return c,
    };
    let selector: Selector = match serde_json::from_str(selector) {
        Ok(s) => s,
        Err(e) => return err(&format!("invalid selector json: {e}")),
    };
    match query(&desc, &theme, size, &selector) {
        Ok(result) => {
            print_json(&result);
            ExitCode::SUCCESS
        }
        Err(errs) => fail_parse(&errs),
    }
}

fn cmd_interact(
    desc: Option<PathBuf>,
    steps: &Path,
    size: &str,
    theme: Option<PathBuf>,
    out: Option<PathBuf>,
) -> ExitCode {
    let (desc, theme, size) = match common(desc, size, theme) {
        Ok(v) => v,
        Err(c) => return c,
    };
    let steps_json = match fs::read_to_string(steps) {
        Ok(s) => s,
        Err(e) => return err(&format!("error reading steps: {e}")),
    };
    let steps: Vec<Step> = match serde_json::from_str(&steps_json) {
        Ok(s) => s,
        Err(e) => return err(&format!("invalid steps json: {e}")),
    };
    match interact(&desc, &theme, size, &steps, out.is_some()) {
        Ok(r) => {
            print_json(&json!({ "emitted": r.emitted, "tree": r.tree }));
            match (out, r.png) {
                (Some(path), Some(png)) => save_png(&png, &path),
                _ => ExitCode::SUCCESS,
            }
        }
        Err(e) => fail(&e),
    }
}

fn cmd_check(desc: Option<PathBuf>, size: &str, theme: Option<PathBuf>, strict: bool) -> ExitCode {
    let (desc, theme, size) = match common(desc, size, theme) {
        Ok(v) => v,
        Err(c) => return c,
    };
    match check_a11y(&desc, &theme, size) {
        Ok(report) => {
            print_json(&report);
            let strict_ok = !strict || report.text_contrast_failures.is_empty();
            if report.legible && report.unlabeled.is_empty() && strict_ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(EXIT_VERIFY_FAILED)
            }
        }
        Err(errs) => fail_parse(&errs),
    }
}

fn cmd_match_aria(
    desc: Option<PathBuf>,
    expected: &Path,
    mode: &str,
    size: &str,
    theme: Option<PathBuf>,
) -> ExitCode {
    let (desc, theme, size) = match common(desc, size, theme) {
        Ok(v) => v,
        Err(c) => return c,
    };
    let mode = match mode {
        "partial" => AriaMode::Partial,
        "strict" => AriaMode::Strict,
        "regex" => AriaMode::Regex,
        other => {
            return err(&format!(
                "unknown mode {other:?}; expected partial|strict|regex"
            ));
        }
    };
    let expected = match fs::read_to_string(expected) {
        Ok(s) => s,
        Err(e) => return err(&format!("error reading expected: {e}")),
    };
    match match_aria(&desc, &theme, size, &expected, mode) {
        Ok(diff) => {
            print_json(&diff);
            verdict(diff.ok)
        }
        Err(errs) => fail_parse(&errs),
    }
}

fn cmd_match_png(
    desc: Option<PathBuf>,
    baseline: &Path,
    // (tolerance, budget, `--mask` values) — bundled to stay under clippy's
    // too-many-arguments limit.
    shot: (u8, f64, &[String]),
    size: &str,
    theme: Option<PathBuf>,
    out: Option<PathBuf>,
) -> ExitCode {
    let (tolerance, budget, masks) = shot;
    let (desc, theme, size) = match common(desc, size, theme) {
        Ok(v) => v,
        Err(c) => return c,
    };
    let masks = match parse_masks(masks) {
        Ok(m) => m,
        Err(c) => return c,
    };
    let baseline = match image::open(baseline) {
        Ok(img) => img.into_rgba8(),
        Err(e) => return err(&format!("error reading baseline: {e}")),
    };
    match match_screenshot(&desc, &theme, size, &baseline, tolerance, budget, &masks) {
        Ok(diff) => {
            print_json(&json!({
                "ok": diff.ok,
                "differing": diff.differing,
                "total": diff.total,
                "max_delta": diff.max_delta,
                "worst": [diff.worst.0, diff.worst.1],
            }));
            if let (Some(path), Some(img)) = (out.as_deref(), diff.diff_png.as_ref())
                && let Err(e) = img.save(path)
            {
                return err(&format!("error writing diff: {e}"));
            }
            verdict(diff.ok)
        }
        Err(e) => fail(&e),
    }
}

fn cmd_focus_order(desc: Option<PathBuf>, size: &str, theme: Option<PathBuf>) -> ExitCode {
    let (desc, theme, size) = match common(desc, size, theme) {
        Ok(v) => v,
        Err(c) => return c,
    };
    match focus_order(&desc, &theme, size) {
        Ok(order) => {
            print_json(&order);
            ExitCode::SUCCESS
        }
        Err(errs) => fail_parse(&errs),
    }
}

fn cmd_layout(desc: Option<PathBuf>, size: &str, theme: Option<PathBuf>) -> ExitCode {
    let (desc, theme, size) = match common(desc, size, theme) {
        Ok(v) => v,
        Err(c) => return c,
    };
    match layout_report(&desc, &theme, size) {
        Ok(report) => {
            print_json(&report);
            if report.small_targets.is_empty() && report.offscreen.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(EXIT_VERIFY_FAILED)
            }
        }
        Err(errs) => fail_parse(&errs),
    }
}

fn cmd_vocabulary() -> ExitCode {
    print_json(&describe_vocabulary());
    ExitCode::SUCCESS
}

fn cmd_schema() -> ExitCode {
    print_json(&description_schema());
    ExitCode::SUCCESS
}

fn cmd_validate(desc: Option<PathBuf>) -> ExitCode {
    let json = match read_input(desc.as_deref()) {
        Ok(j) => j,
        Err(e) => return err(&format!("error reading description: {e}")),
    };
    match validate(&json) {
        Ok(()) => {
            println!("ok");
            ExitCode::SUCCESS
        }
        Err(errs) => fail_parse(&errs),
    }
}

fn cmd_verify(scenario: Option<PathBuf>, out: Option<&Path>, do_bless: bool) -> ExitCode {
    let json = match read_input(scenario.as_deref()) {
        Ok(j) => j,
        Err(e) => return err(&format!("error reading scenario: {e}")),
    };
    let scenario: Scenario = match serde_json::from_str(&json) {
        Ok(s) => s,
        Err(e) => return err(&format!("invalid scenario json: {e}")),
    };
    if do_bless {
        return match bless(&scenario) {
            Ok(path) => {
                eprintln!("blessed {}", path.display());
                ExitCode::SUCCESS
            }
            Err(e) => fail(&e),
        };
    }
    match verify(&scenario) {
        Ok(v) => {
            print_json(&v.report);
            if let (Some(path), Some(diff)) = (out, v.diff_png.as_ref()) {
                if let Err(e) = diff.save(path) {
                    return err(&format!("error writing diff: {e}"));
                }
                eprintln!("wrote {}", path.display());
            }
            verdict(v.report.ok)
        }
        Err(e) => fail(&e),
    }
}

/// Opens a live-reload preview window against `desc` (a real path — there's
/// no stdin fallback, since there'd be nothing to reload). Blocks until the
/// window closes.
fn cmd_preview(desc: PathBuf, size: &str, theme: Option<PathBuf>) -> ExitCode {
    let theme = match load_theme(theme.as_deref()) {
        Ok(t) => t,
        Err(c) => return c,
    };
    let (width, height) = match parse_size(size) {
        Ok(v) => v,
        Err(c) => return c,
    };
    let app = PreviewApp::new(desc, theme);
    let options =
        WindowOptions::titled(app.window_title()).with_size(f64::from(width), f64::from(height));
    match run_app(app, options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => err(&format!("error running preview: {e}")),
    }
}

// ---------------------------------------------------------------- helpers

/// Loads the description, theme, and size shared by most subcommands.
fn common(
    desc: Option<PathBuf>,
    size: &str,
    theme: Option<PathBuf>,
) -> Result<(Description, Theme, (u32, u32)), ExitCode> {
    Ok((
        load_desc(desc.as_deref())?,
        load_theme(theme.as_deref())?,
        parse_size(size)?,
    ))
}

/// Reads a description from a path (or stdin) and parses it, reporting
/// path-pointed problems on failure.
fn load_desc(path: Option<&Path>) -> Result<Description, ExitCode> {
    let json = read_input(path).map_err(|e| err(&format!("error reading description: {e}")))?;
    match serde_json::from_str::<Description>(&json) {
        Ok(desc) => Ok(desc),
        Err(_) => {
            if let Err(errs) = validate(&json) {
                for e in &errs {
                    eprintln!("error: {e}");
                }
            }
            Err(ExitCode::from(EXIT_ERROR))
        }
    }
}

/// Resolves the theme argument to a concrete theme.
fn load_theme(path: Option<&Path>) -> Result<Theme, ExitCode> {
    let Some(path) = path else {
        return Ok(Theme::light());
    };
    let json = fs::read_to_string(path).map_err(|e| err(&format!("error reading theme: {e}")))?;
    let value: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| err(&format!("invalid theme json: {e}")))?;
    resolve_theme(Some(&value)).map_err(|m| err(&format!("invalid theme: {m}")))
}

/// Reads from a path, or stdin when the path is `-` or absent.
fn read_input(path: Option<&Path>) -> io::Result<String> {
    match path {
        Some(p) if p != Path::new("-") => fs::read_to_string(p),
        _ => {
            let mut s = String::new();
            io::stdin().read_to_string(&mut s)?;
            Ok(s)
        }
    }
}

/// Parses a `WxH` size string.
fn parse_size(s: &str) -> Result<(u32, u32), ExitCode> {
    if let Some((w, h)) = s.split_once(['x', 'X'])
        && let (Ok(w), Ok(h)) = (w.trim().parse(), h.trim().parse())
    {
        return Ok((w, h));
    }
    Err(err(&format!(
        "invalid size {s:?}; expected WxH like 800x600"
    )))
}

/// Parses one `--mask` value (`x,y,w,h`, logical px) into a `Bounds`.
fn parse_mask(s: &str) -> Result<Bounds, String> {
    let parts: Vec<&str> = s.split(',').map(str::trim).collect();
    let [x, y, w, h] = parts.as_slice() else {
        return Err(format!(
            "invalid --mask {s:?}; expected x,y,w,h, got {} field(s)",
            parts.len()
        ));
    };
    let field = |name: &str, v: &str| -> Result<f64, String> {
        v.parse::<f64>()
            .map_err(|e| format!("invalid --mask {s:?}: {name}={v:?} ({e})"))
    };
    Ok(Bounds {
        x: field("x", x)?,
        y: field("y", y)?,
        w: field("w", w)?,
        h: field("h", h)?,
    })
}

/// Parses every `--mask` value and rejects hostile rectangles (non-finite
/// coordinates, negative width/height) before they reach the engine.
fn parse_masks(masks: &[String]) -> Result<Vec<Bounds>, ExitCode> {
    let parsed: Vec<Bounds> = masks
        .iter()
        .map(|s| parse_mask(s))
        .collect::<Result<_, _>>()
        .map_err(|e| err(&e))?;
    validate_masks(&parsed).map_err(|e| err(&e))?;
    Ok(parsed)
}

/// Prints a value as pretty JSON to stdout.
fn print_json<T: serde::Serialize>(value: &T) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("DTOs serialize")
    );
}

/// Saves a PNG to `path`, mapping IO errors to the error exit code.
fn save_png(png: &image::RgbaImage, path: &Path) -> ExitCode {
    match png.save(path) {
        Ok(()) => {
            eprintln!("wrote {}", path.display());
            ExitCode::SUCCESS
        }
        Err(e) => err(&format!("error writing {}: {e}", path.display())),
    }
}

/// Saves a PNG when a path is given.
fn save_png_opt(png: &image::RgbaImage, path: Option<&Path>) -> ExitCode {
    match path {
        Some(p) => save_png(png, p),
        None => ExitCode::SUCCESS,
    }
}

/// Prints a message to stderr and returns the error exit code.
fn err(message: &str) -> ExitCode {
    eprintln!("{message}");
    ExitCode::from(EXIT_ERROR)
}

/// Reports an engine error and returns the error exit code.
fn fail(e: &fenestra_render::engine::EngineError) -> ExitCode {
    eprintln!("{e}");
    ExitCode::from(EXIT_ERROR)
}

/// Reports parse errors and returns the error exit code.
fn fail_parse(errs: &[fenestra_describe::error::DescribeError]) -> ExitCode {
    for e in errs {
        eprintln!("error: {e}");
    }
    ExitCode::from(EXIT_ERROR)
}

/// Maps a verification outcome to an exit code.
fn verdict(ok: bool) -> ExitCode {
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_VERIFY_FAILED)
    }
}
