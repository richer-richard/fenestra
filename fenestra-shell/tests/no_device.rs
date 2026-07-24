//! The headless API's no-GPU story (adversarial review 2026-07, finding C):
//! when no wgpu adapter exists, the fallible entry points return an
//! actionable [`ShellError::NoDevice`] and the panicking wrappers panic with
//! that message — never an opaque `expect`.
//!
//! macOS-only: wgpu never compiles the GL backend on macOS, so
//! `WGPU_BACKEND=gl` deterministically enumerates zero adapters —
//! independent of whatever Metal or Vulkan drivers the machine has. The
//! workspace forbids `unsafe` (so no `env::set_var`); the wrapper test
//! re-runs this binary with the variable set in the child's environment.
#![cfg(target_os = "macos")]

use fenestra_core::{Theme, col, text};
use fenestra_shell::{ShellError, render_element, try_render_element};

/// The actual assertions; only meaningful with `WGPU_BACKEND=gl` in the
/// process environment, so the wrapper below drives it in a child process.
#[test]
#[ignore = "driven by no_device_is_actionable, which sets WGPU_BACKEND=gl"]
fn probe_no_device() {
    let result = try_render_element(col::<()>().child(text("x")), &Theme::light(), (100, 50));
    let err = result.expect_err("the gl backend never exists on macOS");
    assert!(matches!(err, ShellError::NoDevice), "got: {err:?}");
    let msg = err.to_string();
    assert!(
        msg.contains("mesa-vulkan-drivers"),
        "the error must say how to fix the environment, got: {msg}"
    );

    let panic = std::panic::catch_unwind(|| {
        render_element(col::<()>().child(text("x")), &Theme::light(), (100, 50))
    })
    .expect_err("the panicking wrapper must panic without an adapter");
    let msg = panic.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(
        msg.contains("headless render failed") && msg.contains("mesa-vulkan-drivers"),
        "the wrapper must carry the actionable message, got: {msg}"
    );
}

#[test]
fn no_device_is_actionable() {
    let exe = std::env::current_exe().expect("test binary path");
    let out = std::process::Command::new(exe)
        .args(["--ignored", "--exact", "probe_no_device"])
        .env("WGPU_BACKEND", "gl")
        .output()
        .expect("re-run test binary");
    assert!(
        out.status.success(),
        "probe failed:\n--- stdout ---\n{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
