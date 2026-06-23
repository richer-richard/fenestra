//! Embedded mode: run a fenestra [`App`] inside a wgpu app *you* own —
//! your event loop, your device, your surface, your frame pacing.
//! fenestra renders to an internal texture and composites onto any
//! target view with premultiplied-alpha blending, so your scene shows
//! through wherever the UI doesn't paint (set a transparent clear).
//!
//! ```ignore
//! // setup (once): your device, your surface format
//! let mut ui = Embedded::new(MyApp::default(), Theme::dark(), &device, surface_format);
//! ui.set_clear(Color::TRANSPARENT);
//!
//! // per winit event:
//! let response = ui.handle_window_event(&window, &event);
//! if response.repaint { window.request_redraw(); }
//! if response.consumed { return; } // fenestra took it
//!
//! // per frame, after your own passes:
//! ui.render(&device, &queue, &surface_view, (w, h), window.scale_factor());
//! ```
//!
//! The batteries-included runner remains the easy path; this is the
//! narrow waist for engines and existing apps. Secondary windows
//! ([`App::windows`]) and IME candidate positioning are runner-only.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use fenestra_core::{
    App, Element, Fonts, Frame, FrameState, InputEvent, Proxy, Theme, build_frame, dispatch,
};
use kurbo::Point;
use vello::wgpu::{self, util::TextureBlitter};
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};

use crate::window::{map_cursor, map_key, wheel_deltas};

/// What the embedded UI did with one window event.
#[derive(Debug, Clone, Copy, Default)]
pub struct EventResponse {
    /// The event targeted fenestra content (pointer over a widget,
    /// keystroke while a widget has focus) — skip your own handling.
    pub consumed: bool,
    /// State changed; render again soon.
    pub repaint: bool,
}

/// A fenestra app embedded in a caller-owned wgpu world. See the
/// module docs for the contract.
pub struct Embedded<A: App> {
    app: A,
    theme: Theme,
    fonts: Fonts,
    state: FrameState,
    renderer: Renderer,
    blitter: TextureBlitter,
    /// Internal premultiplied-alpha target, resized lazily.
    target: Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>,
    last: Option<(Element<A::Msg>, Frame)>,
    pending: Arc<Mutex<Vec<A::Msg>>>,
    cursor: Point,
    /// Cursor icon requested by the last dispatch, applied by
    /// [`Self::handle_window_event`].
    cursor_icon: Option<fenestra_core::Cursor>,
    modifiers: winit::keyboard::ModifiersState,
    started: Instant,
    clear: fenestra_core::Color,
}

impl<A: App> Embedded<A>
where
    A::Msg: Send,
{
    /// Builds the renderer on *your* device. `target_format` is the
    /// format of the views you will pass to [`Self::render`] (usually
    /// your surface format).
    ///
    /// # Panics
    /// If vello's shaders fail to compile on the device.
    pub fn new(
        mut app: A,
        theme: Theme,
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let renderer = Renderer::new(
            device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: AaSupport::area_only(),
                num_init_threads: std::num::NonZeroUsize::new(1),
                pipeline_cache: None,
            },
        )
        .expect("vello renderer on caller device");
        let blitter = wgpu::util::TextureBlitterBuilder::new(device, target_format)
            .blend_state(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING)
            .build();
        let pending: Arc<Mutex<Vec<A::Msg>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&pending);
        app.init(Proxy::new(move |msg| {
            sink.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(msg);
        }));
        let mut state = FrameState::new();
        state.set_clipboard(Box::new(crate::OsClipboard::default()));
        let clear = theme.bg;
        Self {
            app,
            theme,
            fonts: Fonts::with_system(),
            state,
            renderer,
            blitter,
            target: None,
            last: None,
            pending,
            cursor: Point::ORIGIN,
            cursor_icon: None,
            modifiers: winit::keyboard::ModifiersState::default(),
            started: Instant::now(),
            clear,
        }
    }

    /// The base color behind the UI. Defaults to the theme background;
    /// set `Color::TRANSPARENT` to composite over your own scene.
    pub fn set_clear(&mut self, color: fenestra_core::Color) {
        self.clear = color;
    }

    /// Replaces the theme (e.g. a light/dark toggle driven by your app).
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    /// The app under the UI.
    pub fn app(&self) -> &A {
        &self.app
    }

    /// Mutable app access (the next [`Self::render`] rebuilds).
    pub fn app_mut(&mut self) -> &mut A {
        &mut self.app
    }

    /// Drains proxied messages (from [`App::init`] / threads) into the
    /// app. Returns whether anything was applied — repaint if so.
    pub fn pump(&mut self) -> bool {
        let msgs =
            std::mem::take(&mut *self.pending.lock().unwrap_or_else(PoisonError::into_inner));
        let any = !msgs.is_empty();
        for msg in msgs {
            self.app.update(msg);
        }
        any
    }

    fn hits(&self, point: Point) -> bool {
        self.last
            .as_ref()
            .is_some_and(|(_, frame)| frame.hit_chain(point).len() > 1)
    }

    /// Routes one raw input event into the UI. Prefer
    /// [`Self::handle_window_event`] in winit apps; this is the
    /// window-system-agnostic form (and what tests drive).
    pub fn input(&mut self, event: InputEvent) -> EventResponse {
        // Consumption heuristic, judged against the *current* frame:
        // pointer events over a widget, keystrokes while focused.
        let consumed = match &event {
            InputEvent::PointerMove { x, y } => self.hits(Point::new(f64::from(*x), f64::from(*y))),
            InputEvent::PointerDown
            | InputEvent::PointerUp
            | InputEvent::RightDown
            | InputEvent::RightUp
            | InputEvent::Wheel { .. } => self.hits(self.cursor),
            InputEvent::Key(_) | InputEvent::Text(_) | InputEvent::ImePreedit { .. } => {
                self.state.focused().is_some()
            }
            _ => false,
        };
        if let InputEvent::PointerMove { x, y } = event {
            self.cursor = Point::new(f64::from(x), f64::from(y));
        }
        let Some((view, frame)) = &self.last else {
            return EventResponse {
                consumed: false,
                repaint: true,
            };
        };
        let result = dispatch(view, frame, &mut self.state, &mut self.fonts, event);
        self.cursor_icon = result.cursor;
        let had_msgs = !result.msgs.is_empty();
        for msg in result.msgs {
            self.app.update(msg);
        }
        EventResponse {
            consumed,
            repaint: result.redraw || had_msgs,
        }
    }

    /// Translates and routes one winit event (cursor, buttons, wheel,
    /// keyboard with the printable/shortcut split, IME commit/preedit,
    /// modifiers) — the same mapping the built-in runner uses.
    pub fn handle_window_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) -> EventResponse {
        use winit::event::{ElementState, MouseButton, WindowEvent};
        let scale = window.scale_factor();
        match event {
            WindowEvent::CursorMoved { position, .. } =>
            {
                #[expect(clippy::cast_possible_truncation, reason = "positions fit in f32")]
                self.input(InputEvent::PointerMove {
                    x: (position.x / scale) as f32,
                    y: (position.y / scale) as f32,
                })
            }
            WindowEvent::CursorLeft { .. } => self.input(InputEvent::PointerLeave),
            WindowEvent::MouseInput { state, button, .. } => {
                let event = match (button, state) {
                    (MouseButton::Left, ElementState::Pressed) => InputEvent::PointerDown,
                    (MouseButton::Left, ElementState::Released) => InputEvent::PointerUp,
                    (MouseButton::Right, ElementState::Pressed) => InputEvent::RightDown,
                    (MouseButton::Right, ElementState::Released) => InputEvent::RightUp,
                    _ => return EventResponse::default(),
                };
                let response = self.input(event);
                if let Some(cursor) = self.cursor_icon.take() {
                    window.set_cursor(winit::window::Cursor::Icon(map_cursor(cursor)));
                }
                response
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = wheel_deltas(*delta, scale);
                #[expect(clippy::cast_possible_truncation, reason = "deltas fit in f32")]
                self.input(InputEvent::Wheel {
                    dx: dx as f32,
                    dy: dy as f32,
                })
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
                let m = self.modifiers;
                self.input(InputEvent::Modifiers {
                    shift: m.shift_key(),
                    ctrl: m.control_key(),
                    alt: m.alt_key(),
                    meta: m.super_key(),
                })
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let mods = self.modifiers;
                let printable = !mods.control_key()
                    && !mods.super_key()
                    && event
                        .text
                        .as_ref()
                        .is_some_and(|t| !t.is_empty() && t.chars().all(|c| !c.is_control()));
                if printable {
                    match &event.text {
                        Some(t) => self.input(InputEvent::Text(t.to_string())),
                        None => EventResponse::default(),
                    }
                } else if let Some(input) = map_key(event, mods) {
                    self.input(input)
                } else {
                    EventResponse::default()
                }
            }
            WindowEvent::Ime(ime) => match ime {
                winit::event::Ime::Preedit(text, cursor) => self.input(InputEvent::ImePreedit {
                    text: text.clone(),
                    cursor: *cursor,
                }),
                winit::event::Ime::Commit(text) => self.input(InputEvent::Text(text.clone())),
                _ => EventResponse::default(),
            },
            _ => EventResponse::default(),
        }
    }

    /// Whether the last built frame is still animating (keep rendering).
    pub fn animating(&self) -> bool {
        self.last.as_ref().is_some_and(|(_, f)| f.animating)
    }

    /// Builds the current frame and composites it onto `target` with
    /// premultiplied-alpha blending. `physical` is the target size in
    /// physical pixels; `scale` the DPI factor (logical = physical /
    /// scale). Call after your own passes each frame.
    ///
    /// # Panics
    /// If vello fails to render (device loss).
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        physical: (u32, u32),
        scale: f64,
    ) {
        self.pump();
        let (pw, ph) = (physical.0.max(1), physical.1.max(1));
        self.state.tick(self.started.elapsed().as_secs_f64());
        #[expect(clippy::cast_possible_truncation, reason = "window sizes fit in f32")]
        let logical = (
            (f64::from(pw) / scale) as f32,
            (f64::from(ph) / scale) as f32,
        );
        let view = self.app.view_at(fenestra_core::MAIN_WINDOW, logical);
        let frame = build_frame(
            &view,
            &self.theme,
            &mut self.fonts,
            &mut self.state,
            logical,
            scale,
        );
        // Single-pass embedded path: glass renders as its translucent tint
        // (no CPU backdrop blur, which needs a read-back). Headless rendering —
        // the golden source of truth — uses the two-pass `render_plan`. See
        // ARCHITECTURE.md ("Real frosted-glass backdrop blur").
        let scene: Scene = frame.paint(&mut self.fonts, &mut self.state);

        if self
            .target
            .as_ref()
            .is_none_or(|(_, _, w, h)| (*w, *h) != (pw, ph))
        {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("fenestra embedded target"),
                size: wgpu::Extent3d {
                    width: pw,
                    height: ph,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.target = Some((texture, view, pw, ph));
        }
        let (_, internal_view, ..) = self.target.as_ref().expect("just ensured");

        self.renderer
            .render_to_texture(
                device,
                queue,
                &scene,
                internal_view,
                &RenderParams {
                    base_color: self.clear,
                    width: pw,
                    height: ph,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .expect("vello render");

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("fenestra embedded blit"),
        });
        self.blitter
            .copy(device, &mut encoder, internal_view, target);
        queue.submit([encoder.finish()]);
        self.last = Some((view, frame));
    }

    /// The last built frame (after [`Self::render`]) — semantic queries
    /// and inspector dumps work on it like anywhere else.
    pub fn frame(&self) -> Option<&Frame> {
        self.last.as_ref().map(|(_, frame)| frame)
    }

    /// The internal premultiplied-alpha texture view from the last
    /// [`Self::render`] — sample it in your own pipeline for custom
    /// compositing instead of the built-in blit.
    pub fn texture_view(&self) -> Option<&wgpu::TextureView> {
        self.target.as_ref().map(|(_, view, ..)| view)
    }
}
