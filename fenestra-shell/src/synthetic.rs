//! Synthetic event injection for headless testing: agents drive an [`App`]
//! with scripted input and look at the resulting pixels.

use fenestra_core::{App, InputEvent, KeyInput, Theme};
use image::RgbaImage;

/// A scripted input event for [`render_app`].
#[derive(Debug, Clone, PartialEq)]
pub enum SyntheticEvent {
    /// Move the pointer to logical coordinates.
    MouseMove {
        /// Logical x.
        x: f32,
        /// Logical y.
        y: f32,
    },
    /// Press the primary button.
    MouseDown,
    /// Release the primary button.
    MouseUp,
    /// Press the secondary (right) button.
    RightDown,
    /// Release the secondary (right) button.
    RightUp,
    /// Drop an OS file at the current pointer position.
    FileDrop(std::path::PathBuf),
    /// Press a key.
    Key(KeyInput),
    /// Commit text (M5).
    Text(String),
    /// Scroll (winit convention: positive `dy` moves content down, positive
    /// `dx` moves content right).
    Wheel {
        /// Horizontal delta in logical px.
        dx: f32,
        /// Vertical delta in logical px.
        dy: f32,
    },
    /// Focus next.
    Tab,
    /// Focus previous.
    ShiftTab,
    /// Modifier keys changed (shift, ctrl, alt, meta).
    Modifiers(bool, bool, bool, bool),
}

impl From<&SyntheticEvent> for InputEvent {
    fn from(ev: &SyntheticEvent) -> Self {
        match ev {
            SyntheticEvent::MouseMove { x, y } => Self::PointerMove { x: *x, y: *y },
            SyntheticEvent::MouseDown => Self::PointerDown,
            SyntheticEvent::MouseUp => Self::PointerUp,
            SyntheticEvent::RightDown => Self::RightDown,
            SyntheticEvent::RightUp => Self::RightUp,
            SyntheticEvent::FileDrop(p) => Self::FileDrop(p.clone()),
            SyntheticEvent::Key(k) => Self::Key(*k),
            SyntheticEvent::Text(s) => Self::Text(s.clone()),
            SyntheticEvent::Wheel { dx, dy } => Self::Wheel { dx: *dx, dy: *dy },
            SyntheticEvent::Tab => Self::Tab,
            SyntheticEvent::ShiftTab => Self::ShiftTab,
            SyntheticEvent::Modifiers(shift, ctrl, alt, meta) => Self::Modifiers {
                shift: *shift,
                ctrl: *ctrl,
                alt: *alt,
                meta: *meta,
            },
        }
    }
}

/// Drives an app headlessly: dispatches each event against the current
/// view, applies the emitted messages, then renders one settle frame.
/// Deterministic: scale 1.0, reduced motion, embedded fonts only. The
/// requested size is clamped to the device-supported range (at least 1x1,
/// at most the maximum texture dimension).
///
/// [`App::init`] runs first with a collecting [`Proxy`]; proxied messages
/// are applied at deterministic points (before each event and before the
/// settle frame). Messages sent from spawned threads race those drain
/// points — keep proxy use synchronous in tests.
///
/// # Panics
/// If no compute-capable GPU adapter exists or rendering fails.
pub fn render_app<A: App>(
    app: &mut A,
    events: &[SyntheticEvent],
    size: (u32, u32),
    theme: &Theme,
) -> RgbaImage
where
    A::Msg: Send,
{
    let mut harness = crate::Harness::new(&mut *app, theme.clone(), size);
    for ev in events {
        harness.input(ev.into());
    }
    harness.render()
}
