//! Offscreen rendering: a wgpu device plus vello renderer with no window or
//! display server. This is the backbone of fenestra's snapshot testing.

use std::num::NonZeroUsize;

use fenestra_core::{Fonts, Frame, FrameState};
use image::RgbaImage;
use vello::peniko::Color;
use vello::util::RenderContext;
use vello::wgpu::{
    self, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Extent3d, TexelCopyBufferInfo,
    TexelCopyBufferLayout, TextureDescriptor, TextureFormat, TextureUsages,
};
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};

use crate::ShellError;

/// Env var selecting vello's CPU compute pipeline (any value but `0` or
/// empty): the renderer's shader stages run as native Rust instead of GPU
/// compute, leaving the adapter only upload/copy work. This is the switch
/// that makes software adapters viable where their compute rasterization
/// crashes or diverges — Windows' built-in WARP hits
/// `STATUS_ACCESS_VIOLATION` under vello's GPU compute but is fine as a
/// copy engine. Applies to headless rendering and the live window alike;
/// adapter *selection* itself is steered by wgpu's own `WGPU_BACKEND` /
/// `WGPU_ADAPTER_NAME`, which fenestra honors end to end.
pub const CPU_ENV: &str = "FENESTRA_CPU";

/// Reads [`CPU_ENV`] from the process environment.
pub(crate) fn cpu_compute_requested() -> bool {
    cpu_flag(std::env::var_os(CPU_ENV).as_deref())
}

/// `FENESTRA_CPU` semantics: set-and-not-`0` enables (empty disables, so
/// `FENESTRA_CPU= cargo test` behaves like unset).
fn cpu_flag(v: Option<&std::ffi::OsStr>) -> bool {
    v.is_some_and(|v| !v.is_empty() && v != "0")
}

/// A reusable offscreen renderer. Creating one compiles vello's shaders, so
/// tests should create it once and render many scenes through it.
pub struct Headless {
    context: RenderContext,
    dev_id: usize,
    renderer: Renderer,
    max_dim: u32,
}

impl Headless {
    /// Acquires a compute-capable adapter (no surface required) and builds a
    /// vello renderer on it. Set [`CPU_ENV`] to run vello's compute stages
    /// on the CPU (software-adapter environments).
    pub fn new() -> Result<Self, ShellError> {
        let mut context = RenderContext::new();
        let dev_id = pollster::block_on(context.device(None)).ok_or(ShellError::NoDevice)?;
        let device = &context.devices[dev_id].device;
        let max_dim = device.limits().max_texture_dimension_2d;
        let renderer = Renderer::new(
            device,
            RendererOptions {
                use_cpu: cpu_compute_requested(),
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
            max_dim,
        })
    }

    /// The largest texture dimension the device supports; render sizes are
    /// clamped to it.
    pub fn max_dimension(&self) -> u32 {
        self.max_dim
    }

    /// Clamps a requested render size on both axes to the supported
    /// `1..=max_dimension()` range.
    pub fn clamp_size(&self, width: u32, height: u32) -> (u32, u32) {
        (width.clamp(1, self.max_dim), height.clamp(1, self.max_dim))
    }

    /// Renders `scene` at the given pixel size over `base_color` and reads the
    /// result back into an RGBA image. The size is clamped to
    /// `1..=max_dimension()` on both axes, so hostile dimensions cannot
    /// trigger wgpu's fatal validation handler.
    pub fn render(
        &mut self,
        scene: &Scene,
        width: u32,
        height: u32,
        base_color: Color,
    ) -> Result<RgbaImage, ShellError> {
        let (width, height) = self.clamp_size(width, height);
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

    /// Renders `frame` with real frosted-glass backdrop blur and foreground
    /// [`ElementFilter`](fenestra_core::ElementFilter)s via the two-pass
    /// pipeline, falling back to a single pass when the frame has neither.
    ///
    /// The fast path is byte-for-byte identical to [`render`](Self::render): a
    /// frame with no filtered node produces an empty plan, so the backdrop scene
    /// *is* the final image and nothing is read back or reprocessed. Otherwise
    /// the backdrop scene (every glass subtree skipped) is rendered and read
    /// back, each region is blurred/filtered on the CPU
    /// ([`process_specs`](crate::multi_pass::process_specs)), and the final scene
    /// composites those images. `width`/`height` are physical pixels.
    pub fn render_plan(
        &mut self,
        frame: &Frame,
        fonts: &mut Fonts,
        state: &mut FrameState,
        width: u32,
        height: u32,
        base_color: Color,
    ) -> Result<RgbaImage, ShellError> {
        // Paint produces logical-space scenes; a hi-DPI frame (scale != 1)
        // scales them onto the physical texture here. `process_specs`
        // already maps spec rects logical→physical via the same factor, and
        // the injected images are physical-resolution cuts drawn back into
        // logical rects — so the two passes stay consistent at any scale.
        let scale = frame.scale();
        let (backdrop_scene, specs) = frame.paint_backdrop(fonts, state);
        let backdrop_scene = Self::at_scale(backdrop_scene, scale);
        if specs.is_empty() {
            // Fast path: one pass, identical to `render`.
            return self.render(&backdrop_scene, width, height, base_color);
        }
        let backdrop = self.render(&backdrop_scene, width, height, base_color)?;
        let injected = crate::multi_pass::process_specs(&backdrop, &specs, scale);
        let final_scene = Self::at_scale(frame.paint_final(fonts, state, &injected), scale);
        self.render(&final_scene, width, height, base_color)
    }

    /// Wraps a logical-space scene for a physical target. Returns the scene
    /// untouched at scale 1.0 so the reference goldens stay byte-identical.
    fn at_scale(scene: Scene, scale: f64) -> Scene {
        if (scale - 1.0).abs() < f64::EPSILON {
            return scene;
        }
        let mut scaled = Scene::new();
        scaled.append(&scene, Some(vello::kurbo::Affine::scale(scale)));
        scaled
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::cpu_flag;

    #[test]
    fn cpu_flag_semantics() {
        assert!(!cpu_flag(None));
        assert!(!cpu_flag(Some(OsStr::new(""))));
        assert!(!cpu_flag(Some(OsStr::new("0"))));
        assert!(cpu_flag(Some(OsStr::new("1"))));
        assert!(cpu_flag(Some(OsStr::new("true"))));
    }
}
