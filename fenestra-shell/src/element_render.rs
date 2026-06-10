//! Headless element rendering: the API agents use to see what they build.

use std::sync::{Mutex, OnceLock};

use fenestra_core::{Element, Fonts, FrameState, Theme, build_frame};
use image::RgbaImage;

use crate::{Headless, ShellError};

static SHARED: OnceLock<Mutex<Headless>> = OnceLock::new();
static FONTS: OnceLock<Mutex<Fonts>> = OnceLock::new();

/// Runs `f` with a process-wide shared [`Headless`] renderer. Creating a
/// renderer compiles vello's shaders, so tests share one.
pub fn with_headless<R>(f: impl FnOnce(&mut Headless) -> R) -> Result<R, ShellError> {
    // Initialization can fail (no adapter); retry on each call until it
    // succeeds rather than caching the failure.
    if SHARED.get().is_none() {
        let headless = Headless::new()?;
        let _ = SHARED.set(Mutex::new(headless));
    }
    let mutex = SHARED.get().ok_or(ShellError::NoDevice)?;
    let mut guard = mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Ok(f(&mut guard))
}

/// Renders an element tree headlessly at scale factor 1.0 over the theme
/// background, using only the embedded fonts for determinism. This is
/// fenestra's product thesis: agents render what they build and look at it.
///
/// # Panics
/// If no compute-capable GPU adapter exists or rendering fails.
pub fn render_element<Msg>(el: Element<Msg>, theme: &Theme, size: (u32, u32)) -> RgbaImage {
    let mut state = FrameState::new();
    state.reduced_motion = true;
    render_element_with_state(el, theme, size, &mut state)
}

/// Like [`render_element`], but with caller-provided retained state, so
/// tests can render scrolled (and later focused/hovered) configurations.
///
/// # Panics
/// If no compute-capable GPU adapter exists or rendering fails.
pub fn render_element_with_state<Msg>(
    el: Element<Msg>,
    theme: &Theme,
    size: (u32, u32),
    state: &mut FrameState,
) -> RgbaImage {
    let fonts = FONTS.get_or_init(|| Mutex::new(Fonts::embedded()));
    let scene = {
        let mut fonts = fonts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        #[expect(clippy::cast_precision_loss, reason = "window sizes fit in f32")]
        let frame = build_frame(
            &el,
            theme,
            &mut fonts,
            state,
            (size.0 as f32, size.1 as f32),
            1.0,
        );
        frame.paint(&mut fonts)
    };
    with_headless(|headless| headless.render(&scene, size.0, size.1, theme.bg))
        .expect("headless renderer unavailable")
        .expect("headless render failed")
}
