//! fenestra: a pure-Rust native GUI framework with web-grade aesthetics.
//!
//! Re-exports the core IR and theme, the widget kit, and the windowed and
//! headless runners.

pub use fenestra_core::*;
pub use fenestra_kit as kit;
pub use fenestra_shell as shell;

/// Commonly used items: builders, tokens, widgets, and the runner.
pub mod prelude {
    pub use fenestra_core::*;
    // fenestra_kit is re-exported here once it gains its first widgets (M4);
    // a glob of the empty crate is an unused import today.
}
