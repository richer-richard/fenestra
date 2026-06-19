//! `DescribedApp`: a `Description` presented as a fenestra `App`, so the headless
//! `Harness` can drive it. Static + intent — `update` is inert (the framework's
//! retained state handles focus/hover/scroll/caret/overlays), and handlers emit
//! their intent strings, captured via `Harness::take_messages`.

use fenestra_core::{App, Element, Theme};
use fenestra_describe::format::Description;
use fenestra_describe::parse::to_element_lenient;

/// A parsed description driven as an app. The view re-parses the description
/// each frame (the builders' model: pure, rebuilt every redraw); `update` does
/// nothing, because v1 carries no app state across the boundary.
pub struct DescribedApp {
    desc: Description,
    theme: Theme,
}

impl DescribedApp {
    /// Wraps a description and the theme its colors resolve against.
    pub fn new(desc: Description, theme: Theme) -> Self {
        Self { desc, theme }
    }
}

impl App for DescribedApp {
    type Msg = String;

    fn update(&mut self, _msg: String) {
        // Static + intent: no app state. Intents are observed via take_messages.
    }

    fn view(&self) -> Element<String> {
        // Best-effort: the engine validates first and surfaces parse errors, so
        // here we always produce a renderable tree (clamp over panic).
        to_element_lenient(&self.desc, &self.theme).0
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
