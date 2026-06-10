//! Core of the fenestra GUI framework: element IR, theme tokens, style
//! resolution, taffy layout, parley text, and vello scene building.
//!
//! This crate is windowless by design: everything here is a pure function of
//! `(element tree, theme, size, scale)` plus a single retained `FrameState`.

mod element;
mod frame;
mod frame_state;
mod id;
mod layout;
mod painter;
mod style;
mod text;
mod theme;
mod tokens;

pub use element::{Cursor, Element, Kind, col, div, divider, row, spacer, stack, text};
pub use frame::{Frame, build_frame, build_scene};
pub use frame_state::FrameState;
pub use id::WidgetId;
pub use style::*;
pub use text::Fonts;
pub use theme::{Mode, Ramp, StatusColors, Theme};
pub use tokens::*;

// Re-exported so dependents (kit, apps) never need a direct peniko dep.
pub use peniko::Color;
