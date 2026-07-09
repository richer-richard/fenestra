//! The fenestra engine behind the `fenestra` CLI and the `fenestra-mcp`
//! server: render a serialized `Description` to pixels, drive it through
//! scripted interactions, and compare against baselines. One engine, two
//! front doors.

pub mod described_app;
pub mod engine;
pub mod preview_app;
pub mod scenario;
pub mod theme_input;

pub use described_app::DescribedApp;
pub use engine::{
    EngineError, InteractOut, RenderOut, ScreenshotDiff, Step, diff_images, interact,
    match_screenshot, render, validate_masks,
};
pub use preview_app::{PreviewApp, PreviewMsg};
pub use scenario::{
    AriaExpect, CheckOutcome, Expect, QueryExpect, Scenario, ScreenshotExpect, VerifyOut,
    VerifyReport, bless, verify,
};
pub use theme_input::resolve_theme;
