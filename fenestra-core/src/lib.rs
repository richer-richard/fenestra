//! Core of the fenestra GUI framework: element IR, theme tokens, style
//! resolution, taffy layout, parley text, and vello scene building.
//!
//! This crate is windowless by design: everything here is a pure function of
//! `(element tree, theme, size, scale)` plus a single retained `FrameState`.

pub mod paint;
mod theme;

pub use theme::Theme;
