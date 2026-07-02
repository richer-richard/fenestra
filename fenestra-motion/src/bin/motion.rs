//! The `motion` command-line front door: render and probe a composition
//! described as RON (or JSON). Follows the `fenestra` CLI conventions: the
//! document comes from a path (or stdin with `-`), results print as JSON to
//! stdout, artifacts go to `--out`, notes to stderr, and the exit code
//! signals the outcome: `0` ok, `1` a verification failed, `3` a parse or
//! IO error (clap uses `2` for usage errors).

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use fenestra_motion::{Composition, Frames, ResolvedClip};
use serde_json::json;

/// Exit code for a failed verification (reserved for `lint`).
const EXIT_VERIFY_FAILED: u8 = 1;
/// Exit code for parse and IO errors.
const EXIT_ERROR: u8 = 3;

#[derive(Parser)]
#[command(
    name = "motion",
    version,
    about = "Render and verify fenestra motion compositions"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Render frames: `--frame N` for one PNG, `--frames a..b --out dir/`
    /// for a sequence, `--mp4 out.mp4` for a video (needs ffmpeg on PATH).
    Render {
        /// Composition path (`.ron` / `.json`), or `-`/omitted for stdin.
        comp: Option<PathBuf>,
        /// Half-open frame range `a..b` (defaults to the whole timeline).
        #[arg(long)]
        frames: Option<String>,
        /// Render exactly this frame to `--out` as a single PNG.
        #[arg(long, conflicts_with = "frames")]
        frame: Option<u64>,
        /// Preview scale for `--frame` (0.25 renders 1/16 of the pixels).
        #[arg(long, requires = "frame")]
        scale: Option<f64>,
        /// Output: a PNG path with `--frame`, a directory for a sequence.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Encode a video via the ffmpeg pipe instead of writing PNGs.
        #[arg(long, conflicts_with_all = ["frame", "out"])]
        mp4: Option<PathBuf>,
    },
    /// Resolve props and bboxes at a frame; JSON to stdout.
    Probe {
        /// Composition path (`.ron` / `.json`), or `-`/omitted for stdin.
        comp: Option<PathBuf>,
        /// The frame to sample.
        #[arg(long)]
        frame: u64,
        /// Restrict the report to one clip id.
        #[arg(long)]
        clip: Option<String>,
    },
    /// Run the temporal lints (undeclared jumps); exit 1 on problems.
    Lint {
        /// Composition path (`.ron` / `.json`), or `-`/omitted for stdin.
        comp: Option<PathBuf>,
        /// Override every per-prop jump threshold with one value.
        #[arg(long)]
        eps: Option<f32>,
    },
    /// Tile every Nth frame into one labeled contact sheet PNG.
    Sheet {
        /// Composition path (`.ron` / `.json`), or `-`/omitted for stdin.
        comp: Option<PathBuf>,
        /// Thumbnail stride in frames.
        #[arg(long, default_value_t = 30)]
        every: u64,
        /// Thumbnail width in px.
        #[arg(long, default_value_t = 320)]
        thumb_width: u32,
        /// Where to write the sheet.
        #[arg(long)]
        out: PathBuf,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode, String> {
    match cli.command {
        Command::Render {
            comp,
            frames,
            frame,
            scale,
            out,
            mp4,
        } => {
            let comp = load(comp.as_deref())?;
            render(&comp, frames.as_deref(), frame, scale, out, mp4)
        }
        Command::Probe { comp, frame, clip } => {
            let comp = load(comp.as_deref())?;
            probe(&comp, Frames(frame), clip.as_deref())
        }
        Command::Lint { comp, eps } => {
            let comp = load(comp.as_deref())?;
            let problems = fenestra_motion::verify::discontinuities(&comp, eps);
            print_json(&json!({
                "frames": comp.total_frames().0,
                "problems": problems,
            }));
            Ok(if problems.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(EXIT_VERIFY_FAILED)
            })
        }
        Command::Sheet {
            comp,
            every,
            thumb_width,
            out,
        } => {
            let comp = load(comp.as_deref())?;
            let sheet = comp
                .contact_sheet(every, thumb_width)
                .map_err(|e| e.to_string())?;
            sheet
                .save(&out)
                .map_err(|e| format!("write {}: {e}", out.display()))?;
            eprintln!("wrote {}", out.display());
            print_json(&json!({
                "wrote": out,
                "every": every,
                "size": [sheet.width(), sheet.height()],
            }));
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// Loads a composition from a path or stdin: `.json` parses as JSON,
/// everything else tries RON first (the primary form), then JSON.
fn load(path: Option<&std::path::Path>) -> Result<Composition, String> {
    let (src, is_json) = match path {
        Some(p) if p.as_os_str() != "-" => {
            let src =
                std::fs::read_to_string(p).map_err(|e| format!("read {}: {e}", p.display()))?;
            let is_json = p
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("json"));
            (src, is_json)
        }
        _ => {
            let mut src = String::new();
            std::io::stdin()
                .read_to_string(&mut src)
                .map_err(|e| format!("read stdin: {e}"))?;
            (src, false)
        }
    };
    if is_json {
        return Composition::from_json(&src).map_err(|e| e.to_string());
    }
    Composition::from_ron(&src)
        .or_else(|ron_err| Composition::from_json(&src).map_err(|_| ron_err.to_string()))
}

fn parse_range(spec: Option<&str>, comp: &Composition) -> Result<(u64, u64), String> {
    match spec {
        None => Ok((0, comp.total_frames().0)),
        Some(s) => {
            let (a, b) = s
                .split_once("..")
                .ok_or_else(|| format!("--frames wants `a..b`, got {s:?}"))?;
            let a = a
                .parse::<u64>()
                .map_err(|e| format!("--frames start: {e}"))?;
            let b = b.parse::<u64>().map_err(|e| format!("--frames end: {e}"))?;
            if b <= a {
                return Err(format!("--frames {s:?} is empty (end must exceed start)"));
            }
            Ok((a, b))
        }
    }
}

fn render(
    comp: &Composition,
    frames: Option<&str>,
    frame: Option<u64>,
    scale: Option<f64>,
    out: Option<PathBuf>,
    mp4: Option<PathBuf>,
) -> Result<ExitCode, String> {
    if let Some(n) = frame {
        let out = out.ok_or("--frame needs --out <file.png>")?;
        let img = comp
            .render_frame_at(Frames(n), scale.unwrap_or(1.0))
            .map_err(|e| e.to_string())?;
        img.save(&out)
            .map_err(|e| format!("write {}: {e}", out.display()))?;
        eprintln!("wrote {}", out.display());
        print_json(&json!({
            "wrote": out,
            "frame": n,
            "size": [img.width(), img.height()],
        }));
        return Ok(ExitCode::SUCCESS);
    }
    let (a, b) = parse_range(frames, comp)?;
    if let Some(mp4) = mp4 {
        comp.render_video(a..b, &mp4).map_err(|e| e.to_string())?;
        eprintln!("wrote {}", mp4.display());
        print_json(&json!({ "wrote": mp4, "frames": b - a, "fps": comp.fps() }));
        return Ok(ExitCode::SUCCESS);
    }
    let dir = out.ok_or("sequence rendering needs --out <dir> (or use --mp4)")?;
    comp.render_png_sequence(a..b, &dir)
        .map_err(|e| e.to_string())?;
    eprintln!("wrote {} frames to {}", b - a, dir.display());
    print_json(&json!({ "wrote": dir, "frames": b - a, "first": a, "fps": comp.fps() }));
    Ok(ExitCode::SUCCESS)
}

fn probe(comp: &Composition, frame: Frames, only: Option<&str>) -> Result<ExitCode, String> {
    let scene = comp.sample(frame);
    let ids: Vec<String> = match only {
        Some(id) => {
            if scene.resolve(id).is_none() {
                return Err(format!(
                    "no clip {id:?} in this composition (clips: {:?})",
                    comp.clip_ids()
                ));
            }
            vec![id.to_string()]
        }
        None => comp.clip_ids().iter().map(|s| s.to_string()).collect(),
    };
    let clips: Vec<serde_json::Value> = ids
        .iter()
        .map(|id| {
            let r = scene.resolve(id).expect("ids come from the composition");
            clip_json(id, &r)
        })
        .collect();
    print_json(&json!({
        "frame": frame.0,
        "fps": comp.fps(),
        "size": [comp.width(), comp.height()],
        "paint_order": scene.paint_order(),
        "clips": clips,
    }));
    Ok(ExitCode::SUCCESS)
}

fn clip_json(id: &str, r: &ResolvedClip) -> serde_json::Value {
    let color = |c: Option<fenestra_core::Color>| {
        c.map(|c| json!(c.components.to_vec()))
            .unwrap_or(serde_json::Value::Null)
    };
    json!({
        "id": id,
        "visible": r.visible,
        "bbox": r.bbox.map(|b| json!({
            "x": b.x0, "y": b.y0, "w": b.width(), "h": b.height(),
        })).unwrap_or(serde_json::Value::Null),
        "props": {
            "opacity": r.props.opacity,
            "translate": [r.props.translate.0, r.props.translate.1],
            "scale": r.props.scale,
            "scale_xy": [r.props.scale_xy.0, r.props.scale_xy.1],
            "rotate": r.props.rotate,
            "fill_color": color(r.props.fill),
            "stroke_color": color(r.props.stroke),
            "text_color": color(r.props.text_color),
        },
    })
}

fn print_json(v: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(v).expect("serializable"));
}
