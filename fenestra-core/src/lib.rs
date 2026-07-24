//! Core of the fenestra GUI framework: element IR, theme tokens, style
//! resolution, taffy layout, parley text, and vello scene building.
//!
//! This crate is windowless by design: everything here is a pure function of
//! `(element tree, theme, size, scale)` plus a single retained `FrameState`.

mod anim;
mod apca;
mod app;
mod breakpoints;
pub mod canvas;
mod children;
mod chrome;
mod clipboard;
mod cmd;
pub mod effects;
mod element;
mod events;
mod frame;
mod frame_state;
mod ghost;
mod grid;
mod i18n;
mod id;
mod input;
mod layout;
mod menu;
mod nav;
pub mod optical;
mod paint_plan;
mod painter;
mod proxy;
mod query;
mod style;
mod surface;
mod text;
mod theme;
mod tokens;

pub use apca::{lc, lc_abs, meets, required_lc, wcag2_passes, wcag2_ratio};
pub use app::{App, MAIN_WINDOW, WindowDesc};
pub use breakpoints::{Breakpoint, Breakpoints};
pub use children::{FromIter, FromTuple, IntoChildren};
pub use chrome::{ChromeElevation, ChromeText};
pub use clipboard::{Clipboard, MemoryClipboard};
pub use cmd::{Cmd, CmdFuture, CmdUnit, Sub, apply_cmd};
pub use element::{
    Cursor, DrawerSide, Element, ExitAnim, ImageData, InputData, Kind, MAX_TREE_DEPTH,
    OpticalCorrection, Overlay, OverlayMode, OverlayPlacement, PathData, Semantics, Span, SwipeDir,
    VirtualData, col, div, divider, image_from_data, image_payload, image_rgba8, path, raw_input,
    raw_text_area, responsive, responsive_hinted, rich_text, row, spacer, span, stack, text,
};
pub use events::{Dispatch, InputEvent, Key, KeyInput, click_msg_of, dispatch, refresh_hover};
pub use frame::{AccessNode, Frame, TextLegibility, build_frame, build_scene, frame_epoch};
pub use frame_state::FrameState;
pub use i18n::{Catalog, Locale};
pub use id::WidgetId;
pub use menu::{MenuDesc, MenuItemDesc, MenuSpec};
pub use nav::Nav;
pub use paint_plan::{MultiPassSpec, PassKind};
pub use proxy::Proxy;
pub use query::{Query, QueryError, TextMatch, by};
pub use style::*;
pub use surface::{Material, Surface, SurfaceBorder, SurfaceBundle, SurfaceFill, SurfaceRadius};
pub use text::Fonts;
pub use theme::{
    BaseField, Contrast, ContrastViolation, DEFAULT_CORNER_SMOOTHING, DeriveSpec, DuotoneSpec,
    Mode, Ramp, StatusColors, Theme, ThemeSpec, WritingDir, oklch, oklch_of,
};
pub use tokens::*;

// Re-exported so dependents (kit, apps) never need a direct peniko dep.
pub use peniko::Color;
