//! Core of the fenestra GUI framework: element IR, theme tokens, style
//! resolution, taffy layout, parley text, and vello scene building.
//!
//! This crate is windowless by design: everything here is a pure function of
//! `(element tree, theme, size, scale)` plus a single retained `FrameState`.

mod anim;
mod apca;
mod app;
pub mod canvas;
mod children;
mod chrome;
mod clipboard;
pub mod effects;
mod element;
mod events;
mod frame;
mod frame_state;
mod id;
mod input;
mod layout;
pub mod optical;
mod painter;
mod proxy;
mod query;
mod style;
mod surface;
mod text;
mod theme;
mod tokens;

pub use apca::{lc, lc_abs, meets, required_lc};
pub use app::{App, MAIN_WINDOW, WindowDesc};
pub use children::{FromIter, FromTuple, IntoChildren};
pub use chrome::{ChromeElevation, ChromeText};
pub use clipboard::{Clipboard, MemoryClipboard};
pub use element::{
    Cursor, Element, ImageData, InputData, Kind, Overlay, OverlayMode, OverlayPlacement, PathData,
    Semantics, Span, VirtualData, col, div, divider, image_rgba8, path, raw_input, raw_text_area,
    rich_text, row, spacer, span, stack, text,
};
pub use events::{Dispatch, InputEvent, Key, KeyInput, click_msg_of, dispatch, refresh_hover};
pub use frame::{AccessNode, Frame, build_frame, build_scene};
pub use frame_state::FrameState;
pub use id::WidgetId;
pub use proxy::Proxy;
pub use query::{Query, QueryError, TextMatch, by};
pub use style::*;
pub use surface::{Material, Surface, SurfaceBorder, SurfaceBundle, SurfaceFill, SurfaceRadius};
pub use text::Fonts;
pub use theme::{
    BaseField, Contrast, ContrastViolation, DeriveSpec, DuotoneSpec, Mode, Ramp, StatusColors,
    Theme, ThemeSpec, oklch, oklch_of,
};
pub use tokens::*;

// Re-exported so dependents (kit, apps) never need a direct peniko dep.
pub use peniko::Color;
