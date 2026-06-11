//! OS glue for fenestra: the winit + wgpu windowed runner and the headless
//! renderer. Everything that touches a display server lives here;
//! `fenestra-core` and `fenestra-kit` stay windowless.

use std::fmt;

// Headless rendering, GPU readback, the OS clipboard, and the AccessKit
// adapter are native-only; on the web only the windowed (canvas) runner
// exists. `fenestra-core` and `fenestra-kit` compile everywhere.
#[cfg(not(target_arch = "wasm32"))]
mod access;
#[cfg(not(target_arch = "wasm32"))]
mod element_render;
#[cfg(not(target_arch = "wasm32"))]
mod headless;
#[cfg(not(target_arch = "wasm32"))]
mod os_clipboard;
#[cfg(not(target_arch = "wasm32"))]
mod synthetic;
#[cfg(not(target_arch = "wasm32"))]
pub mod testing;
mod window;

#[cfg(not(target_arch = "wasm32"))]
pub use element_render::{
    render_element, render_element_with, render_element_with_state, with_fonts, with_headless,
};
#[cfg(not(target_arch = "wasm32"))]
pub use headless::Headless;
#[cfg(not(target_arch = "wasm32"))]
pub use os_clipboard::OsClipboard;
#[cfg(not(target_arch = "wasm32"))]
pub use synthetic::{SyntheticEvent, render_app};
pub use window::{WindowOptions, run_app};
#[cfg(not(target_arch = "wasm32"))]
pub use window::{run_scene, run_static};

/// Errors from the windowed or headless runners.
#[derive(Debug)]
pub enum ShellError {
    /// No compute-capable wgpu adapter was found.
    NoDevice,
    /// The vello renderer failed to build or render.
    Vello(vello::Error),
    /// The winit event loop failed.
    EventLoop(winit::error::EventLoopError),
    /// GPU readback of the rendered image failed.
    Readback,
}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDevice => write!(f, "no compute-capable wgpu adapter found"),
            Self::Vello(e) => write!(f, "vello renderer error: {e}"),
            Self::EventLoop(e) => write!(f, "winit event loop error: {e}"),
            Self::Readback => write!(f, "GPU readback failed"),
        }
    }
}

impl std::error::Error for ShellError {}
