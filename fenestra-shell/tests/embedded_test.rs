//! 0.7 embedded mode: fenestra on a caller-owned device — input flows,
//! state updates, pixels composite onto the caller's target with alpha,
//! and the consumption contract holds.

use fenestra_core::{App, Color, Element, Semantics, Theme, by, col, text};
use fenestra_shell::Embedded;
use vello::util::RenderContext;
use vello::wgpu;

#[derive(Default)]
struct Hud {
    clicks: u32,
}

#[derive(Clone)]
struct Bump;

impl App for Hud {
    type Msg = Bump;

    fn update(&mut self, Bump: Bump) {
        self.clicks += 1;
    }

    fn view(&self) -> Element<Bump> {
        // A panel in the top-left corner; the rest of the canvas is
        // empty (transparent when the clear color is transparent).
        col().p(16.0).items_start().children([col()
            .p(12.0)
            .rounded(8.0)
            .themed(|t: &Theme, s| s.bg(t.elevated_surface(2)))
            .focusable(true)
            .on_click(Bump)
            .semantics(Semantics::Button)
            .label("bump")
            .children([text(format!("clicks: {}", self.clicks))])])
    }
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn gpu() -> Gpu {
    let mut context = RenderContext::new();
    let dev_id = pollster::block_on(context.device(None)).expect("adapter");
    let handle = &context.devices[dev_id];
    Gpu {
        device: handle.device.clone(),
        queue: handle.queue.clone(),
    }
}

fn readback(gpu: &Gpu, texture: &wgpu::Texture, w: u32, h: u32) -> Vec<u8> {
    let bytes_per_row = (w * 4).next_multiple_of(256);
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(bytes_per_row) * u64::from(h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    rx.recv().expect("map").expect("map ok");
    let data = slice.get_mapped_range();
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let start = (row * bytes_per_row) as usize;
        out.extend_from_slice(&data[start..start + (w * 4) as usize]);
    }
    out
}

#[test]
fn embedded_renders_routes_input_and_composites() {
    let gpu = gpu();
    let (w, h) = (320u32, 240u32);
    // The caller's own target: rendered by them, composited onto by us.
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("caller target"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

    // The caller clears to opaque red (their "scene").
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("caller clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &target_view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
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
    gpu.queue.submit([encoder.finish()]);

    let mut ui = Embedded::new(
        Hud::default(),
        Theme::light(),
        &gpu.device,
        wgpu::TextureFormat::Rgba8Unorm,
    );
    ui.set_clear(Color::TRANSPARENT);
    ui.render(&gpu.device, &gpu.queue, &target_view, (w, h), 1.0);

    // The caller's scene survives where fenestra didn't paint: the
    // bottom-right corner is still pure red.
    let px = readback(&gpu, &target, w, h);
    let at = |x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        (px[i], px[i + 1], px[i + 2], px[i + 3])
    };
    assert_eq!(at(w - 5, h - 5), (255, 0, 0, 255), "caller scene intact");
    // And the panel region shows the near-white panel, not the red
    // scene (green channel: red bg has none, the panel plenty).
    assert!(at(40, 30).1 > 100, "panel composited: {:?}", at(40, 30));

    // Input routes through the same dispatch as everywhere else.
    let node = ui.frame().expect("frame built").get(&by::label("bump"));
    let c = node.rect.center();
    #[expect(clippy::cast_possible_truncation, reason = "test coords")]
    let (cx, cy) = (c.x as f32, c.y as f32);
    let response = ui.input(fenestra_core::InputEvent::PointerMove { x: cx, y: cy });
    assert!(response.consumed, "pointer over the panel is consumed");
    ui.input(fenestra_core::InputEvent::PointerDown);
    let response = ui.input(fenestra_core::InputEvent::PointerUp);
    assert!(response.repaint, "the click changed state");
    assert_eq!(ui.app().clicks, 1);

    // Pointer over empty space is not consumed — the caller keeps it.
    let response = ui.input(fenestra_core::InputEvent::PointerMove {
        x: (w - 10) as f32,
        y: (h - 10) as f32,
    });
    assert!(!response.consumed, "empty space passes through");
}
