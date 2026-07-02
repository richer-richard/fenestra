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

fn write_doc() -> PathBuf {
    let path = std::env::temp_dir().join(format!("motion-cli-{}.ron", std::process::id()));
    std::fs::write(&path, DOC).expect("write doc");
    path
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
