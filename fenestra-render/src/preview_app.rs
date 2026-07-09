//! `PreviewApp`: a live-reload wrapper around a `fenestra/1` description file.
//! `fenestra preview <file>` opens this in a window; a background thread polls
//! the file's content signature every ~200ms and nudges a reload through the
//! [`Proxy`] the runner hands to [`App::init`]. A parse failure never crashes
//! or blanks the window: the last description that parsed cleanly keeps
//! rendering, with a themed error callout (path-pointed, from
//! [`DescribeError`]) banked above it; the very first load, if broken, shows
//! the callout alone over an empty root. Runtime state survives a reload
//! best-effort: a binding the new description still declares keeps its
//! current value, a new one seeds from the new description's initial state,
//! and a removed one is dropped (see [`merge_state`]).
//!
//! The reload/merge logic below (`reload_from_str`, [`merge_state`],
//! `file_signature`) is plain data in, data out, and is exercised headlessly in
//! this module's tests. The windowed run itself
//! (`fenestra_shell::run_app` driving this as an `App`) is NOT covered by
//! CI — it opens a real OS window and needs a display the test runner
//! doesn't have.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use fenestra_core::{App, Element, Proxy, SP3, Theme, col};
use fenestra_describe::error::DescribeError;
use fenestra_describe::format::Description;
use fenestra_describe::parse::to_element_lenient_with;
use fenestra_describe::state::{Action, StateMap};
use fenestra_kit::{Status, callout};
use serde_json::{Value, json};

/// How often the background thread stats the previewed file.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// `PreviewApp`'s message: a description-driven action (a bound widget's
/// change, or an inert intent), or the poll thread's "the file changed" nudge.
#[derive(Debug, Clone)]
pub enum PreviewMsg {
    /// Forwarded from a widget handler in the current view.
    Action(Action),
    /// The poll thread saw the file's content change; re-read and re-parse it.
    Reload,
}

/// A `fenestra/1` description file, live-reloaded from disk.
pub struct PreviewApp {
    path: PathBuf,
    theme: Theme,
    /// The last description that parsed *and* validated cleanly.
    desc: Description,
    /// Runtime state for `desc`'s bindings, carried across reloads.
    state: StateMap,
    /// Problems from the most recent reload attempt; empty means `desc` is
    /// exactly what's on disk right now.
    errors: Vec<DescribeError>,
    /// Set on drop to stop the background poll thread — so a host that opens and
    /// closes a preview without exiting the process doesn't leak the thread.
    stop: Arc<AtomicBool>,
}

impl PreviewApp {
    /// Opens `path` for preview: the first load happens synchronously, so
    /// even a broken file on first open renders the error panel right away
    /// (no blank flash while `App::init` gets around to starting the poll
    /// thread).
    #[must_use]
    pub fn new(path: PathBuf, theme: Theme) -> Self {
        let mut app = Self {
            path,
            theme,
            desc: fallback_description(),
            state: StateMap::new(),
            errors: Vec::new(),
            stop: Arc::new(AtomicBool::new(false)),
        };
        app.reload();
        app
    }

    /// A window title: the file name, plus a clean/error indicator reflecting
    /// the state *at the time this is called*. The windowed runner only
    /// retitles reactively for secondary `App::windows()` entries, not the
    /// main window, so this does not update the OS title bar on later
    /// reloads — the in-window error callout is the live indicator.
    #[must_use]
    pub fn window_title(&self) -> String {
        let name = self.path.file_name().map_or_else(
            || self.path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let status = if self.errors.is_empty() {
            "clean"
        } else {
            "error"
        };
        format!("{name} — fenestra preview [{status}]")
    }

    /// The current parse/validate problems; empty when the on-screen view
    /// matches what's on disk right now.
    #[must_use]
    pub fn errors(&self) -> &[DescribeError] {
        &self.errors
    }

    /// Re-reads and re-parses the file from disk (called on `Reload`, and
    /// once synchronously by [`Self::new`]).
    fn reload(&mut self) {
        match std::fs::read_to_string(&self.path) {
            Ok(json) => self.reload_from_str(&json),
            Err(e) => {
                self.errors = vec![DescribeError::new(
                    "file",
                    format!("cannot read {}: {e}", self.path.display()),
                )];
            }
        }
    }

    /// The reload logic, isolated from disk IO so it's testable headlessly:
    /// parse `json`, and on success — syntactically *and* semantically —
    /// swap in the new description and merge runtime state ([`merge_state`]);
    /// on any failure, record the path-pointed problems and keep the last
    /// good description and state exactly as they were.
    fn reload_from_str(&mut self, json: &str) {
        let desc: Description = match serde_json::from_str(json) {
            Ok(desc) => desc,
            Err(e) => {
                self.errors = match fenestra_describe::parse::validate(json) {
                    Err(errs) => errs,
                    // Only reachable if `validate`'s re-parse ever disagrees
                    // with the direct one above; fall back to the original
                    // error rather than claim the file is fine.
                    Ok(()) => vec![DescribeError::new(String::new(), e.to_string())],
                };
                return;
            }
        };
        let merged = merge_state(&self.state, &desc.state);
        let (_, errors) = to_element_lenient_with(&desc, &self.theme, &merged);
        if errors.is_empty() {
            self.state = merged;
            self.desc = desc;
            self.errors = Vec::new();
        } else {
            self.errors = errors;
        }
    }
}

impl Drop for PreviewApp {
    fn drop(&mut self) {
        // Signal the poll thread to stop; it observes this within one poll
        // interval and returns, so dropping a preview reclaims its thread.
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl App for PreviewApp {
    type Msg = PreviewMsg;

    fn init(&mut self, proxy: Proxy<Self::Msg>) {
        let path = self.path.clone();
        let stop = Arc::clone(&self.stop);
        std::thread::spawn(move || poll_for_changes(&path, &proxy, &stop));
    }

    fn update(&mut self, msg: PreviewMsg) {
        match msg {
            // Inert author intents are observed via take_messages in the
            // headless harness; the windowed preview has no such observer,
            // so there's nothing to do with one here either.
            PreviewMsg::Action(Action::Intent(_)) => {}
            PreviewMsg::Action(Action::SetBool(key, value)) => {
                self.state.insert(key, Value::Bool(value));
            }
            PreviewMsg::Action(Action::SetText(key, value)) => {
                self.state.insert(key, Value::String(value));
            }
            PreviewMsg::Action(Action::SetNumber(key, value)) => {
                self.state.insert(key, json!(value));
            }
            PreviewMsg::Reload => self.reload(),
        }
    }

    fn view(&self) -> Element<PreviewMsg> {
        // Best-effort: `desc` only ever holds a description that already
        // parsed and validated cleanly (clamp over panic).
        let content = to_element_lenient_with(&self.desc, &self.theme, &self.state)
            .0
            .map(PreviewMsg::Action);
        if self.errors.is_empty() {
            return content;
        }
        let message = self
            .errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        col()
            .gap(SP3)
            .children([callout(Status::Danger, message), content])
    }

    fn theme(&self) -> Theme {
        self.theme.clone()
    }
}

/// Merges runtime state across a reload: a key the new description still
/// declares keeps its current value (so a toggled checkbox or typed text
/// survives an edit that leaves that binding alone); a key declared for the
/// first time seeds from the new description's initial value; a key the new
/// description no longer declares is dropped. Best-effort by nature — there's
/// no way to distinguish a renamed binding from a removed one.
fn merge_state(current: &StateMap, fresh_initial: &StateMap) -> StateMap {
    fresh_initial
        .iter()
        .map(|(key, fresh_value)| {
            let value = current
                .get(key)
                .cloned()
                .unwrap_or_else(|| fresh_value.clone());
            (key.clone(), value)
        })
        .collect()
}

/// The view shown before any file has ever parsed cleanly: an empty root, so
/// a broken file on first load renders as the error panel alone.
fn fallback_description() -> Description {
    serde_json::from_str(r#"{"schema":"fenestra/1","root":{"col":{"children":[]}}}"#)
        .expect("the fallback description is valid fenestra/1 JSON")
}

/// Hashes `path`'s bytes into a change signature. Content-based rather than
/// `(mtime, len)` so a same-length rewrite that lands within the filesystem's
/// mtime resolution — a common "I saved and nothing happened" case on coarse
/// (1s) timestamps — still registers as a change. `None` when the file can't be
/// read (e.g. mid-save or renamed), which itself differs from a good read and
/// so still drives a reload.
fn file_signature(path: &Path) -> io::Result<u64> {
    use std::hash::{Hash, Hasher};
    let bytes = std::fs::read(path)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    Ok(hasher.finish())
}

/// Polls `path` every [`POLL_INTERVAL`] and sends [`PreviewMsg::Reload`]
/// whenever its content signature changes. Returns once `stop` is set — the
/// owning [`PreviewApp`] was dropped — so an embedding host that opens and
/// closes previews doesn't leak a thread per preview. (The `fenestra preview`
/// CLI never sets it: the process exit takes the thread down, and sending on a
/// dead `Proxy` after that is a documented no-op.)
fn poll_for_changes(path: &Path, proxy: &Proxy<PreviewMsg>, stop: &AtomicBool) {
    let mut last = file_signature(path).ok();
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(POLL_INTERVAL);
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let current = file_signature(path).ok();
        if current != last {
            last = current;
            proxy.send(PreviewMsg::Reload);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use fenestra_core::{Fonts, FrameState, build_frame};

    use super::*;

    /// Builds the access tree for the app's current view — headless (CPU
    /// layout only, no GPU/window needed), the same recipe `described_app`'s
    /// own test uses.
    fn access_tree(app: &PreviewApp) -> String {
        let view = app.view();
        let mut fonts = Fonts::embedded();
        let mut state = FrameState::new();
        let frame = build_frame(
            &view,
            &app.theme,
            &mut fonts,
            &mut state,
            (400.0, 300.0),
            1.0,
        );
        frame.access_yaml()
    }

    #[test]
    fn poll_signature_detects_a_same_length_edit() {
        let path = env::temp_dir().join("fenestra_preview_sig_test.json");
        std::fs::write(&path, "aaa").unwrap();
        let before = file_signature(&path).unwrap();
        // Same length, different bytes, and (in a test) the same coarse mtime
        // tick — a `(mtime, len)` stat could miss this; a content hash cannot.
        std::fs::write(&path, "bbb").unwrap();
        let after = file_signature(&path).unwrap();
        assert_ne!(
            before, after,
            "a same-length content change must be detected"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn drop_signals_the_poll_thread_to_stop() {
        let missing = env::temp_dir().join("fenestra_preview_drop_test.json");
        let app = PreviewApp::new(missing, Theme::light());
        let stop = Arc::clone(&app.stop);
        assert!(
            !stop.load(Ordering::Relaxed),
            "the flag is clear while the app is alive"
        );
        drop(app);
        assert!(
            stop.load(Ordering::Relaxed),
            "dropping the app must set the poll thread's stop flag"
        );
    }

    #[test]
    fn reload_swaps_in_the_new_tree() {
        let missing = env::temp_dir().join("fenestra_preview_never_read.json");
        let mut app = PreviewApp::new(missing, Theme::light());

        app.reload_from_str(r#"{"schema":"fenestra/1","root":{"button":{"label":"First"}}}"#);
        assert!(app.errors().is_empty(), "{:?}", app.errors());
        assert!(access_tree(&app).contains("First"));

        app.reload_from_str(r#"{"schema":"fenestra/1","root":{"button":{"label":"Second"}}}"#);
        assert!(app.errors().is_empty(), "{:?}", app.errors());
        let tree = access_tree(&app);
        assert!(tree.contains("Second"), "{tree}");
        assert!(!tree.contains("First"), "{tree}");
    }

    #[test]
    fn first_load_of_a_broken_file_shows_the_error_panel_alone() {
        let missing = env::temp_dir().join("fenestra_preview_does_not_exist.json");
        let app = PreviewApp::new(missing, Theme::light());
        assert!(
            !app.errors().is_empty(),
            "an unreadable file is a load error"
        );
        let tree = access_tree(&app);
        assert!(
            !tree.contains("button") && !tree.contains("checkbox"),
            "nothing has ever parsed yet, so the view is the callout over an empty root: {tree}"
        );
    }

    #[test]
    fn broken_edit_shows_error_panel_over_last_good_view() {
        let missing = env::temp_dir().join("fenestra_preview_never_read_2.json");
        let mut app = PreviewApp::new(missing, Theme::light());

        app.reload_from_str(r#"{"schema":"fenestra/1","root":{"button":{"label":"Good"}}}"#);
        assert!(app.errors().is_empty(), "{:?}", app.errors());
        assert!(access_tree(&app).contains("Good"));

        // An unknown field: a typo an author would plausibly make mid-edit.
        app.reload_from_str(r#"{"schema":"fenestra/1","root":{"button":{"labell":"Oops"}}}"#);
        assert!(
            !app.errors().is_empty(),
            "an unknown field should be rejected"
        );
        let tree = access_tree(&app);
        assert!(
            tree.contains("Good"),
            "the last good view should still render: {tree}"
        );
    }

    #[test]
    fn state_survives_reload_for_persisting_keys_and_reseeds_new_ones() {
        let mut current = StateMap::new();
        current.insert("name".to_string(), Value::String("Ada".to_string()));
        current.insert("gone".to_string(), Value::Bool(true));
        let mut fresh = StateMap::new();
        fresh.insert("name".to_string(), Value::String(String::new()));
        fresh.insert("agreed".to_string(), Value::Bool(false));

        let merged = merge_state(&current, &fresh);
        assert_eq!(
            merged.get("name"),
            Some(&Value::String("Ada".to_string())),
            "kept the user's value for a binding that persists"
        );
        assert_eq!(
            merged.get("agreed"),
            Some(&Value::Bool(false)),
            "seeded the new key's default"
        );
        assert!(
            !merged.contains_key("gone"),
            "a binding the new description dropped is not carried forward"
        );
    }

    #[test]
    fn reload_from_bad_json_is_path_pointed_and_keeps_last_good_state() {
        let missing = env::temp_dir().join("fenestra_preview_never_read_3.json");
        let mut app = PreviewApp::new(missing, Theme::light());
        app.reload_from_str(
            r#"{"schema":"fenestra/1","root":{"checkbox":{"bind":"agreed","label":"Agree"}}}"#,
        );
        assert!(app.errors().is_empty(), "{:?}", app.errors());

        // Malformed JSON entirely (a mid-save partial write, e.g.).
        app.reload_from_str(r#"{"schema":"fenestra/1", "root": {"#);
        assert!(!app.errors().is_empty());
        assert!(access_tree(&app).contains("checkbox"), "last good persists");
    }
}
