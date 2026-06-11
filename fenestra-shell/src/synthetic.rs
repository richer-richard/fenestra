//! Synthetic event injection for headless testing: agents drive an [`App`]
//! with scripted input and look at the resulting pixels.

use fenestra_core::{App, FrameState, InputEvent, KeyInput, Theme, build_frame, dispatch};
use image::RgbaImage;

use crate::element_render::with_fonts;
use crate::with_headless;

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
    /// Press a key.
    Key(KeyInput),
    /// Commit text (M5).
    Text(String),
    /// Scroll (winit convention: positive `dy` moves content down).
    Wheel {
        /// Vertical delta in logical px.
        dy: f32,
    },
    /// Focus next.
    Tab,
    /// Focus previous.
    ShiftTab,
}

impl From<&SyntheticEvent> for InputEvent {
    fn from(ev: &SyntheticEvent) -> Self {
        match ev {
            SyntheticEvent::MouseMove { x, y } => Self::PointerMove { x: *x, y: *y },
            SyntheticEvent::MouseDown => Self::PointerDown,
            SyntheticEvent::MouseUp => Self::PointerUp,
            SyntheticEvent::Key(k) => Self::Key(*k),
            SyntheticEvent::Text(s) => Self::Text(s.clone()),
            SyntheticEvent::Wheel { dy } => Self::Wheel { dy: *dy },
            SyntheticEvent::Tab => Self::Tab,
            SyntheticEvent::ShiftTab => Self::ShiftTab,
        }
    }
}

/// Drives an app headlessly: dispatches each event against the current
/// view, applies the emitted messages, then renders one settle frame.
/// Deterministic: scale 1.0, reduced motion, embedded fonts only. The
/// requested size is clamped to the device-supported range (at least 1x1,
/// at most the maximum texture dimension).
///
/// # Panics
/// If no compute-capable GPU adapter exists or rendering fails.
pub fn render_app<A: App>(
    app: &mut A,
    events: &[SyntheticEvent],
    size: (u32, u32),
    theme: &Theme,
) -> RgbaImage {
    // Clamp before layout so the frames and the texture agree on the size.
    let size =
        with_headless(|h| h.clamp_size(size.0, size.1)).expect("headless renderer unavailable");
    let mut state = FrameState::new();
    state.reduced_motion = true;
    #[expect(clippy::cast_precision_loss, reason = "window sizes fit in f32")]
    let logical = (size.0 as f32, size.1 as f32);

    let scene = with_fonts(|fonts| {
        for ev in events {
            let view = app.view();
            let frame = build_frame(&view, theme, fonts, &mut state, logical, 1.0);
            let result = dispatch(&view, &frame, &mut state, fonts, ev.into());
            for msg in result.msgs {
                app.update(msg);
            }
        }
        // One settle frame after the last event.
        let view = app.view();
        let frame = build_frame(&view, theme, fonts, &mut state, logical, 1.0);
        frame.paint(fonts, &mut state)
    });
    with_headless(|headless| headless.render(&scene, size.0, size.1, theme.bg))
        .expect("headless renderer unavailable")
        .expect("headless render failed")
}
