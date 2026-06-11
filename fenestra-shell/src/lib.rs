//! OS glue for fenestra: the winit + wgpu windowed runner and the headless
//! renderer. Everything that touches a display server lives here;
//! `fenestra-core` and `fenestra-kit` stay windowless.

use std::fmt;

mod access;
mod element_render;
mod headless;
mod os_clipboard;
mod synthetic;
pub mod testing;
mod window;

pub use element_render::{render_element, render_element_with_state, with_fonts, with_headless};
pub use headless::Headless;
pub use os_clipboard::OsClipboard;
pub use synthetic::{SyntheticEvent, render_app};
pub use window::{WindowOptions, run_app, run_scene, run_static};

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
