//! Core of the fenestra GUI framework: element IR, theme tokens, style
//! resolution, taffy layout, parley text, and vello scene building.
//!
//! This crate is windowless by design: everything here is a pure function of
//! `(element tree, theme, size, scale)` plus a single retained `FrameState`.

mod anim;
mod app;
mod children;
mod clipboard;
mod element;
mod events;
mod frame;
mod frame_state;
mod id;
mod input;
mod layout;
mod painter;
mod proxy;
mod query;
mod style;
mod text;
mod theme;
mod tokens;

pub use app::{App, MAIN_WINDOW, WindowDesc};
pub use children::{FromIter, FromTuple, IntoChildren};
pub use clipboard::{Clipboard, MemoryClipboard};
pub use element::{
    Cursor, Element, ImageData, InputData, Kind, Overlay, OverlayMode, OverlayPlacement, PathData,
    Semantics, VirtualData, col, div, divider, image_rgba8, path, raw_input, raw_text_area, row,
    spacer, stack, text,
};
pub use events::{Dispatch, InputEvent, Key, KeyInput, click_msg_of, dispatch, refresh_hover};
pub use frame::{AccessNode, Frame, build_frame, build_scene};
pub use frame_state::FrameState;
pub use id::WidgetId;
pub use proxy::Proxy;
pub use query::{Query, QueryError, TextMatch, by};
pub use style::*;
pub use text::Fonts;
pub use theme::{Mode, Ramp, StatusColors, Theme};
pub use tokens::*;

// Re-exported so dependents (kit, apps) never need a direct peniko dep.
pub use peniko::Color;
