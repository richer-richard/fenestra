//! Headless element rendering: the API agents use to see what they build.

use std::sync::{Mutex, OnceLock};

use fenestra_core::{Color, Element, Fonts, FrameState, Theme, build_frame};
use image::RgbaImage;

use crate::{Headless, ShellError};

static SHARED: OnceLock<Mutex<Headless>> = OnceLock::new();
static FONTS: OnceLock<Mutex<Fonts>> = OnceLock::new();

/// Runs `f` with the process-wide embedded-only font system used by all
/// headless rendering.
pub fn with_fonts<R>(f: impl FnOnce(&mut Fonts) -> R) -> R {
    let fonts = FONTS.get_or_init(|| Mutex::new(Fonts::embedded()));
    let mut guard = fonts
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f(&mut guard)
}

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
/// The requested size is clamped to the device-supported range (at least
/// 1x1, at most the maximum texture dimension, typically 8192); check the
/// returned image's dimensions when the input may be out of range.
///
/// # Panics
/// If no compute-capable GPU adapter exists or rendering fails — use
/// [`try_render_element`] to handle those as values (embedding hosts, MCP
/// servers, CI without a software rasterizer installed).
pub fn render_element<Msg>(el: Element<Msg>, theme: &Theme, size: (u32, u32)) -> RgbaImage {
    try_render_element(el, theme, size).unwrap_or_else(|e| panic!("headless render failed: {e}"))
}

/// Fallible twin of [`render_element`]: a missing GPU adapter or a render
/// failure comes back as a [`ShellError`] with an actionable message
/// instead of a panic.
///
/// # Errors
/// [`ShellError::NoDevice`] when no compute-capable wgpu adapter exists;
/// other [`ShellError`]s when the render itself fails.
pub fn try_render_element<Msg>(
    el: Element<Msg>,
    theme: &Theme,
    size: (u32, u32),
) -> Result<RgbaImage, ShellError> {
    let mut state = FrameState::new();
    state.reduced_motion = true;
    try_render_element_with_state(el, theme, size, &mut state)
}

/// Like [`render_element`], but with caller-provided [`Fonts`], so design
/// languages can register Display/Serif faces (`Fonts::register`) and
/// render through them. The requested size is clamped like
/// [`render_element`]'s.
///
/// # Panics
/// If no compute-capable GPU adapter exists or rendering fails — use
/// [`try_render_element_with`] to handle those as values.
pub fn render_element_with<Msg>(
    el: Element<Msg>,
    theme: &Theme,
    size: (u32, u32),
    fonts: &mut Fonts,
) -> RgbaImage {
    try_render_element_with(el, theme, size, fonts)
        .unwrap_or_else(|e| panic!("headless render failed: {e}"))
}

/// Fallible twin of [`render_element_with`].
///
/// # Errors
/// [`ShellError::NoDevice`] when no compute-capable wgpu adapter exists;
/// other [`ShellError`]s when the render itself fails.
pub fn try_render_element_with<Msg>(
    el: Element<Msg>,
    theme: &Theme,
    size: (u32, u32),
    fonts: &mut Fonts,
) -> Result<RgbaImage, ShellError> {
    let size = with_headless(|h| h.clamp_size(size.0, size.1))?;
    let mut state = FrameState::new();
    state.reduced_motion = true;
    #[expect(clippy::cast_precision_loss, reason = "window sizes fit in f32")]
    let frame = build_frame(
        &el,
        theme,
        fonts,
        &mut state,
        (size.0 as f32, size.1 as f32),
        1.0,
    );
    // Route through the two-pass planner so frosted glass shows real backdrop
    // blur; frames with no glass / filter fast-path to a single pass.
    with_headless(|headless| {
        headless.render_plan(&frame, fonts, &mut state, size.0, size.1, theme.bg)
    })?
}

/// Renders an element tree over a caller-supplied base color at a scale
/// factor — the entry point for offline frame samplers (`fenestra-motion`)
/// and any render that needs real alpha: `Color::TRANSPARENT` keeps empty
/// canvas at alpha 0 (note vello output is premultiplied; un-premultiply
/// before writing straight-alpha formats).
///
/// `size` is logical px; the texture is `size × scale` (clamped to the
/// device limit). At `scale == 1.0` this is the same two-pass pipeline as
/// [`render_element`] (frosted glass gets its backdrop pass); other scales
/// render single-pass — glass falls back to its translucent tint, exactly
/// like the live window — which is fine for the cheap-preview use these
/// scaled renders exist for.
///
/// Caller-provided [`Fonts`] and [`FrameState`] keep this path lock-free up
/// to the shared GPU: parallel callers build layouts concurrently and
/// serialize only on the device mutex.
///
/// # Errors
/// [`ShellError`] when no compute-capable GPU adapter exists or the render
/// fails.
pub fn render_element_over<Msg>(
    el: Element<Msg>,
    theme: &Theme,
    size: (u32, u32),
    scale: f64,
    bg: Color,
    fonts: &mut Fonts,
    state: &mut FrameState,
) -> Result<RgbaImage, ShellError> {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "physical sizes are clamped to the device range right after"
    )]
    let physical = (
        (f64::from(size.0) * scale).round() as u32,
        (f64::from(size.1) * scale).round() as u32,
    );
    let (pw, ph) = with_headless(|h| h.clamp_size(physical.0, physical.1))?;
    #[expect(clippy::cast_precision_loss, reason = "window sizes fit in f32")]
    let logical = (size.0 as f32, size.1 as f32);
    let frame = build_frame(&el, theme, fonts, state, logical, scale);
    if (scale - 1.0).abs() < f64::EPSILON {
        return with_headless(|headless| headless.render_plan(&frame, fonts, state, pw, ph, bg))?;
    }
    let logical_scene = frame.paint(fonts, state);
    let mut scene = vello::Scene::new();
    scene.append(&logical_scene, Some(vello::kurbo::Affine::scale(scale)));
    with_headless(|headless| headless.render(&scene, pw, ph, bg))?
}

/// Like [`render_element`], but with caller-provided retained state, so
/// tests can render scrolled (and later focused/hovered) configurations.
/// The requested size is clamped like [`render_element`]'s.
///
/// # Panics
/// If no compute-capable GPU adapter exists or rendering fails — use
/// [`try_render_element_with_state`] to handle those as values.
pub fn render_element_with_state<Msg>(
    el: Element<Msg>,
    theme: &Theme,
    size: (u32, u32),
    state: &mut FrameState,
) -> RgbaImage {
    try_render_element_with_state(el, theme, size, state)
        .unwrap_or_else(|e| panic!("headless render failed: {e}"))
}

/// Fallible twin of [`render_element_with_state`].
///
/// # Errors
/// [`ShellError::NoDevice`] when no compute-capable wgpu adapter exists;
/// other [`ShellError`]s when the render itself fails.
pub fn try_render_element_with_state<Msg>(
    el: Element<Msg>,
    theme: &Theme,
    size: (u32, u32),
    state: &mut FrameState,
) -> Result<RgbaImage, ShellError> {
    // Clamp before layout so the frame and the texture agree on the size.
    let size = with_headless(|h| h.clamp_size(size.0, size.1))?;
    // Hold the font lock across both passes, then nest the headless lock for the
    // render (fonts → headless ordering, matching every other render site).
    with_fonts(|fonts| {
        #[expect(clippy::cast_precision_loss, reason = "window sizes fit in f32")]
        let frame = build_frame(
            &el,
            theme,
            fonts,
            state,
            (size.0 as f32, size.1 as f32),
            1.0,
        );
        with_headless(|headless| {
            headless.render_plan(&frame, fonts, state, size.0, size.1, theme.bg)
        })?
    })
}
