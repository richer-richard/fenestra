//! Offscreen rendering: a wgpu device plus vello renderer with no window or
//! display server. This is the backbone of fenestra's snapshot testing.

use std::num::NonZeroUsize;

use image::RgbaImage;
use vello::peniko::Color;
use vello::util::RenderContext;
use vello::wgpu::{
    self, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Extent3d, TexelCopyBufferInfo,
    TexelCopyBufferLayout, TextureDescriptor, TextureFormat, TextureUsages,
};
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};

use crate::ShellError;

/// A reusable offscreen renderer. Creating one compiles vello's shaders, so
/// tests should create it once and render many scenes through it.
pub struct Headless {
    context: RenderContext,
    dev_id: usize,
    renderer: Renderer,
}

impl Headless {
    /// Acquires a compute-capable adapter (no surface required) and builds a
    /// vello renderer on it.
    pub fn new() -> Result<Self, ShellError> {
        let mut context = RenderContext::new();
        let dev_id = pollster::block_on(context.device(None)).ok_or(ShellError::NoDevice)?;
        let device = &context.devices[dev_id].device;
        let renderer = Renderer::new(
            device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: AaSupport::area_only(),
                num_init_threads: NonZeroUsize::new(1),
                pipeline_cache: None,
            },
        )
        .map_err(ShellError::Vello)?;
        Ok(Self {
            context,
            dev_id,
            renderer,
        })
    }

    /// Renders `scene` at the given pixel size over `base_color` and reads the
    /// result back into an RGBA image.
    pub fn render(
        &mut self,
        scene: &Scene,
        width: u32,
        height: u32,
        base_color: Color,
    ) -> Result<RgbaImage, ShellError> {
        assert!(width > 0 && height > 0, "render size must be non-zero");
        let handle = &self.context.devices[self.dev_id];
        let (device, queue) = (&handle.device, &handle.queue);

        let size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let target = device.create_texture(&TextureDescriptor {
            label: Some("fenestra headless target"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        self.renderer
            .render_to_texture(
                device,
                queue,
                scene,
                &view,
                &RenderParams {
                    base_color,
                    width,
                    height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .map_err(ShellError::Vello)?;

        // wgpu requires copy rows padded to 256 bytes.
        let padded_byte_width = (width * 4).next_multiple_of(256);
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("fenestra headless readback"),
            size: u64::from(padded_byte_width) * u64::from(height),
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("fenestra headless copy"),
        });
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            TexelCopyBufferInfo {
                buffer: &buffer,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_byte_width),
                    rows_per_image: None,
                },
            },
            size,
        );
        queue.submit([encoder.finish()]);

        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|_| ShellError::Readback)?;
        rx.recv()
            .map_err(|_| ShellError::Readback)?
            .map_err(|_| ShellError::Readback)?;

        let data = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height {
            let start = (row * padded_byte_width) as usize;
            pixels.extend_from_slice(&data[start..start + (width * 4) as usize]);
        }
        drop(data);
        buffer.unmap();

        Ok(RgbaImage::from_raw(width, height, pixels)
            .expect("readback buffer matches image dimensions"))
    }
}
