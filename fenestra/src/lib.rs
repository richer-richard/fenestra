//! fenestra: a pure-Rust native GUI framework with web-grade aesthetics.
//!
//! Re-exports the core IR and theme, the widget kit, and the windowed and
//! headless runners.

pub use fenestra_core::*;
pub use fenestra_kit as kit;
pub use fenestra_shell as shell;

pub use fenestra_shell::WindowOptions;

/// Opens a window and runs the app until the window closes.
///
/// # Panics
/// If the event loop or GPU surface cannot be created.
pub fn run<A: App + 'static>(app: A, options: WindowOptions) {
    fenestra_shell::run_app(app, options).expect("fenestra event loop failed");
}

/// Commonly used items: builders, tokens, widgets, and the runner.
pub mod prelude {
    pub use fenestra_core::*;
    pub use fenestra_kit::*;
    pub use fenestra_shell::WindowOptions;
}
