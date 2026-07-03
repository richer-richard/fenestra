//! The `motion` CLI: render (single frame / sequence / video) and probe
//! (resolved props + bboxes as JSON on stdout). House conventions: JSON to
//! stdout, artifacts to `--out`, notes to stderr, exit 0 ok / 1 verification
//! failed / 3 parse-IO.

use std::path::PathBuf;
use std::process::Command;

const DOC: &str = r#"(
    version: 1,
    width: 320,
    height: 180,
    fps: 30,
    duration: 60,
    background: "bg",
    clips: [
        (
            id: "title",
            start: 0,
            end: 60,
            element: text(content: "Hello", style: (size_px: 32.0, color: "text")),
            tracks: [
                (prop: opacity, keys: [
                    (at: 0, value: scalar(0.0), ease: ease_out),
                    (at: 20, value: scalar(1.0)),
                ]),
            ],
        ),
    ],
)"#;

// A second track: opacity rises 0..20 then dips back down at 40, breaking
// an `increasing` monotone assertion over the clip's whole span without
// touching the discontinuity lint (each step stays inside default eps).
const MONOTONE_VIOLATION_DOC: &str = r#"(
    version: 1,
    width: 320,
    height: 180,
    fps: 30,
    duration: 60,
    background: "bg",
    clips: [
        (
            id: "title",
            start: 0,
            end: 60,
            element: text(content: "Hello", style: (size_px: 32.0, color: "text")),
            tracks: [
                (prop: opacity, keys: [
                    (at: 0, value: scalar(0.0), ease: ease_out),
                    (at: 20, value: scalar(1.0)),
                    (at: 40, value: scalar(0.2)),
                ]),
            ],
        ),
    ],
)"#;

fn write_temp(label: &str, ext: &str, content: &str) -> PathBuf {
    // Every test in this binary writes a fixture file; cargo runs tests on
    // parallel threads, so a pid-only path is shared and one test's write
    // can truncate the file while another's spawned `motion` subprocess is
    // still reading it. A per-call counter gives each caller its own file.
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "motion-cli-{label}-{}-{n}.{ext}",
        std::process::id()
    ));
    std::fs::write(&path, content).expect("write fixture");
    path
}

fn write_doc() -> PathBuf {
    write_temp("doc", "ron", DOC)
}

/// The shipped `examples/lower_third.ron` — same path-resolution pattern as
/// `tests/data.rs`'s `shipped_lower_third_example_compiles_and_round_trips`.
fn lower_third_example_path() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/lower_third.ron"
    ))
}

fn motion() -> Command {
    Command::new(env!("CARGO_BIN_EXE_motion"))
}

#[test]
fn probe_prints_resolved_props_as_json() {
    let doc = write_doc();
    let out = motion()
        .args(["probe", doc.to_str().unwrap(), "--frame", "10"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json stdout");
    assert_eq!(v["frame"], 10);
    assert_eq!(v["paint_order"][0], "title");
    let clip = &v["clips"][0];
    assert_eq!(clip["id"], "title");
    assert_eq!(clip["visible"], true);
    let opacity = clip["props"]["opacity"].as_f64().unwrap();
    assert!(
        opacity > 0.0 && opacity < 1.0,
        "mid-ease opacity: {opacity}"
    );
    assert!(clip["bbox"]["w"].as_f64().unwrap() > 10.0);
}

#[test]
fn probe_can_filter_to_one_clip() {
    let doc = write_doc();
    let out = motion()
        .args([
            "probe",
            doc.to_str().unwrap(),
            "--frame",
            "10",
            "--clip",
            "title",
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["clips"].as_array().unwrap().len(), 1);
}

#[test]
fn probe_unknown_clip_exits_3() {
    let doc = write_doc();
    let out = motion()
        .args([
            "probe",
            doc.to_str().unwrap(),
            "--frame",
            "10",
            "--clip",
            "ghost",
        ])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("ghost"), "names the missing clip: {stderr}");
}

#[test]
fn render_single_frame_writes_a_png() {
    let doc = write_doc();
    let png = std::env::temp_dir().join(format!("motion-cli-{}.png", std::process::id()));
    let _ = std::fs::remove_file(&png);
    let out = motion()
        .args([
            "render",
            doc.to_str().unwrap(),
            "--frame",
            "10",
            "--out",
            png.to_str().unwrap(),
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let img = image::open(&png).expect("decodes").to_rgba8();
    assert_eq!(img.dimensions(), (320, 180));
    std::fs::remove_file(&png).expect("cleanup");
}

#[test]
fn render_scaled_frame_shrinks_the_texture() {
    let doc = write_doc();
    let png = std::env::temp_dir().join(format!("motion-cli-scale-{}.png", std::process::id()));
    let _ = std::fs::remove_file(&png);
    let out = motion()
        .args([
            "render",
            doc.to_str().unwrap(),
            "--frame",
            "10",
            "--scale",
            "0.5",
            "--out",
            png.to_str().unwrap(),
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let img = image::open(&png).expect("decodes").to_rgba8();
    assert_eq!(img.dimensions(), (160, 90));
    std::fs::remove_file(&png).expect("cleanup");
}

#[test]
fn render_frame_range_writes_a_sequence() {
    let doc = write_doc();
    let dir = std::env::temp_dir().join(format!("motion-cli-seq-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = motion()
        .args([
            "render",
            doc.to_str().unwrap(),
            "--frames",
            "2..6",
            "--out",
            dir.to_str().unwrap(),
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    for f in 2..6u64 {
        assert!(
            dir.join(format!("frame_{f:05}.png")).exists(),
            "frame {f} written"
        );
    }
    assert!(!dir.join("frame_00006.png").exists(), "range is half-open");
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn render_frames_beyond_the_timeline_clamps_instead_of_hanging() {
    // The shipped doc (write_doc) declares duration: 60. A --frames range
    // reaching toward u64::MAX must clamp to the comp's own duration, not
    // attempt to collect/render billions of frames.
    let doc = write_doc();
    let dir = std::env::temp_dir().join(format!("motion-cli-huge-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = motion()
        .args([
            "render",
            doc.to_str().unwrap(),
            "--frames",
            "0..18446744073709551615",
            "--out",
            dir.to_str().unwrap(),
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        dir.join("frame_00059.png").exists(),
        "renders up to duration"
    );
    assert!(
        !dir.join("frame_00060.png").exists(),
        "clamped at the comp's own duration, not u64::MAX"
    );
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn lint_passes_a_clean_document_and_fails_a_jumpy_one() {
    let doc = write_doc();
    let out = motion()
        .args(["lint", doc.to_str().unwrap()])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(0), "clean doc lints clean");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["problems"].as_array().unwrap().len(), 0);

    // A hold segment snapping mid-span is an undeclared jump: exit 1.
    let jumpy = DOC.replace("ease: ease_out", "ease: hold");
    let path = std::env::temp_dir().join(format!("motion-cli-jump-{}.ron", std::process::id()));
    std::fs::write(&path, jumpy).expect("write");
    let out = motion()
        .args(["lint", path.to_str().unwrap()])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(1), "a jump is a lint failure");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let problems = v["problems"].as_array().unwrap();
    assert!(!problems.is_empty());
    assert_eq!(problems[0]["clip"], "title");
    std::fs::remove_file(&path).expect("cleanup");
}

#[test]
fn sheet_writes_a_labeled_grid_png() {
    let doc = write_doc();
    let png = std::env::temp_dir().join(format!("motion-cli-sheet-{}.png", std::process::id()));
    let _ = std::fs::remove_file(&png);
    let out = motion()
        .args([
            "sheet",
            doc.to_str().unwrap(),
            "--every",
            "20",
            "--out",
            png.to_str().unwrap(),
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let img = image::open(&png).expect("decodes").to_rgba8();
    assert!(img.width() > 100 && img.height() > 50, "a real sheet");
    std::fs::remove_file(&png).expect("cleanup");
}

#[test]
fn a_broken_document_exits_3_with_the_path_pointed_problem() {
    let path = std::env::temp_dir().join(format!("motion-cli-bad-{}.ron", std::process::id()));
    std::fs::write(
        &path,
        DOC.replace("value: scalar(0.0)", "value: pair(1.0, 2.0)"),
    )
    .expect("write");
    let out = motion()
        .args(["probe", path.to_str().unwrap(), "--frame", "0"])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("opacity") && stderr.contains("scalar"),
        "path-pointed: {stderr}"
    );
    std::fs::remove_file(&path).expect("cleanup");
}

// --- `lint --monotone` / `lint --settled-after`: wiring `verify::monotone`
// and `verify::settled` (already tested at the library level in
// `tests/verify.rs`) through to the CLI, alongside the always-on
// `discontinuities` check. ---

#[test]
fn lint_monotone_flag_accepts_a_well_ordered_track() {
    let doc = write_doc();
    let out = motion()
        .args([
            "lint",
            doc.to_str().unwrap(),
            "--monotone",
            "title:opacity:increasing",
        ])
        .output()
        .expect("run");
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["problems"].as_array().unwrap().len(), 0);
}

#[test]
fn lint_monotone_flag_flags_a_direction_violation() {
    let path = write_temp("monotone-bad", "ron", MONOTONE_VIOLATION_DOC);
    let out = motion()
        .args([
            "lint",
            path.to_str().unwrap(),
            "--monotone",
            "title:opacity:increasing",
        ])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(1), "a dip breaks `increasing`");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let problems = v["problems"].as_array().unwrap();
    assert!(!problems.is_empty());
    assert_eq!(problems[0]["clip"], "title");
    assert_eq!(problems[0]["prop"], "Opacity");
}

#[test]
fn lint_monotone_flag_respects_the_frames_range() {
    // The same document is clean if the range never reaches the dip at 40.
    let path = write_temp("monotone-bad", "ron", MONOTONE_VIOLATION_DOC);
    let out = motion()
        .args([
            "lint",
            path.to_str().unwrap(),
            "--monotone",
            "title:opacity:increasing",
            "--frames",
            "0..21",
        ])
        .output()
        .expect("run");
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn lint_monotone_flag_reports_an_unknown_clip_as_a_problem() {
    let doc = write_doc();
    let out = motion()
        .args([
            "lint",
            doc.to_str().unwrap(),
            "--monotone",
            "ghost:opacity:increasing",
        ])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let problems = v["problems"].as_array().unwrap();
    assert!(
        problems.iter().any(|p| p["message"]
            .as_str()
            .unwrap_or_default()
            .contains("no clip")),
        "{problems:?}"
    );
}

#[test]
fn lint_monotone_flag_rejects_a_malformed_spec_with_exit_3() {
    let doc = write_doc();
    let out = motion()
        .args(["lint", doc.to_str().unwrap(), "--monotone", "title-opacity"])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--monotone"), "{stderr}");
}

#[test]
fn lint_frames_flag_requires_monotone() {
    let doc = write_doc();
    let out = motion()
        .args(["lint", doc.to_str().unwrap(), "--frames", "0..10"])
        .output()
        .expect("run");
    assert!(!out.status.success(), "bare --frames is a usage error");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("monotone"), "{stderr}");
}

#[test]
fn lint_settled_after_flag_accepts_a_settled_tail() {
    let doc = write_doc();
    let out = motion()
        .args(["lint", doc.to_str().unwrap(), "--settled-after", "20"])
        .output()
        .expect("run");
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["problems"].as_array().unwrap().len(), 0);
}

#[test]
fn lint_settled_after_flag_flags_still_moving_props() {
    let doc = write_doc();
    let out = motion()
        .args(["lint", doc.to_str().unwrap(), "--settled-after", "5"])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(1), "opacity is still easing at 5");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let problems = v["problems"].as_array().unwrap();
    assert!(!problems.is_empty());
    assert!(
        problems[0]["message"]
            .as_str()
            .unwrap()
            .contains("still moving")
    );
}

#[test]
fn lint_merges_discontinuities_with_monotone_and_settled() {
    // A hold segment snapping mid-span is an undeclared jump (always-on
    // discontinuities check) but is still `increasing` (0 then 1, never
    // decreasing) and settled well after the jump — this proves the three
    // checks' problems are combined into one report rather than one check
    // masking the others.
    let jumpy = DOC.replace("ease: ease_out", "ease: hold");
    let path = write_temp("jumpy-combo", "ron", &jumpy);
    let out = motion()
        .args([
            "lint",
            path.to_str().unwrap(),
            "--monotone",
            "title:opacity:increasing",
            "--settled-after",
            "25",
        ])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(1), "the discontinuity still fails");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let problems = v["problems"].as_array().unwrap();
    assert!(!problems.is_empty());
    assert!(
        problems
            .iter()
            .all(|p| p["message"].as_str().unwrap_or_default().contains("jumps")),
        "only the discontinuity check should have found anything: {problems:?}"
    );
}

// --- Real shipped demos, not just the synthetic DOC fixture. ---

#[test]
fn probe_the_shipped_lower_third_example() {
    let path = lower_third_example_path();
    let out = motion()
        .args(["probe", path.to_str().unwrap(), "--frame", "45"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["frame"], 45);
    assert_eq!(
        v["paint_order"],
        serde_json::json!(["plate", "bar", "title", "subtitle"])
    );
    let title = v["clips"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "title")
        .expect("title clip present");
    assert_eq!(title["visible"], true);
    assert!(title["props"]["opacity"].as_f64().unwrap() > 0.9);
}

#[test]
fn lint_the_shipped_lower_third_example_is_clean() {
    let path = lower_third_example_path();
    let out = motion()
        .args(["lint", path.to_str().unwrap()])
        .output()
        .expect("run");
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["problems"].as_array().unwrap().len(), 0);
}

#[test]
fn lint_monotone_and_settled_against_the_shipped_lower_third_example() {
    // The plate's translate_y falls from 32 to 0 over frames 0..18 (its
    // entrance), and nothing moves after frame 235 (the last fade-out ends
    // at 235 for "plate").
    let path = lower_third_example_path();
    let out = motion()
        .args([
            "lint",
            path.to_str().unwrap(),
            "--monotone",
            "plate:translate_y:decreasing",
            "--frames",
            "0..18",
            "--settled-after",
            "235",
        ])
        .output()
        .expect("run");
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["problems"].as_array().unwrap().len(), 0);
}

#[test]
fn sheet_the_shipped_lower_third_example_writes_a_png() {
    let path = lower_third_example_path();
    let png = std::env::temp_dir().join(format!(
        "motion-cli-lower-third-sheet-{}.png",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&png);
    let out = motion()
        .args([
            "sheet",
            path.to_str().unwrap(),
            "--every",
            "60",
            "--out",
            png.to_str().unwrap(),
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let img = image::open(&png).expect("decodes").to_rgba8();
    assert!(img.width() > 100 && img.height() > 50, "a real sheet");
    std::fs::remove_file(&png).expect("cleanup");
}

// `title_stagger` is code-built (`Composition::new(..).clip(..)`), never
// loaded through `from_ron`/`from_json`, so it carries no `source` doc —
// `to_ron()` returns `DataError::NotSerializable` unconditionally (see
// `tests/data.rs::code_built_compositions_do_not_serialize`), independent
// of whether any clip is a `Clip::dynamic`. There is no way to produce a
// `.ron` fixture for it without changing `demos.rs` itself (out of this
// task's file scope), so it gets no CLI coverage; this test pins the
// reason so the gap doesn't get rediscovered by surprise.
#[test]
fn title_stagger_demo_has_no_source_doc_so_cannot_become_a_ron_fixture() {
    let err = fenestra_motion::demos::title_stagger()
        .to_ron()
        .expect_err("code-built compositions never carry a source doc");
    assert!(err.to_string().contains("data form"), "{err}");
}
