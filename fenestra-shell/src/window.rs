//! The windowed runner: winit event loop + wgpu surface + vello renderer.
//!
//! M0 ships a scene-callback runner; the full `App` runner with input and
//! state arrives in M4 and will reuse this surface plumbing.

use std::sync::Arc;

use vello::peniko::Color;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu::{self, CurrentSurfaceTexture};
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::ShellError;

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

/// A frame paint callback: builds the scene in logical coordinates for the
/// given logical size. The runner applies the DPI scale transform.
pub type PaintFn = dyn FnMut(&mut Scene, f64, f64, Color);

/// Opens a window and repaints via `paint(scene, logical_w, logical_h, bg)`
/// on every redraw. Blocks until the window closes.
pub fn run_scene(
    options: WindowOptions,
    background: Color,
    paint: impl FnMut(&mut Scene, f64, f64, Color) + 'static,
) -> Result<(), ShellError> {
    let event_loop = EventLoop::new().map_err(ShellError::EventLoop)?;
    let mut app = SceneApp {
        context: RenderContext::new(),
        renderers: Vec::new(),
        state: RenderState::Suspended(None),
        scene: Scene::new(),
        fragment: Scene::new(),
        options,
        background,
        paint: Box::new(paint),
    };
    event_loop.run_app(&mut app).map_err(ShellError::EventLoop)
}

enum RenderState {
    Active {
        surface: Box<RenderSurface<'static>>,
        valid_surface: bool,
        window: Arc<Window>,
    },
    Suspended(Option<Arc<Window>>),
}

struct SceneApp {
    context: RenderContext,
    renderers: Vec<Option<Renderer>>,
    state: RenderState,
    scene: Scene,
    fragment: Scene,
    options: WindowOptions,
    background: Color,
    paint: Box<PaintFn>,
}

impl ApplicationHandler for SceneApp {
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

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let RenderState::Active { window, .. } = &self.state {
            self.state = RenderState::Suspended(Some(window.clone()));
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let RenderState::Active {
            surface,
            valid_surface,
            window,
        } = &mut self.state
        else {
            return;
        };
        if window.id() != window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if size.width != 0 && size.height != 0 {
                    self.context
                        .resize_surface(surface, size.width, size.height);
                    *valid_surface = true;
                } else {
                    *valid_surface = false;
                }
                window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                if !*valid_surface {
                    return;
                }
                let width = surface.config.width;
                let height = surface.config.height;
                let scale = window.scale_factor();

                // Build the frame in logical coordinates, then apply DPI scale.
                self.fragment.reset();
                (self.paint)(
                    &mut self.fragment,
                    f64::from(width) / scale,
                    f64::from(height) / scale,
                    self.background,
                );
                self.scene.reset();
                self.scene
                    .append(&self.fragment, Some(vello::kurbo::Affine::scale(scale)));

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
                    CurrentSurfaceTexture::Occluded | CurrentSurfaceTexture::Timeout => {
                        window.request_redraw();
                        return;
                    }
                    CurrentSurfaceTexture::Lost => panic!("wgpu surface was lost"),
                    CurrentSurfaceTexture::Validation => {
                        panic!("validation error acquiring wgpu surface texture")
                    }
                };

                let mut encoder =
                    handle
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
            _ => {}
        }
    }
}
