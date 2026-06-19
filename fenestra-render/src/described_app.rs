//! `DescribedApp`: a `Description` presented as a fenestra `App`, so the headless
//! `Harness` can drive it. It owns the declarative state map: an unbound handler's
//! `Action::Intent` is recorded (observed via `Harness::take_messages`), while a
//! bound widget's `Action::Set*` updates the state, and the next `view` re-renders
//! the bound widgets from it — so typing and toggling reflect.

use fenestra_core::{App, Element, Theme};
use fenestra_describe::format::Description;
use fenestra_describe::parse::to_element_lenient_with;
use fenestra_describe::state::{Action, StateMap};
use serde_json::{Value, json};

/// A parsed description driven as an app, with its runtime state.
pub struct DescribedApp {
    desc: Description,
    theme: Theme,
    state: StateMap,
}

impl DescribedApp {
    /// Wraps a description and the theme its colors resolve against, seeding the
    /// runtime state from the description's initial `state`.
    pub fn new(desc: Description, theme: Theme) -> Self {
        let state = desc.state.clone();
        Self { desc, theme, state }
    }

    /// The current runtime state (after any interactions).
    #[must_use]
    pub fn state(&self) -> &StateMap {
        &self.state
    }
}

impl App for DescribedApp {
    type Msg = Action;

    fn update(&mut self, msg: Action) {
        match msg {
            // Inert author intents are observed via take_messages, not applied.
            Action::Intent(_) => {}
            Action::SetBool(key, value) => {
                self.state.insert(key, Value::Bool(value));
            }
            Action::SetText(key, value) => {
                self.state.insert(key, Value::String(value));
            }
            Action::SetNumber(key, value) => {
                self.state.insert(key, json!(value));
            }
        }
    }

    fn view(&self) -> Element<Action> {
        // Best-effort: the engine validates first and surfaces parse errors, so
        // here we always produce a renderable tree (clamp over panic).
        to_element_lenient_with(&self.desc, &self.theme, &self.state).0
    }

    fn theme(&self) -> Theme {
        self.theme.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fenestra_core::{Fonts, FrameState, build_frame};

    #[test]
    fn view_produces_the_described_tree() {
        let desc =
            serde_json::from_str(r#"{"schema":"fenestra/1","root":{"button":{"label":"Go"}}}"#)
                .unwrap();
        let app = DescribedApp::new(desc, Theme::light());
        let view = app.view();
        let mut fonts = Fonts::embedded();
        let mut state = FrameState::new();
        let frame = build_frame(
            &view,
            &Theme::light(),
            &mut fonts,
            &mut state,
            (200.0, 80.0),
            1.0,
        );
        let aria = frame.access_yaml();
        assert!(aria.contains("button"), "{aria}");
        assert!(aria.contains("Go"), "{aria}");
    }
}
