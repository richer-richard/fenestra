//! The dogfood flagship: a live dashboard over a Claude Code session log.
//!
//! Point it at a session JSONL (or let it find the newest one for this
//! repo) and it shows the agent's tool-call feed (virtualized), token
//! sparkline, per-tool bar chart, and summary stats — refreshed live via
//! a subscription while the session runs. One app exercises the effect
//! layer (`Cmd::task` parse off-thread, `Sub::every` tail), the charts,
//! virtualization, the native menu bar, and headless rendering.
//!
//! `cargo run --example agent_dashboard [path/to/session.jsonl]`
//! `cargo run --example agent_dashboard -- shot` renders both themes
//! headlessly from a bundled fixture to `gallery/agent_dashboard_*.png`.

use std::path::PathBuf;
use std::time::Duration;

use fenestra::prelude::*;
use fenestra_charts::{BarChartAxes, sparkline};

/// One tool invocation from the log.
#[derive(Clone)]
struct Call {
    time: String,
    tool: String,
    summary: String,
}

/// Everything the dashboard shows, parsed off-thread.
#[derive(Clone, Default)]
struct Stats {
    calls: Vec<Call>,
    /// Output tokens per assistant message, oldest first.
    tokens: Vec<f32>,
    total_output: u64,
    by_tool: Vec<(String, f32)>,
}

fn parse_log(text: &str) -> Stats {
    let mut stats = Stats::default();
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("assistant") {
            continue;
        }
        let time = v
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(|t| t.split('T').nth(1))
            .map(|t| t.chars().take(8).collect::<String>())
            .unwrap_or_default();
        let message = v.get("message");
        if let Some(usage) = message.and_then(|m| m.get("usage")) {
            let out = usage
                .get("output_tokens")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            stats.total_output += out;
            #[expect(clippy::cast_precision_loss, reason = "chart heights only")]
            stats.tokens.push(out as f32);
        }
        let Some(content) = message
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
        else {
            continue;
        };
        for block in content {
            if block.get("type").and_then(|t| t.as_str()) != Some("tool_use") {
                continue;
            }
            let tool = block
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("?")
                .to_owned();
            // A short human-readable input summary: the most telling field.
            let input = block.get("input");
            let summary = ["description", "command", "file_path", "subject", "prompt"]
                .iter()
                .find_map(|k| input.and_then(|i| i.get(k)).and_then(|v| v.as_str()))
                .unwrap_or("")
                .chars()
                .take(80)
                .collect();
            *counts.entry(tool.clone()).or_default() += 1;
            stats.calls.push(Call {
                time: time.clone(),
                tool,
                summary,
            });
        }
    }
    #[expect(clippy::cast_precision_loss, reason = "chart heights only")]
    {
        stats.by_tool = counts.into_iter().map(|(k, v)| (k, v as f32)).collect();
    }
    stats
        .by_tool
        .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    stats.by_tool.truncate(8);
    stats
}

/// The newest session log for this repo's Claude Code project, if any.
fn newest_session() -> Option<PathBuf> {
    let dir = dirs_home()?.join(".claude/projects/-Users-richardhuang-GitHub-fenestra");
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()? {
        let path = entry.ok()?.path();
        if path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        let modified = path.metadata().ok()?.modified().ok()?;
        if newest.as_ref().is_none_or(|(t, _)| modified > *t) {
            newest = Some((modified, path));
        }
    }
    newest.map(|(_, p)| p)
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

struct AgentDashboard {
    path: Option<PathBuf>,
    stats: Stats,
    live: bool,
    dark: bool,
    status: String,
}

#[derive(Clone)]
enum Msg {
    Reload,
    Loaded(Stats, String),
    ToggleLive,
    ToggleTheme,
}

impl AgentDashboard {
    fn load_cmd(&self) -> Cmd<Msg> {
        let Some(path) = self.path.clone() else {
            // Nothing to load: keep whatever is shown (the shot fixture);
            // only report the miss when there is nothing at all.
            if self.stats.calls.is_empty() {
                return Cmd::msg(Msg::Loaded(Stats::default(), "no session log found".into()));
            }
            return Cmd::none();
        };
        Cmd::task(move || match std::fs::read_to_string(&path) {
            Ok(text) => {
                let stats = parse_log(&text);
                let status = format!(
                    "{} · {} calls",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    stats.calls.len()
                );
                Msg::Loaded(stats, status)
            }
            Err(e) => Msg::Loaded(Stats::default(), format!("read failed: {e}")),
        })
    }
}

impl App for AgentDashboard {
    type Msg = Msg;

    fn update(&mut self, _: Msg) {}

    fn update_with(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Reload => self.load_cmd(),
            Msg::Loaded(stats, status) => {
                self.stats = stats;
                self.status = status;
                Cmd::none()
            }
            Msg::ToggleLive => {
                self.live = !self.live;
                Cmd::none()
            }
            Msg::ToggleTheme => {
                self.dark = !self.dark;
                Cmd::none()
            }
        }
    }

    fn init_cmd(&mut self) -> Cmd<Msg> {
        self.load_cmd()
    }

    fn subscriptions(&self) -> Vec<Sub<Msg>> {
        if self.live {
            vec![Sub::every("tail", Duration::from_secs(2), || Msg::Reload)]
        } else {
            Vec::new()
        }
    }

    fn menu(&self) -> Option<MenuSpec<Msg>> {
        Some(MenuSpec::new([
            MenuDesc::new(
                "Session",
                [MenuItemDesc::item("Reload", Msg::Reload).accelerator("CmdOrCtrl+R")],
            ),
            MenuDesc::new(
                "View",
                [
                    MenuItemDesc::item(
                        if self.live {
                            "Pause live tail"
                        } else {
                            "Resume live tail"
                        },
                        Msg::ToggleLive,
                    ),
                    MenuItemDesc::item("Toggle theme", Msg::ToggleTheme),
                ],
            ),
        ]))
    }

    fn theme(&self) -> Theme {
        if self.dark {
            Theme::dark()
        } else {
            Theme::light()
        }
    }

    fn view(&self) -> Element<Msg> {
        let stats = &self.stats;
        let spark_values: Vec<f32> = stats.tokens.iter().rev().take(120).rev().copied().collect();
        let top_tool = stats
            .by_tool
            .first()
            .map_or_else(|| "—".to_owned(), |(name, n)| format!("{name} ({n})"));

        let header = row().items_center().gap(SP3).children((
            text("Agent session")
                .size(TextSize::Lg)
                .weight(Weight::Semibold),
            text(&self.status)
                .size(TextSize::Sm)
                .themed(|t: &Theme, s| s.color(t.text_muted))
                .grow(),
            badge(
                if self.live { "live" } else { "paused" },
                if self.live {
                    Status::Success
                } else {
                    Status::Warning
                },
            ),
            button(if self.live { "Pause" } else { "Resume" })
                .variant(ButtonVariant::Secondary)
                .on_click(Msg::ToggleLive),
            button("Reload")
                .variant(ButtonVariant::Secondary)
                .on_click(Msg::Reload),
        ));

        let cards = row().gap(SP3).children((
            stat_card::<Msg>("Tool calls", stats.calls.len().to_string()),
            stat_card::<Msg>("Output tokens", stats.total_output.to_string()),
            stat_card::<Msg>("Top tool", top_tool),
        ));

        let charts = row().gap(SP3).items_end().children((
            col().gap(SP2).children((
                text("output tokens per turn")
                    .size(TextSize::Xs)
                    .themed(|t: &Theme, s| s.color(t.text_muted)),
                sparkline::<Msg>(spark_values).w(320.0).h(48.0),
            )),
            BarChartAxes::new(stats.by_tool.iter().map(|(k, v)| (k.clone(), *v)))
                .w(360.0)
                .h(150.0)
                .build::<Msg>()
                .grow(),
        ));

        let calls = stats.calls.clone();
        let count = calls.len();
        let feed = virtual_list(count, 28.0, move |i| {
            // Newest first.
            let call = &calls[count - 1 - i];
            row().gap(SP3).items_center().px(SP2).children((
                text(&call.time)
                    .size(TextSize::Xs)
                    .mono()
                    .themed(|t: &Theme, s| s.color(t.text_muted)),
                badge(&call.tool, Status::Accent),
                text(&call.summary).size(TextSize::Sm).grow(),
            ))
        })
        .id("feed");

        col().p(SP5).gap(SP4).w_full().h_full().children((
            header,
            cards,
            charts,
            text(format!("{} calls, newest first", count))
                .size(TextSize::Xs)
                .themed(|t: &Theme, s| s.color(t.text_muted)),
            feed.grow(),
        ))
    }
}

/// A tiny deterministic fixture for headless shots (no real log needed).
const FIXTURE: &str = include_str!("assets/agent_dashboard_fixture.jsonl");

fn main() {
    let arg = std::env::args().nth(1);
    if arg.as_deref() == Some("shot") {
        let out = std::path::Path::new("gallery");
        std::fs::create_dir_all(out).expect("create gallery dir");
        for dark in [false, true] {
            let mut app = AgentDashboard {
                path: None,
                stats: parse_log(FIXTURE),
                live: false,
                dark,
                status: "session.jsonl · fixture".into(),
            };
            let theme = app.theme();
            let img = fenestra::shell::render_app(&mut app, &[], (1080, 720), &theme);
            let name = if dark {
                "agent_dashboard_dark.png"
            } else {
                "agent_dashboard_light.png"
            };
            img.save(out.join(name)).expect("write shot");
            println!("wrote gallery/{name}");
        }
        return;
    }
    let path = arg.map(PathBuf::from).or_else(newest_session);
    fenestra::run(
        AgentDashboard {
            path,
            stats: Stats::default(),
            live: true,
            dark: false,
            status: "loading…".into(),
        },
        WindowOptions::titled("Agent session dashboard").with_size(1080.0, 720.0),
    );
}
