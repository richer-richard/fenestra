//! Embedded mode: YOUR winit event loop, YOUR wgpu device and surface,
//! YOUR render pass — fenestra composites a HUD over it with alpha.
//! The host "scene" here is an animated clear color; swap in a game
//! renderer and nothing about the fenestra side changes.

use std::sync::Arc;
use std::time::Instant;

use fenestra::prelude::*;
use fenestra::shell::vello::util::RenderContext;
use fenestra::shell::{Embedded, wgpu, winit};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

#[derive(Default)]
struct Hud {
    clicks: u32,
    paused: bool,
}

#[derive(Clone)]
enum Msg {
    Bump,
    TogglePause,
}

impl App for Hud {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Bump => self.clicks += 1,
            Msg::TogglePause => self.paused = !self.paused,
        }
    }

    fn view(&self) -> Element<Msg> {
        col().p(SP6).items_start().children([col()
            .p(SP4)
            .gap(SP3)
            .w(260.0)
            .rounded(R_LG)
            .shadow(ShadowToken::Lg)
            .themed(|t: &Theme, s| s.bg(t.elevated_surface(2)).border(1.0, t.border_subtle))
            .children((
                text("Embedded HUD").weight(Weight::Semibold),
                text("The teal pulse behind this panel is the host app's own render pass.")
                    .size(TextSize::Sm)
                    .themed(|t: &Theme, s| s.color(t.text_muted)),
                text(format!("clicks: {}", self.clicks)).size(TextSize::Sm),
                row().gap(SP2).children([
                    button("Bump").on_click(Msg::Bump),
                    button(if self.paused { "Resume" } else { "Pause" })
                        .variant(ButtonVariant::Secondary)
                        .on_click(Msg::TogglePause),
                ]),
            ))])
    }
}

struct Host {
    context: RenderContext,
    window: Option<HostWindow>,
    started: Instant,
}

struct HostWindow {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    ui: Embedded<Hud>,
}

impl ApplicationHandler for Host {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("fenestra embedded in a wgpu app")
                        .with_inner_size(winit::dpi::LogicalSize::new(720.0, 480.0)),
                )
                .expect("window"),
        );
        let surface = self
            .context
            .instance
            .create_surface(window.clone())
            .expect("surface");
        let dev_id =
            pollster::block_on(self.context.device(Some(&surface))).expect("compatible adapter");
        let handle = &self.context.devices[dev_id];
        let (device, queue) = (handle.device.clone(), handle.queue.clone());
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(handle.adapter(), size.width.max(1), size.height.max(1))
            .expect("surface config");
        config.usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
        surface.configure(&device, &config);

        let mut ui = Embedded::new(Hud::default(), Theme::dark(), &device, config.format);
        ui.set_clear(Color::TRANSPARENT); // the host scene shows through
        window.request_redraw();
        self.window = Some(HostWindow {
            window,
            surface,
            config,
            device,
            queue,
            ui,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(host) = &mut self.window else {
            return;
        };
        // fenestra first; it says whether it consumed the event.
        let response = host.ui.handle_window_event(&host.window, &event);
        if response.repaint {
            host.window.request_redraw();
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                host.config.width = size.width.max(1);
                host.config.height = size.height.max(1);
                host.surface.configure(&host.device, &host.config);
            }
            WindowEvent::RedrawRequested => {
                use wgpu::CurrentSurfaceTexture;
                let frame = match host.surface.get_current_texture() {
                    CurrentSurfaceTexture::Success(frame)
                    | CurrentSurfaceTexture::Suboptimal(frame) => frame,
                    _ => {
                        host.surface.configure(&host.device, &host.config);
                        host.window.request_redraw();
                        return;
                    }
                };
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                // ---- the host app's own pass: an animated clear.
                let t = self.started.elapsed().as_secs_f64();
                let pulse = if host.ui.app().paused {
                    0.25
                } else {
                    0.25 + 0.15 * (t * 1.4).sin()
                };
                let mut encoder =
                    host.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("host scene"),
                        });
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("host clear"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.02,
                                g: pulse,
                                b: pulse * 1.3,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                host.queue.submit([encoder.finish()]);

                // ---- fenestra composites the HUD over it.
                host.ui.render(
                    &host.device,
                    &host.queue,
                    &view,
                    (host.config.width, host.config.height),
                    host.window.scale_factor(),
                );
                frame.present();

                // The host paces frames; fenestra animations and the
                // pulse both want another one.
                if !host.ui.app().paused || host.ui.animating() {
                    host.window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    let mut host = Host {
        context: RenderContext::new(),
        window: None,
        started: Instant::now(),
    };
    event_loop.run_app(&mut host).expect("run");
}
