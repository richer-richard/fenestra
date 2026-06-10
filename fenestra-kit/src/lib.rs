//! Design-system widgets for fenestra, built only on `fenestra-core`'s
//! public API. Widgets land in M4-M6; the painting specimen lives here so
//! the example and the golden tests render the exact same tree.

mod demo;
pub mod icons;
mod specimen;
mod type_specimen;
mod widgets;

pub use demo::{holy_grail, scroll_demo};
pub use specimen::specimen;
pub use type_specimen::type_specimen;
pub use widgets::*;
