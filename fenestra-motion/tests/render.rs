//! The determinism contract, as CI tests. Measured on the reference GPU
//! (macOS/Metal, 2026-07-02): vello's compute rasterizer wobbles ±1 LSB on a
//! tiny fraction of antialiased pixels even for the *same scene object*
//! rendered twice in one process — float accumulation order is not
//! associative. So the contract is layered, and these tests pin each layer
//! at its true strength:
//!
//! 1. sampling (`sample`/`resolve`) is EXACTLY deterministic — pure math;
//! 2. the same frame rendered twice agrees within ±1 per channel on < 0.1%
//!    of pixels (usually zero) — in practice indistinguishable, and far
//!    inside the 3/255 + 0.2% golden tolerance used across machines;
//! 3. a parallel PNG-sequence render agrees with standalone single-frame
//!    renders within the same bound — parallelism adds nothing on top of
//!    the GPU's own noise;
//!
//! plus straight-alpha output over a transparent background and the ffmpeg
//! pipe (skipped, loudly, when no ffmpeg binary is on PATH).

use fenestra_core::{Color, div, text};
use fenestra_motion::{Clip, Composition, Frames, Prop, Track, ease_out, key};

fn comp() -> Composition {
    Composition::new(320, 180, 30)
        .duration(Frames(12))
        .background(Color::new([0.1, 0.1, 0.12, 1.0]))
        .clip(
            Clip::new("title", 0..12)
                .element(|| text("Motion"))
                .animate(Prop::Opacity, Track::new([key(0, 0.0f32), key(10, 1.0)]))
                .animate(
                    Prop::TranslateY,
                    Track::new([key(0, 24.0f32).ease(ease_out()), key(10, 0.0)]),
                ),
        )
}

/// The pixel-agreement bound the GPU actually provides in-process: ±1 per
/// channel on under 0.1% of pixels.
fn assert_pixels_close(a: &image::RgbaImage, b: &image::RgbaImage, what: &str) {
    assert_eq!(a.dimensions(), b.dimensions(), "{what}: dimensions");
    let mut differing = 0usize;
    for (pa, pb) in a.pixels().zip(b.pixels()) {
        let max = pa.0.iter().zip(pb.0).map(|(x, y)| x.abs_diff(y)).max();
        if let Some(d) = max
            && d > 0
        {
            assert!(d <= 1, "{what}: channel delta {d} exceeds the ±1 LSB bound");
            differing += 1;
        }
    }
    let total = (a.width() * a.height()) as usize;
    assert!(
        differing * 1000 <= total,
        "{what}: {differing}/{total} pixels differ (> 0.1%)"
    );
}

#[test]
fn sampling_is_exactly_deterministic() {
    let comp = comp();
    for f in [0u64, 3, 5, 10, 11] {
        let a = comp.sample(Frames(f)).resolve("title").unwrap();
        let b = comp.sample(Frames(f)).resolve("title").unwrap();
        assert_eq!(a, b, "sample layer must be pure math at frame {f}");
    }
}

#[test]
fn same_frame_twice_agrees_within_the_gpu_bound() {
    let comp = comp();
    let a = comp.render_frame(Frames(5)).expect("render");
    let b = comp.render_frame(Frames(5)).expect("render");
    assert_pixels_close(&a, &b, "same frame twice");
}

#[test]
fn parallel_sequence_agrees_with_standalone_frames() {
    let comp = comp();
    let dir = std::env::temp_dir().join(format!("fenestra-motion-seq-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    // The parallel path writes the sequence...
    comp.render_png_sequence(0..12, &dir).expect("sequence");

    // ...and every file must decode to the standalone render's pixels.
    for f in 0..12u64 {
        let path = dir.join(format!("frame_{f:05}.png"));
        let disk = image::open(&path).expect("decode frame").to_rgba8();
        let standalone = comp.render_frame(Frames(f)).expect("render");
        assert_pixels_close(&disk, &standalone, &format!("parallel frame {f}"));
    }
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn transparent_background_writes_straight_alpha() {
    // A half-opaque white box over a transparent canvas: straight alpha
    // means white channels with ~50% alpha, NOT premultiplied gray.
    let comp = Composition::new(64, 64, 30).duration(Frames(1)).clip(
        Clip::new("veil", 0..1)
            .element(|| div().w(64.0).h(64.0).bg(Color::new([1.0, 1.0, 1.0, 1.0])))
            .animate(Prop::Opacity, Track::new([key(0, 0.5f32)])),
    );
    let img = comp.render_frame(Frames(0)).expect("render");
    let px = img.get_pixel(32, 32).0;
    assert!(
        px[3] >= 120 && px[3] <= 135,
        "half-opaque alpha, got {px:?}"
    );
    assert!(
        px[0] >= 250 && px[1] >= 250 && px[2] >= 250,
        "straight (un-premultiplied) white channels, got {px:?}"
    );
}

#[test]
fn a_single_thread_pool_still_renders_a_sequence() {
    // Regression: the ordered-writer pipeline must not require a second
    // rayon thread (the writer used to be a join arm that a 1-thread pool
    // never reached — producers filled the bounded channel and hung).
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .expect("pool");
    let comp = comp();
    let dir = std::env::temp_dir().join(format!("fenestra-motion-1t-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    pool.install(|| comp.render_png_sequence(0..6, &dir))
        .expect("sequence on one thread");
    for f in 0..6u64 {
        assert!(dir.join(format!("frame_{f:05}.png")).exists());
    }
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn a_write_error_mid_sequence_propagates() {
    let comp = comp();
    let dir = std::env::temp_dir().join(format!("fenestra-motion-werr-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // Squat a directory on the first frame's path: fs::write must fail and
    // the pipeline must surface it instead of reporting success.
    std::fs::create_dir_all(dir.join("frame_00000.png")).expect("squat");
    let err = comp
        .render_png_sequence(0..6, &dir)
        .expect_err("the writer's failure is the caller's failure");
    assert!(
        matches!(err, fenestra_motion::MotionError::Io(_)),
        "an io error, loudly: {err}"
    );
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn a_failing_ffmpeg_reports_its_stderr_not_a_broken_pipe() {
    if std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_err()
    {
        eprintln!("SKIP: no ffmpeg on PATH");
        return;
    }
    let comp = comp();
    // An output directory that doesn't exist: ffmpeg exits nonzero after
    // failing to open it, closing our pipe mid-stream. The error must carry
    // ffmpeg's own diagnostic, not the writer's broken pipe.
    let out = std::path::Path::new("/nonexistent-fenestra-motion-dir/out.mp4");
    let err = comp
        .render_video(0..12, out)
        .expect_err("unwritable output fails");
    let msg = err.to_string();
    assert!(
        msg.contains("ffmpeg failed"),
        "ffmpeg's stderr tail survives the broken pipe: {msg}"
    );
}

#[test]
#[cfg(unix)]
fn a_chatty_encoders_stderr_does_not_deadlock_the_frame_writer() {
    // A test-double "ffmpeg" that bursts well past the OS pipe buffer
    // (~64KB) to stderr BEFORE it starts reading stdin at all: if stderr
    // isn't drained concurrently with the frame-writing loop, the encoder
    // blocks on its own stderr write, never reaches stdin, and our writer
    // blocks writing frames — a real deadlock, not a hypothetical one.
    let script =
        std::env::temp_dir().join(format!("motion-chatty-ffmpeg-{}.sh", std::process::id()));
    std::fs::write(
        &script,
        "#!/bin/sh\nyes 'warning: something' | head -c 200000 >&2\ncat >/dev/null\nexit 0\n",
    )
    .expect("write test double");
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script).expect("stat").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).expect("chmod");
    }

    let out = std::env::temp_dir().join(format!("motion-chatty-out-{}.mp4", std::process::id()));
    let (tx, rx) = std::sync::mpsc::channel();
    let out_for_thread = out.clone();
    let script_for_thread = script.clone();
    // A hung render leaks this thread; the test still fails promptly via
    // the timeout below instead of hanging the whole suite.
    std::thread::spawn(move || {
        let result = comp().render_video_with(0..12, &out_for_thread, &script_for_thread);
        let _ = tx.send(result);
    });
    let result = rx
        .recv_timeout(std::time::Duration::from_secs(20))
        .expect("render_video_with must not deadlock on a chatty encoder's stderr");
    result.expect("a chatty-but-well-behaved encoder should still succeed");

    let _ = std::fs::remove_file(&out);
    let _ = std::fs::remove_file(&script);
}

#[test]
fn semi_transparent_composition_background_is_not_double_composited() {
    // An empty canvas (no clips) with a half-opaque background: the base
    // clear color IS the background — nothing should paint over it a
    // second time, which would darken/increase its opacity.
    let comp = Composition::new(64, 64, 30)
        .duration(Frames(1))
        .background(Color::new([0.2, 0.4, 0.6, 0.5]));
    let img = comp.render_frame(Frames(0)).expect("render");
    let px = img.get_pixel(32, 32).0;
    // Straight alpha ~127 (0.5 * 255); double-compositing over itself would
    // push this toward ~191 (0.5 + 0.5*0.5) and darken the RGB channels.
    assert!(
        px[3] >= 120 && px[3] <= 135,
        "background alpha applied exactly once, got {px:?}"
    );
}

#[test]
fn missing_ffmpeg_binary_fails_with_its_name() {
    let comp = comp();
    let out = std::env::temp_dir().join("fenestra-motion-missing.mp4");
    let err = comp
        .render_video_with(
            0..2,
            &out,
            std::path::Path::new("ffmpeg-definitely-not-here"),
        )
        .expect_err("a missing encoder must fail loudly");
    let msg = err.to_string();
    assert!(
        msg.contains("ffmpeg-definitely-not-here"),
        "the error names the binary: {msg}"
    );
}

#[test]
fn ffmpeg_pipe_encodes_a_video() {
    if std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_err()
    {
        eprintln!("SKIP: no ffmpeg on PATH; the pipe test needs one");
        return;
    }
    let comp = comp();
    let out = std::env::temp_dir().join(format!("fenestra-motion-{}.mp4", std::process::id()));
    let _ = std::fs::remove_file(&out);
    comp.render_video(0..12, &out).expect("encode");
    let meta = std::fs::metadata(&out).expect("output exists");
    assert!(
        meta.len() > 500,
        "a real video came out: {} bytes",
        meta.len()
    );
    std::fs::remove_file(&out).expect("cleanup");
}
