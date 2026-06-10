//! M4 acceptance: switch travel animation goldens at t = 0, 0.5, 1 with
//! reduced motion OFF, driven through the real transition engine with a
//! controlled clock.

use std::path::PathBuf;

use fenestra_core::{Element, FrameState, SP4, Theme, build_frame, col};
use fenestra_kit::switch;
use fenestra_shell::{testing::assert_png_snapshot, with_fonts, with_headless};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

fn view<Msg: Clone + 'static>(on: bool) -> Element<Msg> {
    col().p(SP4).items_start().children([switch(on)])
}

const SIZE: (u32, u32) = (80, 56);
/// The switch travel duration is 160ms.
const TRAVEL: f64 = 0.160;

#[test]
fn switch_travel_goldens() {
    let theme = Theme::light();
    let mut state = FrameState::new();
    state.reduced_motion = false; // animate for real

    let shot = |state: &mut FrameState, on: bool, t: f64, name: &str, expect_anim: bool| {
        state.tick(t);
        let scene = with_fonts(|fonts| {
            #[expect(clippy::cast_precision_loss, reason = "test sizes are tiny")]
            let frame = build_frame(
                &view::<()>(on),
                &theme,
                fonts,
                state,
                (SIZE.0 as f32, SIZE.1 as f32),
                1.0,
            );
            assert_eq!(
                frame.animating, expect_anim,
                "animating flag at t={t} for {name}"
            );
            frame.paint(fonts, state)
        });
        let image = with_headless(|h| h.render(&scene, SIZE.0, SIZE.1, theme.bg))
            .expect("headless")
            .expect("render");
        assert_png_snapshot(snapshot_dir(), name, &image);
    };

    // Settle the off state, then flip on at t=0.
    shot(&mut state, false, 0.0, "switch_anim_t0_off", false);
    // Retarget frame: still at the off visuals (t = 0 of the segment).
    shot(&mut state, true, 0.0, "switch_anim_t0", true);
    // Mid-flight.
    shot(&mut state, true, TRAVEL * 0.5, "switch_anim_t50", true);
    // Settled.
    shot(&mut state, true, TRAVEL, "switch_anim_t100", false);
}
