//! Best-effort detection of the OS "reduce motion" accessibility setting.
//!
//! Queried through each platform's own CLI rather than native FFI, so the
//! workspace's `unsafe_code = forbid` holds (no `objc2` / Win32 bindings). The
//! query is cheap and run only at window creation and on focus-regained, so a
//! mid-session toggle is picked up. Any read failure — or an unrecognized
//! platform — reports `false` (full motion), the safe, unchanged default.

#[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
fn query() -> bool {
    // `reduceMotion = 1` under the Universal Access domain.
    std::process::Command::new("defaults")
        .args(["read", "com.apple.universalaccess", "reduceMotion"])
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "1")
        .unwrap_or(false)
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "linux"))]
fn query() -> bool {
    // GNOME: animations disabled is the reduce-motion signal.
    std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "enable-animations"])
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "false")
        .unwrap_or(false)
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
fn query() -> bool {
    // The WindowMetrics `MinAnimate` REG_SZ is `0` when animations are off.
    std::process::Command::new("reg")
        .args([
            "query",
            r"HKCU\Control Panel\Desktop\WindowMetrics",
            "/v",
            "MinAnimate",
        ])
        .output()
        .map(|o| {
            // `MinAnimate    REG_SZ    0` — the value is the last token. `.trim()`
            // narrows the `Cow` to a `&str` first, then `&str` iterators apply.
            o.status.success()
                && String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .split_whitespace()
                    .next_back()
                    == Some("0")
        })
        .unwrap_or(false)
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(any(target_os = "macos", target_os = "linux", target_os = "windows"))
))]
fn query() -> bool {
    false
}

/// Whether the OS currently requests reduced motion. Drives
/// [`FrameState::reduced_motion`](fenestra_core::FrameState) so animations snap
/// for users who asked for it (WCAG 2.3.3). Non-wasm only; the web build reads
/// `prefers-reduced-motion` through the browser instead.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn os_reduce_motion() -> bool {
    query()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    #[test]
    fn os_reduce_motion_never_panics() {
        // The value is environment-dependent (the user's accessibility setting);
        // what we guarantee is that querying it is safe and falls back cleanly
        // when the platform CLI is missing.
        let _ = super::os_reduce_motion();
    }
}
