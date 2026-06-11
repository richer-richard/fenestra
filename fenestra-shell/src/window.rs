//! The windowed runners: winit event loop + wgpu surface + vello renderer.
//!
//! [`run_scene`] paints via a raw scene callback (no input). [`run_static`]
//! runs an element view with scrolling and animation frames; the full `App`
//! runner with messages arrives in M4 and builds on the same plumbing.

use std::sync::Arc;
use std::time::{Duration, Instant};

use fenestra_core::{
    App, Element, Fonts, FrameState, InputEvent, Key, KeyInput, Theme, build_frame, dispatch,
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
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::ShellError;

/// One wheel "line" in logical pixels.
const LINE_SCROLL_PX: f64 = 40.0;

/// A raw paint callback: `(scene, logical_w, logical_h, background)`.
type PaintFn = Box<dyn FnMut(&mut Scene, f64, f64, Color)>;
/// A message-free element view function.
type ViewFn = Box<dyn Fn(&Theme) -> Element<()>>;

/// Options for the application window.
#[derive(Debug, Clone)]
pub struct WindowOptions {
    /// Window title.
    pub title: String,
    /// Initial inner size in logical pixels.
    pub inner_size: (f64, f64),
}

impl WindowOptions {
    /// A window with the given title and the default 1024x768 logical size.
    pub fn titled(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            inner_size: (1024.0, 768.0),
        }
    }

    /// Sets the initial inner size in logical pixels.
    pub fn with_size(mut self, width: f64, height: f64) -> Self {
        self.inner_size = (width, height);
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
}

/// Shared surface plumbing for every windowed runner.
struct WindowShell {
    context: RenderContext,
    renderers: Vec<Option<Renderer>>,
    state: RenderState,
    scene: Scene,
    options: WindowOptions,
    background: Color,
}

impl WindowShell {
    fn new(options: WindowOptions, background: Color) -> Self {
        Self {
            context: RenderContext::new(),
            renderers: Vec::new(),
            state: RenderState::Suspended(None),
            scene: Scene::new(),
            options,
            background,
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let RenderState::Suspended(cached_window) = &mut self.state else {
            return;
        };
        let window = cached_window.take().unwrap_or_else(|| {
            let attrs = Window::default_attributes()
                .with_title(self.options.title.clone())
                .with_inner_size(LogicalSize::new(
                    self.options.inner_size.0,
                    self.options.inner_size.1,
                ));
            Arc::new(
                event_loop
                    .create_window(attrs)
                    .expect("failed to create window"),
            )
        });
        self.activate(window);
    }

    /// Builds (or rebuilds, after a lost surface) the swapchain for `window`
    /// and enters the active state.
    fn activate(&mut self, window: Arc<Window>) {
        let size = window.inner_size();
        let surface = pollster::block_on(self.context.create_surface(
            window.clone(),
            size.width.max(1),
            size.height.max(1),
            wgpu::PresentMode::AutoVsync,
        ))
        .expect("failed to create wgpu surface");

        self.renderers
            .resize_with(self.context.devices.len(), || None);
        self.renderers[surface.dev_id].get_or_insert_with(|| {
            Renderer::new(
                &self.context.devices[surface.dev_id].device,
                RendererOptions {
                    use_cpu: false,
                    antialiasing_support: AaSupport::area_only(),
                    ..Default::default()
                },
            )
            .expect("failed to create vello renderer")
        });

        self.state = RenderState::Active {
            surface: Box::new(surface),
            valid_surface: size.width != 0 && size.height != 0,
            window,
        };
    }

    fn suspended(&mut self) {
        if let RenderState::Active { window, .. } = &self.state {
            self.state = RenderState::Suspended(Some(window.clone()));
        }
    }

    fn window(&self) -> Option<&Arc<Window>> {
        match &self.state {
            RenderState::Active { window, .. } => Some(window),
            RenderState::Suspended(_) => None,
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
            RenderState::Suspended(_) => None,
        }
    }

    /// Scales the logical fragment to physical pixels and presents it.
    fn present(&mut self, fragment: &Scene) {
        let RenderState::Active {
            surface,
            valid_surface,
            window,
        } = &mut self.state
        else {
            return;
        };
        if !*valid_surface {
            return;
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
            .expect("vello render failed");

        let surface_texture = match surface.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(texture) => texture,
            CurrentSurfaceTexture::Outdated | CurrentSurfaceTexture::Suboptimal(_) => {
                self.context.configure_surface(surface);
                window.request_redraw();
                return;
            }
            CurrentSurfaceTexture::Occluded => {
                // Hidden window: skip the frame; WindowEvent::Occluded(false)
                // requests the next redraw when it becomes visible again.
                return;
            }
            CurrentSurfaceTexture::Timeout => {
                window.request_redraw();
                return;
            }
            CurrentSurfaceTexture::Lost => {
                // Recoverable (GPU reset, driver update, display change):
                // rebuild the swapchain on the same window and repaint.
                let window = window.clone();
                window.request_redraw();
                self.activate(window);
                return;
            }
            CurrentSurfaceTexture::Validation => {
                panic!("validation error acquiring wgpu surface texture")
            }
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
        handle.device.poll(wgpu::PollType::Poll).unwrap();
    }
}

// ------------------------------------------------------------- run_scene

/// Opens a window and repaints via `paint(scene, logical_w, logical_h, bg)`
/// on every redraw. Blocks until the window closes. Low-level escape hatch;
/// element views should prefer [`run_static`] (or the M4 `App` runner).
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
    event_loop.run_app(&mut app).map_err(ShellError::EventLoop)
}

struct SceneApp {
    shell: WindowShell,
    fragment: Scene,
    paint: PaintFn,
}

impl ApplicationHandler for SceneApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.shell.resumed(event_loop);
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
                self.shell.present(&fragment);
                self.fragment = fragment;
            }
            _ => {}
        }
    }
}

// ------------------------------------------------------------- run_static

/// Opens a window showing a message-free element view. The view is rebuilt
/// on every redraw; scroll state persists in a [`FrameState`]. Blocks until
/// the window closes.
pub fn run_static(
    options: WindowOptions,
    theme: Theme,
    view: impl Fn(&Theme) -> Element<()> + 'static,
) -> Result<(), ShellError> {
    let event_loop = EventLoop::new().map_err(ShellError::EventLoop)?;
    let background = theme.bg;
    let mut app = StaticApp {
        shell: WindowShell::new(options, background),
        theme,
        fonts: Fonts::with_system(),
        state: FrameState::new(),
        view: Box::new(view),
        cursor: Point::ORIGIN,
        started: Instant::now(),
        last_frame: None,
    };
    event_loop.run_app(&mut app).map_err(ShellError::EventLoop)
}

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
        let scene = frame.paint(&mut self.fonts, &mut self.state);
        self.shell.present(&scene);
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

impl ApplicationHandler for StaticApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.shell.resumed(event_loop);
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
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(y) * LINE_SCROLL_PX,
                    MouseScrollDelta::PixelDelta(pos) => {
                        let scale = self.shell.window().map_or(1.0, |w| w.scale_factor());
                        pos.y / scale
                    }
                };
                if let Some(frame) = &self.last_frame
                    && let Some(id) = frame.scrollable_at(self.cursor)
                {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "scroll deltas fit in f32"
                    )]
                    self.state.scroll_by(id, -dy as f32);
                    if let Some(w) = self.shell.window() {
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

/// A type-erased app message crossing from a [`fenestra_core::Proxy`]
/// (any thread) into the event loop.
struct ProxyEvent(Box<dyn std::any::Any + Send>);

/// Runs an [`App`]: the full Elm-shaped loop with hit testing, hover/active/
/// focus, keyboard navigation, message dispatch, and event-driven repaint
/// (animation frames only while something animates). Calls [`App::init`]
/// with a [`fenestra_core::Proxy`] before the first frame; proxied messages
/// wake the loop and repaint. Blocks until the window closes.
pub fn run_app<A: App + 'static>(mut app: A, options: WindowOptions) -> Result<(), ShellError>
where
    A::Msg: Send,
{
    let event_loop = EventLoop::<ProxyEvent>::with_user_event()
        .build()
        .map_err(ShellError::EventLoop)?;
    let proxy = event_loop.create_proxy();
    app.init(fenestra_core::Proxy::new(move |msg: A::Msg| {
        // Dropped silently once the loop is gone (window closed).
        let _ = proxy.send_event(ProxyEvent(Box::new(msg)));
    }));
    let background = app.theme().bg;
    let mut state = FrameState::new();
    state.set_clipboard(Box::new(crate::OsClipboard::default()));
    let mut runner = AppRunner {
        shell: WindowShell::new(options, background),
        app,
        fonts: Fonts::with_system(),
        state,
        cursor: Point::ORIGIN,
        started: Instant::now(),
        last: None,
        modifiers: winit::keyboard::ModifiersState::empty(),
    };
    event_loop
        .run_app(&mut runner)
        .map_err(ShellError::EventLoop)
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
    modifiers: winit::keyboard::ModifiersState,
}

impl<A: App> AppRunner<A> {
    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some((lw, lh, scale)) = self.shell.logical_size() else {
            return;
        };
        let theme = self.app.theme();
        self.shell.background = theme.bg;
        self.state.tick(self.started.elapsed().as_secs_f64());
        let view = self.app.view();
        #[expect(clippy::cast_possible_truncation, reason = "window sizes fit in f32")]
        let frame = build_frame(
            &view,
            &theme,
            &mut self.fonts,
            &mut self.state,
            (lw as f32, lh as f32),
            scale,
        );
        let scene = frame.paint(&mut self.fonts, &mut self.state);
        self.shell.present(&scene);
        // Content may have moved under a stationary pointer (scroll,
        // layout change): refresh hover and repaint once more if it did.
        if refresh_hover(&view, &frame, &mut self.state)
            && let Some(w) = self.shell.window()
        {
            w.request_redraw();
        }
        if frame.animating {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(16),
            ));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
        self.last = Some((view, frame));
    }

    fn input(&mut self, event: InputEvent) {
        let Some((view, frame)) = &self.last else {
            return;
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
        if (result.redraw || had_msgs)
            && let Some(w) = self.shell.window()
        {
            w.request_redraw();
        }
    }
}

fn map_cursor(cursor: fenestra_core::Cursor) -> winit::window::CursorIcon {
    match cursor {
        fenestra_core::Cursor::Default => winit::window::CursorIcon::Default,
        fenestra_core::Cursor::Pointer => winit::window::CursorIcon::Pointer,
        fenestra_core::Cursor::Text => winit::window::CursorIcon::Text,
        fenestra_core::Cursor::NotAllowed => winit::window::CursorIcon::NotAllowed,
    }
}

/// Translates a winit key event into a fenestra [`InputEvent`].
fn map_key(
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

impl<A: App> ApplicationHandler<ProxyEvent> for AppRunner<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.shell.resumed(event_loop);
        if let Some(w) = self.shell.window() {
            w.set_ime_allowed(true);
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: ProxyEvent) {
        if let Ok(msg) = event.0.downcast::<A::Msg>() {
            self.app.update(*msg);
            if let Some(w) = self.shell.window() {
                w.request_redraw();
            }
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
            WindowEvent::ModifiersChanged(mods) => self.modifiers = mods.state(),
            WindowEvent::Occluded(occluded) => {
                if !occluded && let Some(w) = self.shell.window() {
                    w.request_redraw();
                }
            }
            WindowEvent::CursorLeft { .. } => self.input(InputEvent::PointerLeave),
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.shell.window().map_or(1.0, |w| w.scale_factor());
                self.cursor = Point::new(position.x / scale, position.y / scale);
                #[expect(clippy::cast_possible_truncation, reason = "positions fit in f32")]
                self.input(InputEvent::PointerMove {
                    x: self.cursor.x as f32,
                    y: self.cursor.y as f32,
                });
            }
            WindowEvent::MouseInput {
                state,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                self.input(match state {
                    winit::event::ElementState::Pressed => InputEvent::PointerDown,
                    winit::event::ElementState::Released => InputEvent::PointerUp,
                });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(y) * LINE_SCROLL_PX,
                    MouseScrollDelta::PixelDelta(pos) => {
                        let scale = self.shell.window().map_or(1.0, |w| w.scale_factor());
                        pos.y / scale
                    }
                };
                #[expect(clippy::cast_possible_truncation, reason = "deltas fit in f32")]
                self.input(InputEvent::Wheel { dy: dy as f32 });
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
                            self.input(InputEvent::Text(t.to_string()));
                        }
                    } else if let Some(input) = map_key(&event, mods) {
                        self.input(input);
                    }
                }
            }
            WindowEvent::Ime(ime) => match ime {
                winit::event::Ime::Preedit(text, cursor) => {
                    self.input(InputEvent::ImePreedit { text, cursor });
                }
                winit::event::Ime::Commit(text) => {
                    self.input(InputEvent::Text(text));
                }
                winit::event::Ime::Enabled | winit::event::Ime::Disabled => {}
            },
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }
}
