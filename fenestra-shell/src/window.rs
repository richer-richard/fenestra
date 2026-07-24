//! The windowed runners: winit event loop + wgpu surface + vello renderer.
//!
//! [`run_scene`] paints via a raw scene callback (no input). [`run_static`]
//! runs an element view with scrolling and animation frames; the full `App`
//! runner with messages arrives in M4 and builds on the same plumbing.

use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

#[cfg(not(target_arch = "wasm32"))]
use fenestra_core::Theme;
use fenestra_core::{
    App, Element, Fonts, FrameState, InputEvent, Key, KeyInput, build_frame, dispatch,
    refresh_hover,
};
use kurbo::Point;
use vello::peniko::Color;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu::{self, CurrentSurfaceTexture};
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{MouseScrollDelta, StartCause, WindowEvent};
#[cfg(not(target_arch = "wasm32"))]
use winit::event_loop::EventLoopProxy;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::ShellError;

/// One wheel "line" in logical pixels.
pub(crate) const LINE_SCROLL_PX: f64 = 40.0;

/// Extracts `(dx, dy)` logical-pixel deltas from a winit wheel event, honoring
/// the window scale for pixel deltas.
pub(crate) fn wheel_deltas(delta: MouseScrollDelta, scale: f64) -> (f64, f64) {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => {
            (f64::from(x) * LINE_SCROLL_PX, f64::from(y) * LINE_SCROLL_PX)
        }
        MouseScrollDelta::PixelDelta(pos) => (pos.x / scale, pos.y / scale),
    }
}

/// A raw paint callback: `(scene, logical_w, logical_h, background)`.
#[cfg(not(target_arch = "wasm32"))]
type PaintFn = Box<dyn FnMut(&mut Scene, f64, f64, Color)>;
/// A message-free element view function.
#[cfg(not(target_arch = "wasm32"))]
type ViewFn = Box<dyn Fn(&Theme) -> Element<()>>;

/// Options for the application window.
#[derive(Debug, Clone)]
pub struct WindowOptions {
    /// Window title.
    pub title: String,
    /// Initial inner size in logical pixels.
    pub inner_size: (f64, f64),
    /// Minimum inner size in logical pixels.
    pub min_size: Option<(f64, f64)>,
    /// Whether the window can be resized (true by default).
    pub resizable: bool,
    /// Open maximized.
    pub maximized: bool,
    /// Open borderless-fullscreen on the current monitor.
    pub fullscreen: bool,
    /// Window icon as straight-alpha RGBA8 `(width, height, pixels)`.
    pub icon: Option<(u32, u32, Vec<u8>)>,
    /// Custom faces registered on the runner's fonts before the first
    /// frame: design languages work in windows, not just headlessly.
    pub fonts: Vec<(fenestra_core::FamilyRole, Vec<u8>)>,
}

impl WindowOptions {
    /// A window with the given title and the default 1024x768 logical size.
    pub fn titled(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            inner_size: (1024.0, 768.0),
            min_size: None,
            resizable: true,
            maximized: false,
            fullscreen: false,
            icon: None,
            fonts: Vec::new(),
        }
    }

    /// Sets the initial inner size in logical pixels.
    pub fn with_size(mut self, width: f64, height: f64) -> Self {
        self.inner_size = (width, height);
        self
    }

    /// Sets the minimum inner size in logical pixels.
    pub fn with_min_size(mut self, width: f64, height: f64) -> Self {
        self.min_size = Some((width, height));
        self
    }

    /// Allows or forbids resizing (allowed by default).
    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Opens the window maximized.
    pub fn maximized(mut self) -> Self {
        self.maximized = true;
        self
    }

    /// Opens borderless-fullscreen on the current monitor.
    pub fn fullscreen(mut self) -> Self {
        self.fullscreen = true;
        self
    }

    /// Sets the window icon from straight-alpha RGBA8 pixels (ignored on
    /// platforms without window icons, including the web).
    pub fn with_icon(mut self, width: u32, height: u32, rgba: Vec<u8>) -> Self {
        self.icon = Some((width, height, rgba));
        self
    }

    /// Registers a custom face under a family role for this window's
    /// fonts (TTF/OTF bytes; see `Fonts::register`).
    pub fn with_font(mut self, role: fenestra_core::FamilyRole, data: Vec<u8>) -> Self {
        self.fonts.push((role, data));
        self
    }
}

enum RenderState {
    Active {
        surface: Box<RenderSurface<'static>>,
        valid_surface: bool,
        window: Arc<Window>,
    },
    Suspended(Option<Arc<Window>>),
    /// Window created; the async surface setup is in flight (web only).
    #[cfg(target_arch = "wasm32")]
    Pending(Arc<Window>),
}

/// Shared surface plumbing for every windowed runner.
struct WindowShell {
    context: RenderContext,
    renderers: Vec<Option<Renderer>>,
    state: RenderState,
    scene: Scene,
    options: WindowOptions,
    background: Color,
    /// The first unrecoverable failure (GPU/surface/renderer). Recorded by
    /// [`Self::fail`], which also stops the event loop; the runner entry
    /// function returns it once `run_app` unwinds. Runners with secondary
    /// windows funnel those failures into the main shell's slot too.
    fatal: Option<ShellError>,
    /// Completed async surface setup, parked until the next [`Self::pump`]
    /// (web only; the web is single-threaded so `Rc<RefCell>` suffices).
    #[cfg(target_arch = "wasm32")]
    ready: WasmReady,
}

/// The handoff slot for the web's async surface creation: the setup result,
/// carrying the error when WebGPU is unavailable.
#[cfg(target_arch = "wasm32")]
type WasmReady = std::rc::Rc<
    std::cell::RefCell<Option<Result<(RenderContext, Box<RenderSurface<'static>>), ShellError>>>,
>;

impl WindowShell {
    fn new(options: WindowOptions, background: Color) -> Self {
        Self {
            context: RenderContext::new(),
            renderers: Vec::new(),
            state: RenderState::Suspended(None),
            scene: Scene::new(),
            options,
            background,
            fatal: None,
            #[cfg(target_arch = "wasm32")]
            ready: WasmReady::default(),
        }
    }

    /// Records the first unrecoverable failure and stops the event loop; the
    /// runner entry function surfaces it as its `Err` return. Adversarial
    /// review 2026-07 (finding B): these paths used to be `expect`s — a VM
    /// without GPU drivers or a mid-run device loss took the process down
    /// instead of returning an actionable error.
    fn fail(&mut self, event_loop: &ActiveEventLoop, err: ShellError) {
        // The web loop cannot return an error (`run_app` already returned):
        // surface the failure in the browser console before stopping.
        #[cfg(target_arch = "wasm32")]
        web_sys::console::error_1(&format!("fenestra: {err}").into());
        if self.fatal.is_none() {
            self.fatal = Some(err);
        }
        event_loop.exit();
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) -> Result<(), ShellError> {
        self.resumed_with(event_loop, |_, _| {})
    }

    /// Like [`Self::resumed`], but runs `before_visible` between window
    /// creation and the first `set_visible(true)` — the AccessKit adapter
    /// must attach while the window is still hidden.
    fn resumed_with(
        &mut self,
        event_loop: &ActiveEventLoop,
        before_visible: impl FnOnce(&ActiveEventLoop, &Arc<Window>),
    ) -> Result<(), ShellError> {
        let cached = match &mut self.state {
            RenderState::Suspended(cached_window) => cached_window.take(),
            _ => return Ok(()),
        };
        let window = match cached {
            Some(window) => window,
            None => {
                let attrs = Window::default_attributes()
                    .with_title(self.options.title.clone())
                    .with_inner_size(LogicalSize::new(
                        self.options.inner_size.0,
                        self.options.inner_size.1,
                    ))
                    .with_resizable(self.options.resizable)
                    .with_maximized(self.options.maximized)
                    .with_visible(false);
                let attrs = match self.options.min_size {
                    Some((w, h)) => attrs.with_min_inner_size(LogicalSize::new(w, h)),
                    None => attrs,
                };
                let attrs = if self.options.fullscreen {
                    attrs.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
                } else {
                    attrs
                };
                #[cfg(not(target_arch = "wasm32"))]
                let attrs = match self.options.icon.clone() {
                    Some((w, h, rgba)) => match winit::window::Icon::from_rgba(rgba, w, h) {
                        Ok(icon) => attrs.with_window_icon(Some(icon)),
                        // Malformed icon data: open without one, never panic.
                        Err(_) => attrs,
                    },
                    None => attrs,
                };
                #[cfg(target_arch = "wasm32")]
                let attrs = {
                    use winit::platform::web::WindowAttributesExtWebSys;
                    // winit creates the canvas; have it inserted into the page.
                    attrs.with_append(true)
                };
                Arc::new(
                    event_loop
                        .create_window(attrs)
                        .map_err(ShellError::WindowCreate)?,
                )
            }
        };
        before_visible(event_loop, &window);
        let was_hidden = window.is_visible() == Some(false);
        self.activate(window.clone())?;
        if was_hidden {
            window.set_visible(true);
        }
        Ok(())
    }

    /// Builds (or rebuilds, after a lost surface) the swapchain for `window`
    /// and enters the active state.
    #[cfg(not(target_arch = "wasm32"))]
    fn activate(&mut self, window: Arc<Window>) -> Result<(), ShellError> {
        let size = window.inner_size();
        let surface = pollster::block_on(self.context.create_surface(
            window.clone(),
            size.width.max(1),
            size.height.max(1),
            wgpu::PresentMode::AutoVsync,
        ))
        .map_err(ShellError::Surface)?;

        self.renderers
            .resize_with(self.context.devices.len(), || None);
        if self.renderers[surface.dev_id].is_none() {
            self.renderers[surface.dev_id] = Some(
                Renderer::new(
                    &self.context.devices[surface.dev_id].device,
                    RendererOptions {
                        use_cpu: false,
                        antialiasing_support: AaSupport::area_only(),
                        ..Default::default()
                    },
                )
                .map_err(ShellError::Vello)?,
            );
        }

        self.state = RenderState::Active {
            surface: Box::new(surface),
            valid_surface: size.width != 0 && size.height != 0,
            window,
        };
        Ok(())
    }

    /// Web: surface/device setup is async — kick it off and park in
    /// `Pending`; [`Self::pump`] finishes the activation when it lands.
    /// Setup failure (no WebGPU) travels through the `ready` slot and
    /// surfaces on the next pump. Always `Ok` here.
    #[cfg(target_arch = "wasm32")]
    fn activate(&mut self, window: Arc<Window>) -> Result<(), ShellError> {
        let size = window.inner_size();
        let ready = std::rc::Rc::clone(&self.ready);
        let win = window.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let mut context = RenderContext::new();
            let surface = context
                .create_surface(
                    win.clone(),
                    size.width.max(1),
                    size.height.max(1),
                    wgpu::PresentMode::AutoVsync,
                )
                .await
                .map(|surface| (context, Box::new(surface)))
                .map_err(ShellError::Surface);
            *ready.borrow_mut() = Some(surface);
            win.request_redraw();
        });
        self.state = RenderState::Pending(window);
        Ok(())
    }

    /// Completes a pending web activation once the async setup finished.
    /// No-op on native and while nothing is pending.
    fn pump(&mut self) -> Result<(), ShellError> {
        #[cfg(target_arch = "wasm32")]
        if let RenderState::Pending(window) = &self.state
            && let Some(setup) = self.ready.borrow_mut().take()
        {
            let (context, surface) = setup?;
            let window = window.clone();
            self.context = context;
            self.renderers.clear();
            self.renderers
                .resize_with(self.context.devices.len(), || None);
            if self.renderers[surface.dev_id].is_none() {
                self.renderers[surface.dev_id] = Some(
                    Renderer::new(
                        &self.context.devices[surface.dev_id].device,
                        RendererOptions {
                            use_cpu: false,
                            antialiasing_support: AaSupport::area_only(),
                            ..Default::default()
                        },
                    )
                    .map_err(ShellError::Vello)?,
                );
            }
            let size = window.inner_size();
            self.state = RenderState::Active {
                surface,
                valid_surface: size.width != 0 && size.height != 0,
                window,
            };
        }
        Ok(())
    }

    fn suspended(&mut self) {
        if let RenderState::Active { window, .. } = &self.state {
            self.state = RenderState::Suspended(Some(window.clone()));
        }
    }

    fn window(&self) -> Option<&Arc<Window>> {
        match &self.state {
            RenderState::Active { window, .. } => Some(window),
            _ => None,
        }
    }

    fn resized(&mut self, width: u32, height: u32) {
        let RenderState::Active {
            surface,
            valid_surface,
            window,
        } = &mut self.state
        else {
            return;
        };
        if width != 0 && height != 0 {
            self.context.resize_surface(surface, width, height);
            *valid_surface = true;
        } else {
            *valid_surface = false;
        }
        window.request_redraw();
    }

    /// Logical size and scale factor of the active window.
    fn logical_size(&self) -> Option<(f64, f64, f64)> {
        match &self.state {
            RenderState::Active {
                surface, window, ..
            } => {
                let scale = window.scale_factor();
                Some((
                    f64::from(surface.config.width) / scale,
                    f64::from(surface.config.height) / scale,
                    scale,
                ))
            }
            _ => None,
        }
    }

    /// Scales the logical fragment to physical pixels and presents it.
    /// Recoverable surface states (lost/outdated/occluded/timeout) are
    /// handled in place; anything else is an error for [`Self::fail`].
    fn present(&mut self, fragment: &Scene) -> Result<(), ShellError> {
        let RenderState::Active {
            surface,
            valid_surface,
            window,
        } = &mut self.state
        else {
            return Ok(());
        };
        if !*valid_surface {
            return Ok(());
        }
        let width = surface.config.width;
        let height = surface.config.height;
        let scale = window.scale_factor();

        self.scene.reset();
        self.scene
            .append(fragment, Some(vello::kurbo::Affine::scale(scale)));

        let handle = &self.context.devices[surface.dev_id];
        self.renderers[surface.dev_id]
            .as_mut()
            .expect("renderer exists for surface device")
            .render_to_texture(
                &handle.device,
                &handle.queue,
                &self.scene,
                &surface.target_view,
                &RenderParams {
                    base_color: self.background,
                    width,
                    height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .map_err(ShellError::Vello)?;

        let surface_texture = match surface.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(texture) => texture,
            CurrentSurfaceTexture::Outdated | CurrentSurfaceTexture::Suboptimal(_) => {
                self.context.configure_surface(surface);
                window.request_redraw();
                return Ok(());
            }
            CurrentSurfaceTexture::Occluded => {
                // Hidden window: skip the frame; WindowEvent::Occluded(false)
                // requests the next redraw when it becomes visible again.
                return Ok(());
            }
            CurrentSurfaceTexture::Timeout => {
                window.request_redraw();
                return Ok(());
            }
            CurrentSurfaceTexture::Lost => {
                // Recoverable (GPU reset, driver update, display change):
                // rebuild the swapchain on the same window and repaint.
                let window = window.clone();
                window.request_redraw();
                return self.activate(window);
            }
            CurrentSurfaceTexture::Validation => return Err(ShellError::SurfaceValidation),
        };

        let mut encoder = handle
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("fenestra surface blit"),
            });
        surface.blitter.copy(
            &handle.device,
            &mut encoder,
            &surface.target_view,
            &surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default()),
        );
        handle.queue.submit([encoder.finish()]);
        surface_texture.present();
        handle
            .device
            .poll(wgpu::PollType::Poll)
            .map_err(ShellError::Poll)?;
        Ok(())
    }
}

// ------------------------------------------------------------- run_scene

/// Opens a window and repaints via `paint(scene, logical_w, logical_h, bg)`
/// on every redraw. Blocks until the window closes. Low-level escape hatch;
/// element views should prefer [`run_static`] (or the M4 `App` runner).
#[cfg(not(target_arch = "wasm32"))]
pub fn run_scene(
    options: WindowOptions,
    background: Color,
    paint: impl FnMut(&mut Scene, f64, f64, Color) + 'static,
) -> Result<(), ShellError> {
    let event_loop = EventLoop::new().map_err(ShellError::EventLoop)?;
    let mut app = SceneApp {
        shell: WindowShell::new(options, background),
        fragment: Scene::new(),
        paint: Box::new(paint),
    };
    let run = event_loop.run_app(&mut app).map_err(ShellError::EventLoop);
    match app.shell.fatal.take() {
        Some(err) => Err(err),
        None => run,
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct SceneApp {
    shell: WindowShell,
    fragment: Scene,
    paint: PaintFn,
}

#[cfg(not(target_arch = "wasm32"))]
impl ApplicationHandler for SceneApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(e) = self.shell.resumed(event_loop) {
            self.shell.fail(event_loop, e);
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.shell.suspended();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.shell.window().is_none_or(|w| w.id() != window_id) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self.shell.resized(size.width, size.height),
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(w) = self.shell.window() {
                    w.request_redraw();
                }
            }
            WindowEvent::Occluded(occluded) => {
                if !occluded && let Some(w) = self.shell.window() {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                let Some((lw, lh, _scale)) = self.shell.logical_size() else {
                    return;
                };
                self.fragment.reset();
                let bg = self.shell.background;
                (self.paint)(&mut self.fragment, lw, lh, bg);
                let fragment = std::mem::replace(&mut self.fragment, Scene::new());
                if let Err(e) = self.shell.present(&fragment) {
                    self.shell.fail(event_loop, e);
                }
                self.fragment = fragment;
            }
            _ => {}
        }
    }
}

/// A [`FrameState`] for a live window, seeded with the OS "reduce motion"
/// accessibility setting so animations snap for users who asked for it.
#[cfg(not(target_arch = "wasm32"))]
fn live_state() -> FrameState {
    let mut state = FrameState::new();
    state.reduced_motion = crate::reduce_motion::os_reduce_motion();
    state
}

/// The wasm build reads `prefers-reduced-motion` through the browser, so the
/// live state is the plain default here.
#[cfg(target_arch = "wasm32")]
fn live_state() -> FrameState {
    FrameState::new()
}

// ------------------------------------------------------------- run_static

/// Opens a window showing a message-free element view. The view is rebuilt
/// on every redraw; scroll state persists in a [`FrameState`]. Blocks until
/// the window closes.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_static(
    options: WindowOptions,
    theme: Theme,
    view: impl Fn(&Theme) -> Element<()> + 'static,
) -> Result<(), ShellError> {
    let event_loop = EventLoop::new().map_err(ShellError::EventLoop)?;
    let background = theme.bg;
    let mut fonts = Fonts::with_system();
    for (role, data) in &options.fonts {
        fonts.register(*role, data.clone());
    }
    let mut app = StaticApp {
        shell: WindowShell::new(options, background),
        theme,
        fonts,
        state: live_state(),
        view: Box::new(view),
        cursor: Point::ORIGIN,
        started: Instant::now(),
        last_frame: None,
    };
    let run = event_loop.run_app(&mut app).map_err(ShellError::EventLoop);
    match app.shell.fatal.take() {
        Some(err) => Err(err),
        None => run,
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct StaticApp {
    shell: WindowShell,
    theme: Theme,
    fonts: Fonts,
    state: FrameState,
    view: ViewFn,
    /// Cursor position in logical coordinates.
    cursor: Point,
    started: Instant,
    /// The frame from the last redraw, used to route input between frames.
    last_frame: Option<fenestra_core::Frame>,
}

#[cfg(not(target_arch = "wasm32"))]
impl StaticApp {
    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some((lw, lh, scale)) = self.shell.logical_size() else {
            return;
        };
        self.state.tick(self.started.elapsed().as_secs_f64());
        let el = (self.view)(&self.theme);
        #[expect(clippy::cast_possible_truncation, reason = "window sizes fit in f32")]
        let frame = build_frame(
            &el,
            &self.theme,
            &mut self.fonts,
            &mut self.state,
            (lw as f32, lh as f32),
            scale,
        );
        // Live-window limitation: the swapchain path renders a single pass, so
        // frosted glass shows its translucent tint without the CPU backdrop
        // blur (which needs a read-back). Headless rendering — the golden source
        // of truth — uses the two-pass `render_plan`. See ARCHITECTURE.md
        // ("Real frosted-glass backdrop blur").
        let scene = frame.paint(&mut self.fonts, &mut self.state);
        if let Err(e) = self.shell.present(&scene) {
            self.shell.fail(event_loop, e);
            return;
        }
        if frame.animating {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(16),
            ));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
        self.last_frame = Some(frame);
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ApplicationHandler for StaticApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(e) = self.shell.resumed(event_loop) {
            self.shell.fail(event_loop, e);
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.shell.suspended();
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::ResumeTimeReached { .. })
            && let Some(w) = self.shell.window()
        {
            w.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.shell.window().is_none_or(|w| w.id() != window_id) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self.shell.resized(size.width, size.height),
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(w) = self.shell.window() {
                    w.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.shell.window().map_or(1.0, |w| w.scale_factor());
                self.cursor = Point::new(position.x / scale, position.y / scale);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scale = self.shell.window().map_or(1.0, |w| w.scale_factor());
                let (dx, dy) = wheel_deltas(delta, scale);
                if let Some(frame) = &self.last_frame {
                    let y_target = (dy.abs() > 1e-3)
                        .then(|| frame.scrollable_y_at(self.cursor))
                        .flatten();
                    let x_target = (dx.abs() > 1e-3)
                        .then(|| frame.scrollable_x_at(self.cursor))
                        .flatten();
                    #[expect(clippy::cast_possible_truncation, reason = "scroll deltas fit in f32")]
                    {
                        if let Some(id) = y_target {
                            self.state.scroll_by(id, -dy as f32);
                        }
                        if let Some(id) = x_target {
                            self.state.scroll_by_x(id, -dx as f32);
                        }
                    }
                    if (y_target.is_some() || x_target.is_some())
                        && let Some(w) = self.shell.window()
                    {
                        w.request_redraw();
                    }
                }
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }
}

// ------------------------------------------------------------- run_app

/// User events crossing into the app runner's loop: type-erased app
/// messages from a [`fenestra_core::Proxy`] (any thread), and AccessKit's
/// activation/action events.
enum RunnerEvent {
    App(Box<dyn std::any::Any + Send>),
    #[cfg(not(target_arch = "wasm32"))]
    Access(accesskit_winit::Event),
}

#[cfg(not(target_arch = "wasm32"))]
impl From<accesskit_winit::Event> for RunnerEvent {
    fn from(event: accesskit_winit::Event) -> Self {
        Self::Access(event)
    }
}

/// Runs an [`App`]: the full Elm-shaped loop with hit testing, hover/active/
/// focus, keyboard navigation, message dispatch, and event-driven repaint
/// (animation frames only while something animates). Calls [`App::init`]
/// with a [`fenestra_core::Proxy`] before the first frame; proxied messages
/// wake the loop and repaint. Blocks until the window closes.
pub fn run_app<A: App + 'static>(mut app: A, options: WindowOptions) -> Result<(), ShellError>
where
    A::Msg: Send,
{
    let event_loop = EventLoop::<RunnerEvent>::with_user_event()
        .build()
        .map_err(ShellError::EventLoop)?;
    #[cfg(not(target_arch = "wasm32"))]
    let access_proxy = event_loop.create_proxy();
    let proxy = event_loop.create_proxy();
    app.init(fenestra_core::Proxy::new(move |msg: A::Msg| {
        // Dropped silently once the loop is gone (window closed).
        let _ = proxy.send_event(RunnerEvent::App(Box::new(msg)));
    }));
    let background = app.theme().bg;
    let mut fonts = Fonts::with_system();
    for (role, data) in &options.fonts {
        fonts.register(*role, data.clone());
    }
    #[cfg(target_arch = "wasm32")]
    let state = live_state();
    #[cfg(not(target_arch = "wasm32"))]
    let mut state = live_state();
    #[cfg(not(target_arch = "wasm32"))]
    state.set_clipboard(Box::new(crate::OsClipboard::default()));
    let runner = AppRunner {
        shell: WindowShell::new(options, background),
        app,
        fonts,
        state,
        cursor: Point::ORIGIN,
        started: Instant::now(),
        last: None,
        dirty: true,
        cached_scene: None,
        modifiers: winit::keyboard::ModifiersState::empty(),
        #[cfg(not(target_arch = "wasm32"))]
        adapter: None,
        #[cfg(not(target_arch = "wasm32"))]
        proxy: access_proxy,
        #[cfg(not(target_arch = "wasm32"))]
        secondary: std::collections::HashMap::new(),
    };
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut runner = runner;
        let run = event_loop
            .run_app(&mut runner)
            .map_err(ShellError::EventLoop);
        match runner.shell.fatal.take() {
            Some(err) => Err(err),
            None => run,
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        // Non-blocking on the web: the loop keeps running after main returns.
        event_loop.spawn_app(runner);
        Ok(())
    }
}

struct AppRunner<A: App> {
    shell: WindowShell,
    app: A,
    fonts: Fonts,
    state: FrameState,
    cursor: Point,
    started: Instant,
    /// View and frame from the last redraw, for input routing.
    last: Option<(Element<A::Msg>, fenestra_core::Frame)>,
    /// Anything changed since the last full frame? OS-driven redraws
    /// (expose, un-occlude) re-present the cached scene when clean —
    /// the whole build/layout/paint pipeline is skipped.
    dirty: bool,
    /// The last painted scene with its (logical w, h, scale) key.
    cached_scene: Option<(Scene, (f64, f64, f64))>,
    modifiers: winit::keyboard::ModifiersState,
    /// The AccessKit adapter, created before the window first shows.
    #[cfg(not(target_arch = "wasm32"))]
    adapter: Option<accesskit_winit::Adapter>,
    /// Loop proxy handed to the adapter for activation/action events.
    #[cfg(not(target_arch = "wasm32"))]
    proxy: EventLoopProxy<RunnerEvent>,
    /// Secondary windows declared by [`App::windows`], keyed by their
    /// stable key and reconciled after every update (native only).
    #[cfg(not(target_arch = "wasm32"))]
    secondary: std::collections::HashMap<String, SecondaryWindow<A>>,
}

/// One reconciled secondary window: its own surface, retained state, and
/// accessibility adapter; app state and fonts are shared.
#[cfg(not(target_arch = "wasm32"))]
struct SecondaryWindow<A: App> {
    shell: WindowShell,
    state: FrameState,
    cursor: Point,
    last: Option<(Element<A::Msg>, fenestra_core::Frame)>,
    on_close: A::Msg,
    title: String,
    adapter: Option<accesskit_winit::Adapter>,
}

impl<A: App> AppRunner<A> {
    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(e) = self.shell.pump() {
            self.shell.fail(event_loop, e);
            return;
        }
        let Some((lw, lh, scale)) = self.shell.logical_size() else {
            return;
        };
        // Clean frame at the same size: re-present the cached scene and
        // skip build/layout/paint entirely (expose/un-occlude redraws).
        if !self.dirty
            && let Some((scene, key)) = &self.cached_scene
            && *key == (lw, lh, scale)
        {
            if let Err(e) = self.shell.present(scene) {
                self.shell.fail(event_loop, e);
            }
            return;
        }
        let theme = self.app.theme();
        self.shell.background = theme.bg;
        self.state.tick(self.started.elapsed().as_secs_f64());
        #[expect(clippy::cast_possible_truncation, reason = "window sizes fit in f32")]
        let logical = (lw as f32, lh as f32);
        let view = self.app.view_at(fenestra_core::MAIN_WINDOW, logical);
        let frame = build_frame(
            &view,
            &theme,
            &mut self.fonts,
            &mut self.state,
            logical,
            scale,
        );
        // Single-pass live window: glass is tint-only here (see the `run_app`
        // redraw note); two-pass blur is the headless golden path.
        let scene = frame.paint(&mut self.fonts, &mut self.state);
        if let Err(e) = self.shell.present(&scene) {
            self.shell.fail(event_loop, e);
            return;
        }
        // The frame is clean until something changes it; animation and
        // hover refresh keep it dirty so the pipeline runs again.
        self.cached_scene = Some((scene, (lw, lh, scale)));
        self.dirty = frame.animating;
        // Content may have moved under a stationary pointer (scroll,
        // layout change): refresh hover and repaint once more if it did.
        if refresh_hover(&view, &frame, &mut self.state)
            && let Some(w) = self.shell.window()
        {
            self.dirty = true;
            w.request_redraw();
        }
        if frame.animating {
            #[cfg(not(target_arch = "wasm32"))]
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(16),
            ));
            // The browser paces frames; just ask for the next one.
            #[cfg(target_arch = "wasm32")]
            if let Some(w) = self.shell.window() {
                w.request_redraw();
            }
        } else {
            #[cfg(not(target_arch = "wasm32"))]
            let secondary_animating = self
                .secondary
                .values()
                .any(|b| b.last.as_ref().is_some_and(|(_, f)| f.animating));
            #[cfg(target_arch = "wasm32")]
            let secondary_animating = false;
            if !secondary_animating {
                event_loop.set_control_flow(ControlFlow::Wait);
            }
        }
        self.last = Some((view, frame));
        // Anchor the IME candidate window to the focused caret.
        if let Some(caret) = self.state.ime_caret()
            && let Some(w) = self.shell.window()
        {
            w.set_ime_cursor_area(
                winit::dpi::LogicalPosition::new(caret.x0, caret.y0),
                winit::dpi::LogicalSize::new(1.0, caret.height()),
            );
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.push_access_tree();
    }

    /// Pushes the current frame's accessibility projection to the platform
    /// (no-op until assistive technology activates the tree).
    #[cfg(not(target_arch = "wasm32"))]
    fn push_access_tree(&mut self) {
        let scale = self.shell.window().map_or(1.0, |w| w.scale_factor());
        let focus = self.state.focused();
        if let Some(adapter) = &mut self.adapter
            && let Some((_, frame)) = &self.last
        {
            adapter.update_if_active(|| crate::access::tree_update(frame, focus, scale));
        }
    }

    fn input(&mut self, event: InputEvent) -> bool {
        let Some((view, frame)) = &self.last else {
            return false;
        };
        let result = dispatch(view, frame, &mut self.state, &mut self.fonts, event);
        if let Some(cursor) = result.cursor
            && let Some(w) = self.shell.window()
        {
            w.set_cursor(winit::window::Cursor::Icon(map_cursor(cursor)));
        }
        let had_msgs = !result.msgs.is_empty();
        for msg in result.msgs {
            self.app.update(msg);
        }
        if result.redraw || had_msgs {
            self.dirty = true;
            if let Some(w) = self.shell.window() {
                w.request_redraw();
            }
        }
        had_msgs
    }

    /// Routes one input event into the main window, reconciling windows
    /// afterwards if it produced messages.
    fn input_main(&mut self, event_loop: &ActiveEventLoop, event: InputEvent) {
        if self.input(event) {
            self.after_update(event_loop);
        }
    }

    /// Routes one input event into a secondary window, reconciling
    /// windows afterwards if it produced messages.
    #[cfg(not(target_arch = "wasm32"))]
    fn secondary_input_main(&mut self, key: &str, event_loop: &ActiveEventLoop, event: InputEvent) {
        if self.secondary_input(key, event) {
            self.after_update(event_loop);
        }
    }

    /// Reconciles secondary windows against [`App::windows`] and asks
    /// every window for a repaint — called whenever messages were applied.
    fn after_update(&mut self, event_loop: &ActiveEventLoop) {
        self.dirty = true;
        #[cfg(not(target_arch = "wasm32"))]
        self.reconcile_windows(event_loop);
        #[cfg(target_arch = "wasm32")]
        let _ = event_loop;
        if let Some(w) = self.shell.window() {
            w.request_redraw();
        }
        #[cfg(not(target_arch = "wasm32"))]
        for bundle in self.secondary.values() {
            if let Some(w) = bundle.shell.window() {
                w.request_redraw();
            }
        }
    }

    /// Opens, closes, and retitles secondary windows to match the app's
    /// declared list.
    #[cfg(not(target_arch = "wasm32"))]
    fn reconcile_windows(&mut self, event_loop: &ActiveEventLoop) {
        let desired = self.app.windows();
        self.secondary
            .retain(|key, _| desired.iter().any(|d| &d.key == key));
        for desc in desired {
            match self.secondary.get_mut(&desc.key) {
                Some(bundle) => {
                    bundle.on_close = desc.on_close;
                    if bundle.title != desc.title {
                        bundle.title.clone_from(&desc.title);
                        if let Some(w) = bundle.shell.window() {
                            w.set_title(&desc.title);
                        }
                    }
                }
                None => {
                    let mut shell = WindowShell::new(
                        WindowOptions::titled(desc.title.clone())
                            .with_size(desc.size.0, desc.size.1),
                        self.shell.background,
                    );
                    let proxy = self.proxy.clone();
                    let mut adapter = None;
                    if let Err(e) = shell.resumed_with(event_loop, |el, window| {
                        adapter = Some(accesskit_winit::Adapter::with_event_loop_proxy(
                            el, window, proxy,
                        ));
                    }) {
                        // Failing to open a declared window is app-fatal: the
                        // GPU/window stack is broken, not just this window.
                        self.shell.fail(event_loop, e);
                        return;
                    }
                    if let Some(w) = shell.window() {
                        w.set_ime_allowed(true);
                        w.request_redraw();
                    }
                    let mut state = live_state();
                    state.set_clipboard(Box::new(crate::OsClipboard::default()));
                    self.secondary.insert(
                        desc.key.clone(),
                        SecondaryWindow {
                            shell,
                            state,
                            cursor: Point::ORIGIN,
                            last: None,
                            on_close: desc.on_close,
                            title: desc.title,
                            adapter,
                        },
                    );
                }
            }
        }
    }

    /// Redraws one secondary window: the same pipeline as the main one,
    /// against its own retained state and `view_for(key)`.
    #[cfg(not(target_arch = "wasm32"))]
    fn secondary_redraw(&mut self, key: &str, event_loop: &ActiveEventLoop) {
        let theme = self.app.theme_for(key);
        let now = self.started.elapsed().as_secs_f64();
        let Some(bundle) = self.secondary.get_mut(key) else {
            return;
        };
        if let Err(e) = bundle.shell.pump() {
            self.shell.fail(event_loop, e);
            return;
        }
        let Some((lw, lh, scale)) = bundle.shell.logical_size() else {
            return;
        };
        bundle.shell.background = theme.bg;
        bundle.state.tick(now);
        #[expect(clippy::cast_possible_truncation, reason = "window sizes fit in f32")]
        let logical = (lw as f32, lh as f32);
        let view = self.app.view_at(key, logical);
        let frame = build_frame(
            &view,
            &theme,
            &mut self.fonts,
            &mut bundle.state,
            logical,
            scale,
        );
        // Single-pass live window: glass is tint-only here (see the `run_app`
        // redraw note); two-pass blur is the headless golden path.
        let scene = frame.paint(&mut self.fonts, &mut bundle.state);
        if let Err(e) = bundle.shell.present(&scene) {
            // Secondary failures stop the app through the main shell's
            // channel: device-level errors are app-fatal, not per-window.
            self.shell.fail(event_loop, e);
            return;
        }
        if refresh_hover(&view, &frame, &mut bundle.state)
            && let Some(w) = bundle.shell.window()
        {
            w.request_redraw();
        }
        if frame.animating {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(16),
            ));
        }
        if let Some(caret) = bundle.state.ime_caret()
            && let Some(w) = bundle.shell.window()
        {
            w.set_ime_cursor_area(
                winit::dpi::LogicalPosition::new(caret.x0, caret.y0),
                winit::dpi::LogicalSize::new(1.0, caret.height()),
            );
        }
        bundle.last = Some((view, frame));
        let focus = bundle.state.focused();
        if let Some(adapter) = &mut bundle.adapter
            && let Some((_, frame)) = &bundle.last
        {
            adapter.update_if_active(|| crate::access::tree_update(frame, focus, scale));
        }
    }

    /// The full event handler for one secondary window — the same arms as
    /// the main window, against the bundle's own surface and state.
    #[cfg(not(target_arch = "wasm32"))]
    fn secondary_window_event(
        &mut self,
        key: &str,
        event_loop: &ActiveEventLoop,
        event: WindowEvent,
    ) {
        if let Some(bundle) = self.secondary.get_mut(key)
            && let Some(window) = bundle.shell.window()
            && let Some(adapter) = &mut bundle.adapter
        {
            adapter.process_event(window, &event);
        }
        match event {
            WindowEvent::CloseRequested => {
                if let Some(msg) = self.secondary.get(key).map(|b| b.on_close.clone()) {
                    self.app.update(msg);
                    self.after_update(event_loop);
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(bundle) = self.secondary.get_mut(key) {
                    bundle.shell.resized(size.width, size.height);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } | WindowEvent::Occluded(false) => {
                if let Some(w) = self.secondary.get(key).and_then(|b| b.shell.window()) {
                    w.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
                let m = self.modifiers;
                self.secondary_input_main(
                    key,
                    event_loop,
                    InputEvent::Modifiers {
                        shift: m.shift_key(),
                        ctrl: m.control_key(),
                        alt: m.alt_key(),
                        meta: m.super_key(),
                    },
                );
            }
            WindowEvent::DroppedFile(path) => {
                self.secondary_input_main(key, event_loop, InputEvent::FileDrop(path));
            }
            WindowEvent::CursorLeft { .. } => {
                self.secondary_input_main(key, event_loop, InputEvent::PointerLeave);
            }
            WindowEvent::CursorMoved { position, .. } => {
                let Some(bundle) = self.secondary.get_mut(key) else {
                    return;
                };
                let scale = bundle.shell.window().map_or(1.0, |w| w.scale_factor());
                bundle.cursor = Point::new(position.x / scale, position.y / scale);
                #[expect(clippy::cast_possible_truncation, reason = "positions fit in f32")]
                let (x, y) = (bundle.cursor.x as f32, bundle.cursor.y as f32);
                self.secondary_input_main(key, event_loop, InputEvent::PointerMove { x, y });
            }
            WindowEvent::MouseInput {
                state,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                self.secondary_input_main(
                    key,
                    event_loop,
                    match state {
                        winit::event::ElementState::Pressed => InputEvent::PointerDown,
                        winit::event::ElementState::Released => InputEvent::PointerUp,
                    },
                );
            }
            WindowEvent::MouseInput {
                state,
                button: winit::event::MouseButton::Right,
                ..
            } => {
                self.secondary_input_main(
                    key,
                    event_loop,
                    match state {
                        winit::event::ElementState::Pressed => InputEvent::RightDown,
                        winit::event::ElementState::Released => InputEvent::RightUp,
                    },
                );
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scale = self
                    .secondary
                    .get(key)
                    .and_then(|b| b.shell.window())
                    .map_or(1.0, |w| w.scale_factor());
                let (dx, dy) = wheel_deltas(delta, scale);
                #[expect(clippy::cast_possible_truncation, reason = "deltas fit in f32")]
                self.secondary_input_main(
                    key,
                    event_loop,
                    InputEvent::Wheel {
                        dx: dx as f32,
                        dy: dy as f32,
                    },
                );
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == winit::event::ElementState::Pressed =>
            {
                let mods = self.modifiers;
                let printable = !mods.control_key()
                    && !mods.super_key()
                    && event
                        .text
                        .as_ref()
                        .is_some_and(|t| !t.is_empty() && t.chars().all(|c| !c.is_control()));
                if printable {
                    if let Some(t) = &event.text {
                        self.secondary_input_main(key, event_loop, InputEvent::Text(t.to_string()));
                    }
                } else if let Some(input) = map_key(&event, mods) {
                    self.secondary_input_main(key, event_loop, input);
                }
            }
            WindowEvent::Ime(ime) => match ime {
                winit::event::Ime::Preedit(text, cursor) => {
                    self.secondary_input_main(
                        key,
                        event_loop,
                        InputEvent::ImePreedit { text, cursor },
                    );
                }
                winit::event::Ime::Commit(text) => {
                    self.secondary_input_main(key, event_loop, InputEvent::Text(text));
                }
                winit::event::Ime::Enabled | winit::event::Ime::Disabled => {}
            },
            WindowEvent::RedrawRequested => self.secondary_redraw(key, event_loop),
            _ => {}
        }
    }

    /// Dispatches one input event against a secondary window. Returns
    /// whether messages were applied (the caller then reconciles).
    #[cfg(not(target_arch = "wasm32"))]
    fn secondary_input(&mut self, key: &str, event: InputEvent) -> bool {
        let Some(bundle) = self.secondary.get_mut(key) else {
            return false;
        };
        let Some((view, frame)) = &bundle.last else {
            return false;
        };
        let result = dispatch(view, frame, &mut bundle.state, &mut self.fonts, event);
        if let Some(cursor) = result.cursor
            && let Some(w) = bundle.shell.window()
        {
            w.set_cursor(winit::window::Cursor::Icon(map_cursor(cursor)));
        }
        let had_msgs = !result.msgs.is_empty();
        if (result.redraw || had_msgs)
            && let Some(w) = bundle.shell.window()
        {
            w.request_redraw();
        }
        let msgs = result.msgs;
        for msg in msgs {
            self.app.update(msg);
        }
        had_msgs
    }
}

pub(crate) fn map_cursor(cursor: fenestra_core::Cursor) -> winit::window::CursorIcon {
    match cursor {
        fenestra_core::Cursor::Default => winit::window::CursorIcon::Default,
        fenestra_core::Cursor::Pointer => winit::window::CursorIcon::Pointer,
        fenestra_core::Cursor::Text => winit::window::CursorIcon::Text,
        fenestra_core::Cursor::NotAllowed => winit::window::CursorIcon::NotAllowed,
    }
}

/// Translates a winit key event into a fenestra [`InputEvent`].
pub(crate) fn map_key(
    event: &winit::event::KeyEvent,
    mods: winit::keyboard::ModifiersState,
) -> Option<InputEvent> {
    use winit::keyboard::{Key as WKey, NamedKey};
    let key = match &event.logical_key {
        WKey::Named(NamedKey::Tab) => {
            return Some(if mods.shift_key() {
                InputEvent::ShiftTab
            } else {
                InputEvent::Tab
            });
        }
        WKey::Named(named) => match named {
            NamedKey::Enter => Key::Enter,
            NamedKey::Space => Key::Space,
            NamedKey::Escape => Key::Escape,
            NamedKey::ArrowLeft => Key::ArrowLeft,
            NamedKey::ArrowRight => Key::ArrowRight,
            NamedKey::ArrowUp => Key::ArrowUp,
            NamedKey::ArrowDown => Key::ArrowDown,
            NamedKey::Home => Key::Home,
            NamedKey::End => Key::End,
            NamedKey::Backspace => Key::Backspace,
            NamedKey::Delete => Key::Delete,
            NamedKey::PageUp => Key::PageUp,
            NamedKey::PageDown => Key::PageDown,
            _ => return None,
        },
        WKey::Character(s) => Key::Char(s.chars().next()?),
        _ => return None,
    };
    Some(InputEvent::Key(KeyInput {
        key,
        shift: mods.shift_key(),
        ctrl: mods.control_key(),
        alt: mods.alt_key(),
        meta: mods.super_key(),
    }))
}

impl<A: App> ApplicationHandler<RunnerEvent> for AppRunner<A> {
    #[cfg(not(target_arch = "wasm32"))]
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Fresh surface, possibly fresh scale: rebuild rather than trust
        // a scene cached across the suspend.
        self.dirty = true;
        let adapter = &mut self.adapter;
        let proxy = self.proxy.clone();
        if let Err(e) = self.shell.resumed_with(event_loop, |el, window| {
            // The adapter must attach while the window is still hidden.
            if adapter.is_none() {
                *adapter = Some(accesskit_winit::Adapter::with_event_loop_proxy(
                    el, window, proxy,
                ));
            }
        }) {
            self.shell.fail(event_loop, e);
            return;
        }
        if let Some(w) = self.shell.window() {
            w.set_ime_allowed(true);
        }
        for bundle in self.secondary.values_mut() {
            if let Err(e) = bundle.shell.resumed(event_loop) {
                self.shell.fail(event_loop, e);
                return;
            }
        }
        self.reconcile_windows(event_loop);
    }

    #[cfg(target_arch = "wasm32")]
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.dirty = true;
        if let Err(e) = self.shell.resumed(event_loop) {
            self.shell.fail(event_loop, e);
            return;
        }
        if let Some(w) = self.shell.window() {
            w.set_ime_allowed(true);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: RunnerEvent) {
        match event {
            RunnerEvent::App(msg) => {
                if let Ok(msg) = msg.downcast::<A::Msg>() {
                    self.app.update(*msg);
                    self.after_update(event_loop);
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            RunnerEvent::Access(ev) => {
                // Route by window id: `None` is the main window, `Some(key)`
                // a secondary one; unknown ids are stale events.
                let is_main = self.shell.window().is_some_and(|w| w.id() == ev.window_id);
                let skey = (!is_main)
                    .then(|| {
                        self.secondary
                            .iter()
                            .find(|(_, b)| b.shell.window().is_some_and(|w| w.id() == ev.window_id))
                            .map(|(k, _)| k.clone())
                    })
                    .flatten();
                if !is_main && skey.is_none() {
                    return;
                }
                match ev.window_event {
                    accesskit_winit::WindowEvent::InitialTreeRequested => match &skey {
                        None => {
                            if self.last.is_some() {
                                self.push_access_tree();
                            } else if let Some(w) = self.shell.window() {
                                w.request_redraw();
                            }
                        }
                        Some(key) => {
                            if let Some(bundle) = self.secondary.get_mut(key) {
                                let scale = bundle.shell.window().map_or(1.0, |w| w.scale_factor());
                                let focus = bundle.state.focused();
                                if let Some((_, frame)) = &bundle.last {
                                    if let Some(adapter) = &mut bundle.adapter {
                                        adapter.update_if_active(|| {
                                            crate::access::tree_update(frame, focus, scale)
                                        });
                                    }
                                } else if let Some(w) = bundle.shell.window() {
                                    w.request_redraw();
                                }
                            }
                        }
                    },
                    accesskit_winit::WindowEvent::ActionRequested(req) => {
                        let id = fenestra_core::WidgetId(req.target_node.0);
                        match req.action {
                            accesskit::Action::Click => {
                                let msg = match &skey {
                                    None => self.last.as_ref().and_then(|(view, frame)| {
                                        fenestra_core::click_msg_of(view, frame, &self.state, id)
                                    }),
                                    Some(key) => self.secondary.get(key).and_then(|bundle| {
                                        bundle.last.as_ref().and_then(|(view, frame)| {
                                            fenestra_core::click_msg_of(
                                                view,
                                                frame,
                                                &bundle.state,
                                                id,
                                            )
                                        })
                                    }),
                                };
                                if let Some(msg) = msg {
                                    self.app.update(msg);
                                    self.after_update(event_loop);
                                }
                            }
                            accesskit::Action::Focus => match &skey {
                                None => {
                                    self.state.set_focus(Some(id));
                                    self.dirty = true;
                                    if let Some(w) = self.shell.window() {
                                        w.request_redraw();
                                    }
                                }
                                Some(key) => {
                                    if let Some(bundle) = self.secondary.get_mut(key) {
                                        bundle.state.set_focus(Some(id));
                                        if let Some(w) = bundle.shell.window() {
                                            w.request_redraw();
                                        }
                                    }
                                }
                            },
                            _ => {}
                        }
                    }
                    accesskit_winit::WindowEvent::AccessibilityDeactivated => {}
                }
            }
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.shell.suspended();
        #[cfg(not(target_arch = "wasm32"))]
        for bundle in self.secondary.values_mut() {
            bundle.shell.suspended();
        }
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if !matches!(cause, StartCause::ResumeTimeReached { .. }) {
            return;
        }
        if let Some(w) = self.shell.window() {
            w.request_redraw();
        }
        #[cfg(not(target_arch = "wasm32"))]
        for bundle in self.secondary.values() {
            if let Some(w) = bundle.shell.window() {
                w.request_redraw();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.shell.window().is_none_or(|w| w.id() != window_id) {
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(key) = self
                .secondary
                .iter()
                .find(|(_, b)| b.shell.window().is_some_and(|w| w.id() == window_id))
                .map(|(k, _)| k.clone())
            {
                self.secondary_window_event(&key, event_loop, event);
            }
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(adapter) = &mut self.adapter
            && let Some(window) = self.shell.window()
        {
            adapter.process_event(window, &event);
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                // The cache key also guards size, but coalesced resizes can
                // land back on the cached geometry mid-drag.
                self.dirty = true;
                self.shell.resized(size.width, size.height);
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.dirty = true;
                if let Some(w) = self.shell.window() {
                    w.request_redraw();
                }
            }
            WindowEvent::Focused(true) => {
                // Re-read the OS "reduce motion" setting on focus, so a change
                // made while the app was in the background takes effect.
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let reduce = crate::reduce_motion::os_reduce_motion();
                    if reduce != self.state.reduced_motion {
                        self.state.reduced_motion = reduce;
                        self.dirty = true;
                        if let Some(w) = self.shell.window() {
                            w.request_redraw();
                        }
                    }
                }
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
                let m = self.modifiers;
                self.input_main(
                    event_loop,
                    InputEvent::Modifiers {
                        shift: m.shift_key(),
                        ctrl: m.control_key(),
                        alt: m.alt_key(),
                        meta: m.super_key(),
                    },
                );
            }
            WindowEvent::Occluded(occluded) => {
                if !occluded && let Some(w) = self.shell.window() {
                    w.request_redraw();
                }
            }
            WindowEvent::DroppedFile(path) => {
                self.input_main(event_loop, InputEvent::FileDrop(path))
            }
            WindowEvent::CursorLeft { .. } => self.input_main(event_loop, InputEvent::PointerLeave),
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.shell.window().map_or(1.0, |w| w.scale_factor());
                self.cursor = Point::new(position.x / scale, position.y / scale);
                #[expect(clippy::cast_possible_truncation, reason = "positions fit in f32")]
                self.input_main(
                    event_loop,
                    InputEvent::PointerMove {
                        x: self.cursor.x as f32,
                        y: self.cursor.y as f32,
                    },
                );
            }
            WindowEvent::MouseInput {
                state,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                self.input_main(
                    event_loop,
                    match state {
                        winit::event::ElementState::Pressed => InputEvent::PointerDown,
                        winit::event::ElementState::Released => InputEvent::PointerUp,
                    },
                );
            }
            WindowEvent::MouseInput {
                state,
                button: winit::event::MouseButton::Right,
                ..
            } => {
                self.input_main(
                    event_loop,
                    match state {
                        winit::event::ElementState::Pressed => InputEvent::RightDown,
                        winit::event::ElementState::Released => InputEvent::RightUp,
                    },
                );
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scale = self.shell.window().map_or(1.0, |w| w.scale_factor());
                let (dx, dy) = wheel_deltas(delta, scale);
                #[expect(clippy::cast_possible_truncation, reason = "deltas fit in f32")]
                self.input_main(
                    event_loop,
                    InputEvent::Wheel {
                        dx: dx as f32,
                        dy: dy as f32,
                    },
                );
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == winit::event::ElementState::Pressed =>
            {
                {
                    let mods = self.modifiers;
                    // Printable input arrives as Text (it may be multi-char);
                    // named keys and shortcuts go through Key.
                    let printable = !mods.control_key()
                        && !mods.super_key()
                        && event
                            .text
                            .as_ref()
                            .is_some_and(|t| !t.is_empty() && t.chars().all(|c| !c.is_control()));
                    if printable {
                        if let Some(t) = &event.text {
                            self.input_main(event_loop, InputEvent::Text(t.to_string()));
                        }
                    } else if let Some(input) = map_key(&event, mods) {
                        self.input_main(event_loop, input);
                    }
                }
            }
            WindowEvent::Ime(ime) => match ime {
                winit::event::Ime::Preedit(text, cursor) => {
                    self.input_main(event_loop, InputEvent::ImePreedit { text, cursor });
                }
                winit::event::Ime::Commit(text) => {
                    self.input_main(event_loop, InputEvent::Text(text));
                }
                winit::event::Ime::Enabled | winit::event::Ime::Disabled => {}
            },
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }
}
