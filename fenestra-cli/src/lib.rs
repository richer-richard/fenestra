//! The fenestra engine behind the `fenestra` CLI and the `fenestra-mcp`
//! server: render a serialized `Description` to pixels, drive it through
//! scripted interactions, and compare against baselines. One engine, two
//! front doors.

pub mod described_app;
pub mod theme_input;

pub use described_app::DescribedApp;
pub use theme_input::resolve_theme;
