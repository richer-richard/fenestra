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
use std::io::{Read as _, Write as _};
use std::path::Path;
use std::sync::mpsc;

use fenestra_anim::{FrameRange, Frames};
use fenestra_core::{Color, Fonts, FrameState};
use fenestra_shell::{ShellError, render_element_over};
use image::RgbaImage;
use rayon::prelude::*;

use crate::composition::Composition;

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
    /// A contact sheet could not be built (e.g. it would exceed the GPU
    /// texture ceiling); the message names the fix.
    Sheet(String),
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
            Self::Sheet(msg) => write!(f, "contact sheet: {msg}"),
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

/// Kills and reaps the wrapped ffmpeg child on drop if it's still running.
/// `std::process::Child` is not automatically waited on drop, so a panic
/// unwinding out of `render_video_with` (e.g. a rayon worker panic mid
/// pipe) before the normal `wait()` would otherwise orphan the process.
/// A no-op in the ordinary path: by the time this drops there, the child
/// has already exited and `try_wait` reports so.
struct KillOnDrop(std::process::Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
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
        // `element()` already paints `self.background` as a root fill when
        // it has any alpha (SampledScene::element) — that rect IS the
        // background, so the base clear color stays transparent. Passing
        // `self.background` again here would composite it a second time,
        // darkening/increasing the opacity of any non-opaque background.
        let mut img = FONTS.with_borrow_mut(|fonts| {
            let mut state = FrameState::new();
            state.reduced_motion = true;
            render_element_over(
                el,
                &self.theme,
                (self.width, self.height),
                scale,
                Color::TRANSPARENT,
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
        self.for_each_frame_ordered(
            range.into(),
            |img| encode_png(&img),
            |frame, bytes| {
                std::fs::write(dir.join(format!("frame_{frame:05}.png")), bytes)?;
                Ok(())
            },
        )
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
        // -nostats -loglevel error keeps stderr near-silent during a healthy
        // encode, but that alone doesn't ELIMINATE the hazard (a codec that
        // keeps emitting error-level lines without exiting could still fill
        // the OS pipe buffer): stderr is drained on its own thread for the
        // whole piping duration below, so ffmpeg can never block writing it.
        let mut child = KillOnDrop(
            std::process::Command::new(ffmpeg)
                .args(["-y", "-nostats", "-loglevel", "error"])
                .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-s", &size])
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
                })?,
        );
        let mut stdin = child.0.stdin.take().expect("piped stdin");
        let mut stderr = child.0.stderr.take().expect("piped stderr");
        let stderr_reader = std::thread::spawn(move || {
            let mut buf = String::new();
            let _ = stderr.read_to_string(&mut buf);
            buf
        });
        let piped = self.for_each_frame_ordered(
            range.into(),
            |img| Ok(img.into_raw()),
            |_, bytes| {
                stdin.write_all(&bytes)?;
                Ok(())
            },
        );
        drop(stdin);
        let status = child.0.wait()?;
        let stderr = stderr_reader
            .join()
            .expect("the stderr reader thread does not panic");
        // An ffmpeg failure closes our pipe mid-stream, so `piped` holds a
        // broken-pipe error that would mask the real diagnostic: report the
        // encoder's own stderr first, and never leave a truncated file
        // behind on any error path.
        if !status.success() {
            let _ = std::fs::remove_file(out.as_ref());
            let tail: Vec<&str> = stderr.lines().rev().take(12).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            return Err(MotionError::Ffmpeg(tail.join("\n")));
        }
        if let Err(e) = piped {
            let _ = std::fs::remove_file(out.as_ref());
            return Err(e);
        }
        Ok(())
    }

    /// The shared sink pipeline: frames render (and `produce` post-process)
    /// on rayon workers while a dedicated OS thread `consume`s them strictly
    /// in frame order, so parallelism reorders nothing and adds no error
    /// beyond the GPU's own ±1 LSB noise. The channel bounds in-flight
    /// sends (a slow writer backpressures the workers); the reorder map
    /// briefly buffers out-of-order completions. The writer must NOT be a
    /// rayon task: on a single-thread pool a join arm never gets stolen and
    /// the producers would fill the channel and hang.
    fn for_each_frame_ordered(
        &self,
        range: FrameRange,
        produce: impl Fn(RgbaImage) -> Result<Vec<u8>, MotionError> + Sync + Send,
        mut consume: impl FnMut(u64, Vec<u8>) -> Result<(), MotionError> + Send,
    ) -> Result<(), MotionError> {
        let (tx, rx) = mpsc::sync_channel::<(u64, Vec<u8>)>(rayon::current_num_threads() * 2);
        let frames: Vec<u64> = (range.start.0..range.end.0).collect();
        // Set by the writer on failure so producers stop burning GPU time on
        // frames nobody will consume.
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        std::thread::scope(|s| {
            let writer = s.spawn(|| -> Result<(), MotionError> {
                let mut next = range.start.0;
                let mut pending: BTreeMap<u64, Vec<u8>> = BTreeMap::new();
                for (frame, bytes) in rx {
                    pending.insert(frame, bytes);
                    while let Some(bytes) = pending.remove(&next) {
                        if let Err(e) = consume(next, bytes) {
                            cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
                            return Err(e);
                        }
                        next += 1;
                    }
                }
                Ok(())
            });
            let rendered = frames.into_par_iter().try_for_each_with(
                tx,
                |tx, frame| -> Result<(), MotionError> {
                    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                        return Ok(());
                    }
                    let img = self.render_frame(Frames(frame))?;
                    let bytes = produce(img)?;
                    // A closed channel means the writer bailed; its error is
                    // the one to report.
                    let _ = tx.send((frame, bytes));
                    Ok(())
                },
            );
            let written = writer.join().expect("the writer thread does not panic");
            rendered?;
            written
        })
    }
}

/// Renders a verification artifact (contact sheet) over the theme's opaque
/// background at scale 1.0 — same fonts and pipeline as frame renders.
pub(crate) fn render_sheet(
    el: fenestra_core::Element<()>,
    theme: &fenestra_core::Theme,
    size: (u32, u32),
) -> Result<RgbaImage, MotionError> {
    FONTS
        .with_borrow_mut(|fonts| {
            let mut state = FrameState::new();
            state.reduced_motion = true;
            render_element_over(el, theme, size, 1.0, theme.bg, fonts, &mut state)
        })
        .map_err(Into::into)
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
