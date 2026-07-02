//! Rasterization and sinks. Motion never owns pixels: every frame goes
//! through fenestra's headless pipeline (`render_element_over`), and the
//! sinks fan frames out over rayon into a bounded, order-restoring writer.
//!
//! Determinism: layout and scene building run per worker thread (own
//! embedded fonts, fresh per-frame state) and are exactly deterministic;
//! the GPU serializes behind the process-wide device mutex. Rasterized
//! pixels carry the hardware's own noise floor — vello's compute rasterizer
//! wobbles ±1 LSB on a tiny fraction of antialiased pixels even for the
//! same scene rendered twice (measured on the Metal reference, 2026-07-02)
//! — so the CI determinism tests pin renders to that bound, and
//! cross-machine reproduction goes through the tolerance-based golden
//! harness like every fenestra visual test.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;
use std::sync::mpsc;

use fenestra_core::{Fonts, FrameState};
use fenestra_shell::{ShellError, render_element_over};
use image::RgbaImage;
use rayon::prelude::*;

use crate::composition::Composition;
use crate::timeline::{FrameRange, Frames};

/// A render or sink failure. Rendering is deterministic, so these are
/// environmental: no GPU, an IO problem, or a missing/failed encoder.
#[derive(Debug)]
pub enum MotionError {
    /// The headless renderer was unavailable or failed.
    Shell(ShellError),
    /// Filesystem or pipe IO failed.
    Io(std::io::Error),
    /// PNG encoding failed.
    Png(image::ImageError),
    /// The ffmpeg binary was not found (the string names what was looked
    /// for). PNG sinks never need it.
    FfmpegMissing(String),
    /// ffmpeg ran but exited unsuccessfully; carries its stderr tail.
    Ffmpeg(String),
}

impl std::fmt::Display for MotionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shell(e) => write!(f, "headless render failed: {e}"),
            Self::Io(e) => write!(f, "io failed: {e}"),
            Self::Png(e) => write!(f, "png encoding failed: {e}"),
            Self::FfmpegMissing(bin) => write!(
                f,
                "video encoder {bin:?} not found on PATH — install ffmpeg, or use the PNG-sequence sink (which never needs it)"
            ),
            Self::Ffmpeg(tail) => write!(f, "ffmpeg failed:\n{tail}"),
        }
    }
}

impl std::error::Error for MotionError {}

impl From<ShellError> for MotionError {
    fn from(e: ShellError) -> Self {
        Self::Shell(e)
    }
}

impl From<std::io::Error> for MotionError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

thread_local! {
    /// Per-thread embedded fonts: workers lay out and paint concurrently,
    /// serializing only on the shared GPU. Embedded-only, so every thread
    /// shapes text identically.
    static FONTS: RefCell<Fonts> = RefCell::new(Fonts::embedded());
}

impl Composition {
    /// Rasterizes one frame to straight-alpha RGBA8 — random access: any
    /// frame, any order, identical to the same frame in a full run up to
    /// the GPU's ±1 LSB antialiasing noise (see the module docs).
    ///
    /// # Errors
    /// [`MotionError::Shell`] when no GPU adapter exists or the render
    /// fails.
    pub fn render_frame(&self, frame: Frames) -> Result<RgbaImage, MotionError> {
        self.render_frame_at(frame, 1.0)
    }

    /// [`render_frame`](Self::render_frame) at a scale factor: the cheap
    /// preview for agent inspection loops (`0.25` renders 1/16 of the
    /// pixels; layout is identical, frosted glass falls back to its tint).
    ///
    /// # Errors
    /// [`MotionError::Shell`] when no GPU adapter exists or the render
    /// fails.
    pub fn render_frame_at(&self, frame: Frames, scale: f64) -> Result<RgbaImage, MotionError> {
        let el = self.sample(frame).element();
        let mut img = FONTS.with_borrow_mut(|fonts| {
            let mut state = FrameState::new();
            state.reduced_motion = true;
            render_element_over(
                el,
                &self.theme,
                (self.width, self.height),
                scale,
                self.background,
                fonts,
                &mut state,
            )
        })?;
        // vello output is premultiplied; PNG and compositors want straight
        // alpha. Over an opaque background alpha is 255 everywhere and this
        // is a no-op skipped up front.
        if self.background.components[3] < 1.0 {
            unpremultiply(&mut img);
        }
        Ok(img)
    }

    /// Renders `range` as `frame_%05d.png` into `dir` (created if missing),
    /// alpha preserved. Frames render and encode in parallel; a bounded,
    /// order-restoring writer puts them on disk in frame order. Never needs
    /// ffmpeg.
    ///
    /// # Errors
    /// Render or IO failure on any frame (the first error wins).
    pub fn render_png_sequence(
        &self,
        range: impl Into<FrameRange>,
        dir: impl AsRef<Path>,
    ) -> Result<(), MotionError> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir)?;
        self.for_each_frame_ordered(range.into(), encode_png, |frame, bytes| {
            std::fs::write(dir.join(format!("frame_{frame:05}.png")), bytes)?;
            Ok(())
        })
    }

    /// Renders one frame straight to a PNG file — the agent inspection
    /// loop's single-frame sink.
    ///
    /// # Errors
    /// Render, encode, or IO failure.
    pub fn render_frame_png(
        &self,
        frame: Frames,
        path: impl AsRef<Path>,
    ) -> Result<(), MotionError> {
        let img = self.render_frame(frame)?;
        std::fs::write(path, encode_png(&img)?)?;
        Ok(())
    }

    /// Encodes `range` to a video by piping raw RGBA frames into `ffmpeg`
    /// (`-f rawvideo -pix_fmt rgba`) — the fast opaque path. Alpha video is
    /// deliberately not encoded here: render a PNG sequence and use the
    /// ProRes 4444 / VP9 recipes in the README instead.
    ///
    /// # Errors
    /// [`MotionError::FfmpegMissing`] when no `ffmpeg` is on PATH (PNG
    /// sinks never need it); [`MotionError::Ffmpeg`] with the stderr tail
    /// when encoding fails.
    pub fn render_video(
        &self,
        range: impl Into<FrameRange>,
        out: impl AsRef<Path>,
    ) -> Result<(), MotionError> {
        self.render_video_with(range, out, Path::new("ffmpeg"))
    }

    /// [`render_video`](Self::render_video) with an explicit encoder
    /// binary.
    ///
    /// # Errors
    /// As [`render_video`](Self::render_video).
    pub fn render_video_with(
        &self,
        range: impl Into<FrameRange>,
        out: impl AsRef<Path>,
        ffmpeg: &Path,
    ) -> Result<(), MotionError> {
        let size = format!("{}x{}", self.width, self.height);
        let rate = self.fps.to_string();
        let mut child = std::process::Command::new(ffmpeg)
            .args(["-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &size])
            .args(["-r", &rate, "-i", "-", "-pix_fmt", "yuv420p"])
            .arg(out.as_ref())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    MotionError::FfmpegMissing(ffmpeg.display().to_string())
                } else {
                    MotionError::Io(e)
                }
            })?;
        let mut stdin = child.stdin.take().expect("piped stdin");
        let piped = self.for_each_frame_ordered(
            range.into(),
            |img| Ok(img.as_raw().clone()),
            |_, bytes| {
                stdin.write_all(&bytes)?;
                Ok(())
            },
        );
        drop(stdin);
        let output = child.wait_with_output()?;
        piped?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let tail: Vec<&str> = stderr.lines().rev().take(12).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            return Err(MotionError::Ffmpeg(tail.join("\n")));
        }
        Ok(())
    }

    /// The shared sink pipeline: frames render (and `produce` post-process)
    /// on rayon workers; `consume` sees them strictly in frame order via a
    /// bounded reorder buffer, so parallelism reorders nothing and adds no
    /// error beyond the GPU's own ±1 LSB noise.
    fn for_each_frame_ordered(
        &self,
        range: FrameRange,
        produce: impl Fn(&RgbaImage) -> Result<Vec<u8>, MotionError> + Sync + Send,
        mut consume: impl FnMut(u64, Vec<u8>) -> Result<(), MotionError> + Send,
    ) -> Result<(), MotionError> {
        // Bound the in-flight buffer so a slow writer backpressures the
        // workers instead of buffering the whole sequence in memory.
        let (tx, rx) = mpsc::sync_channel::<(u64, Vec<u8>)>(rayon::current_num_threads() * 2);
        let frames: Vec<u64> = (range.start.0..range.end.0).collect();
        let (rendered, written) = rayon::join(
            move || -> Result<(), MotionError> {
                frames.into_par_iter().try_for_each_with(
                    tx,
                    |tx, frame| -> Result<(), MotionError> {
                        let img = self.render_frame(Frames(frame))?;
                        let bytes = produce(&img)?;
                        // A closed channel means the writer bailed; its
                        // error is the one to report.
                        let _ = tx.send((frame, bytes));
                        Ok(())
                    },
                )
            },
            move || -> Result<(), MotionError> {
                let mut next = range.start.0;
                let mut pending: BTreeMap<u64, Vec<u8>> = BTreeMap::new();
                for (frame, bytes) in rx {
                    pending.insert(frame, bytes);
                    while let Some(bytes) = pending.remove(&next) {
                        consume(next, bytes)?;
                        next += 1;
                    }
                }
                Ok(())
            },
        );
        rendered?;
        written
    }
}

/// Converts premultiplied RGBA8 (vello's output) to straight alpha in
/// place: `c = round(c·255 / a)`. Fully transparent pixels stay zeroed;
/// opaque pixels are untouched.
fn unpremultiply(img: &mut RgbaImage) {
    for px in img.pixels_mut() {
        let a = u16::from(px.0[3]);
        if a == 0 || a == 255 {
            continue;
        }
        for c in &mut px.0[..3] {
            let straight = (u16::from(*c) * 255 + a / 2) / a;
            *c = u8::try_from(straight.min(255)).expect("clamped to u8 range");
        }
    }
}

/// Deterministic PNG encoding (fixed encoder, fixed settings): the same
/// pixels give the same file bytes.
fn encode_png(img: &RgbaImage) -> Result<Vec<u8>, MotionError> {
    let mut bytes = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut bytes);
    image::ImageEncoder::write_image(
        encoder,
        img.as_raw(),
        img.width(),
        img.height(),
        image::ExtendedColorType::Rgba8,
    )
    .map_err(MotionError::Png)?;
    Ok(bytes)
}
