//! Design-system widgets for fenestra, built only on `fenestra-core`'s
//! public API: every color routes through theme tokens, every interactive
//! widget has hover/active/focus/disabled states, and color changes animate
//! with Fast transitions. The specimen and gallery trees double as the
//! golden-test corpus.
#![warn(missing_docs)]

mod demo;
mod gallery;
pub mod icons;
mod specimen;
mod type_specimen;
mod widgets;

pub use demo::{holy_grail, poster, scroll_demo};
pub use gallery::{gallery_controls, gallery_display};
pub use specimen::specimen;
pub use type_specimen::type_specimen;
pub use widgets::*;
