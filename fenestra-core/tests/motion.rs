//! Motion completion: FLIP / shared-element layout animation (`.animate_layout()`)
//! and exit animations (`.exit()` / `.exit_to()`). Every assertion is headless
//! (no GPU): the retained `FrameState` exposes the motion lifecycle through the
//! `has_anim` / `has_exiting` / `exiting_settled` / `prev_rect` hooks, and
//! `Frame::animating` reports whether the runner must keep scheduling.
//!
//! The load-bearing invariant: under `reduced_motion` (which headless golden
//! rendering forces) FLIP snaps and exits settle on creation, so neither ever
//! perturbs a frame — the goldens stay byte-identical.

use fenestra_core::{Element, Fonts, FrameState, Theme, Transition, build_frame, by, col, div};

/// A fixed canvas so layout rows land at predictable pixels.
const SIZE: (f32, f32) = (200.0, 400.0);

// ----------------------------------------------------------------- FLIP

/// A keyed element pushed down the page by a leading spacer of height `top_h`.
fn flip_view(top_h: f32) -> Element<()> {
    col::<()>().w(200.0).h(400.0).children([
        div::<()>().id("top").w(100.0).h(top_h).shrink0(),
        div::<()>()
            .id("k")
            .w(100.0)
            .h(20.0)
            .shrink0()
            .animate_layout(),
    ])
}

#[test]
fn flip_snaps_under_reduced_motion() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;

    let f1 = build_frame(&flip_view(10.0), &theme, &mut fonts, &mut state, SIZE, 1.0);
    let k = f1.get(&by::id("k")).id;
    let f2 = build_frame(&flip_view(60.0), &theme, &mut fonts, &mut state, SIZE, 1.0);

    // The element jumps straight to its new layout row: no spring is held and
    // nothing animates, so a headless golden is unperturbed.
    assert!(
        !state.has_anim(k),
        "reduced motion must not inject a FLIP spring"
    );
    assert!(!f2.animating, "reduced motion snaps; nothing animates");
    assert!(
        (f2.rect_of(k).unwrap().y0 - 60.0).abs() < 1.0,
        "layout still moved to the new row"
    );
}

#[test]
fn flip_injects_a_spring_on_a_real_move() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new(); // reduced_motion = false

    let f1 = build_frame(&flip_view(10.0), &theme, &mut fonts, &mut state, SIZE, 1.0);
    let k = f1.get(&by::id("k")).id;
    assert!(!f1.animating, "first mount with no movement is settled");
    assert!(
        (state.prev_rect(k).unwrap().y0 - 10.0).abs() < 1.0,
        "the first frame's rect is recorded for next-frame FLIP"
    );

    let f2 = build_frame(&flip_view(60.0), &theme, &mut fonts, &mut state, SIZE, 1.0);
    assert!(
        state.has_anim(k),
        "a real layout move retargets a retained spring"
    );
    assert!(f2.animating, "the FLIP slide keeps the runner scheduling");
    // FLIP offsets paint, not layout: the rect is the new row either way.
    assert!((f2.rect_of(k).unwrap().y0 - 60.0).abs() < 1.0);
    assert!(
        (state.prev_rect(k).unwrap().y0 - 60.0).abs() < 1.0,
        "prev_rects advanced to this frame's measurement"
    );
}

#[test]
fn flip_slide_settles_over_time() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();

    let f1 = build_frame(&flip_view(10.0), &theme, &mut fonts, &mut state, SIZE, 1.0);
    let k = f1.get(&by::id("k")).id;
    let f2 = build_frame(&flip_view(60.0), &theme, &mut fonts, &mut state, SIZE, 1.0);
    assert!(f2.animating, "the slide is in flight right after the move");

    // Hold the new layout steady and let the spring run out.
    state.tick(5.0);
    let f3 = build_frame(&flip_view(60.0), &theme, &mut fonts, &mut state, SIZE, 1.0);
    assert!(
        !f3.animating,
        "the FLIP spring settles and releases the runner"
    );
    assert!(
        state.has_anim(k),
        "the settled spring is retained while mounted"
    );
}

/// Two equal-height rows; identity follows the key when `keyed`, else the index.
fn list_view(order: [&str; 2], keyed: bool) -> Element<()> {
    let item = |name: &str| {
        let d = div::<()>().w(100.0).h(20.0).shrink0().animate_layout();
        if keyed { d.id(name) } else { d }
    };
    col::<()>()
        .w(200.0)
        .h(400.0)
        .children([item(order[0]), item(order[1])])
}

#[test]
fn flip_needs_a_stable_key() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();

    // Index-keyed: reordering swaps which row sits at each index, but the
    // per-index id and pixel position are unchanged, so FLIP cannot see a move.
    let mut state = FrameState::new();
    build_frame(
        &list_view(["a", "b"], false),
        &theme,
        &mut fonts,
        &mut state,
        SIZE,
        1.0,
    );
    let f = build_frame(
        &list_view(["b", "a"], false),
        &theme,
        &mut fonts,
        &mut state,
        SIZE,
        1.0,
    );
    assert!(
        !f.animating,
        "index-keyed reorder cannot FLIP: identity follows position"
    );

    // Stable-keyed: identity follows the key, so the displaced row slides.
    let mut state = FrameState::new();
    let f1 = build_frame(
        &list_view(["a", "b"], true),
        &theme,
        &mut fonts,
        &mut state,
        SIZE,
        1.0,
    );
    let a = f1.get(&by::id("a")).id;
    let f2 = build_frame(
        &list_view(["b", "a"], true),
        &theme,
        &mut fonts,
        &mut state,
        SIZE,
        1.0,
    );
    assert!(f2.animating, "keyed reorder slides the moved row");
    assert!(state.has_anim(a), "the moved row holds a retargeted spring");
}

// ----------------------------------------------------------------- exit

/// A list whose second, exit-tagged child is present only when `present`.
fn exit_view(present: bool) -> Element<()> {
    let mut kids: Vec<Element<()>> = vec![div::<()>().id("keep").w(100.0).h(20.0).shrink0()];
    if present {
        kids.push(
            div::<()>()
                .id("gone")
                .w(100.0)
                .h(20.0)
                .shrink0()
                .exit(Transition::all()),
        );
    }
    col::<()>().w(200.0).h(400.0).children(kids)
}

#[test]
fn exit_ghost_created_on_departure() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new(); // motion on

    let f1 = build_frame(&exit_view(true), &theme, &mut fonts, &mut state, SIZE, 1.0);
    let gone = f1.get(&by::id("gone")).id;
    assert!(!state.has_exiting(gone), "a present element is not exiting");

    let f2 = build_frame(&exit_view(false), &theme, &mut fonts, &mut state, SIZE, 1.0);
    assert!(
        state.has_exiting(gone),
        "removing an exit-tagged element starts an exit"
    );
    assert_eq!(
        state.exiting_settled(gone),
        Some(false),
        "the exit is mid-flight, not settled"
    );
    assert!(
        f2.animating,
        "an in-flight exit keeps the runner scheduling"
    );

    // Painting at t0 (no clock advance) must not panic nor settle the ghost.
    let _ = f2.paint(&mut fonts, &mut state);
    assert!(
        state.has_exiting(gone),
        "still exiting immediately after one paint at t0"
    );
}

#[test]
fn exit_settles_instantly_under_reduced_motion_and_is_gced() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;

    let f1 = build_frame(&exit_view(true), &theme, &mut fonts, &mut state, SIZE, 1.0);
    let gone = f1.get(&by::id("gone")).id;

    let f2 = build_frame(&exit_view(false), &theme, &mut fonts, &mut state, SIZE, 1.0);
    // Detected, but settled at once: it is never painted and the frame is inert.
    assert_eq!(state.exiting_settled(gone), Some(true));
    assert!(
        !f2.animating,
        "a settled exit does not animate (headless stays inert)"
    );
    let _ = f2.paint(&mut fonts, &mut state); // a settled ghost paints nothing

    let _f3 = build_frame(&exit_view(false), &theme, &mut fonts, &mut state, SIZE, 1.0);
    assert!(
        !state.has_exiting(gone),
        "a settled exit is garbage-collected on the next build"
    );
}

#[test]
fn exit_cancels_when_the_element_reappears() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();

    let f1 = build_frame(&exit_view(true), &theme, &mut fonts, &mut state, SIZE, 1.0);
    let gone = f1.get(&by::id("gone")).id;
    let _f2 = build_frame(&exit_view(false), &theme, &mut fonts, &mut state, SIZE, 1.0);
    assert!(state.has_exiting(gone), "the exit started");

    let f3 = build_frame(&exit_view(true), &theme, &mut fonts, &mut state, SIZE, 1.0);
    assert!(
        !state.has_exiting(gone),
        "the element reappearing cancels its exit"
    );
    assert!(
        f3.query(&by::id("gone")).is_some(),
        "and it is live again in the tree"
    );
}

#[test]
fn exit_completes_after_its_duration_then_is_collected() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();

    let f1 = build_frame(&exit_view(true), &theme, &mut fonts, &mut state, SIZE, 1.0);
    let gone = f1.get(&by::id("gone")).id;
    let f2 = build_frame(&exit_view(false), &theme, &mut fonts, &mut state, SIZE, 1.0);
    assert_eq!(state.exiting_settled(gone), Some(false));

    // Advance well past the 200ms exit and paint: the ghost reaches its end.
    state.tick(5.0);
    let _ = f2.paint(&mut fonts, &mut state);
    assert_eq!(
        state.exiting_settled(gone),
        Some(true),
        "the exit settles once its duration has elapsed"
    );

    let _f3 = build_frame(&exit_view(false), &theme, &mut fonts, &mut state, SIZE, 1.0);
    assert!(
        !state.has_exiting(gone),
        "the completed exit is collected next build"
    );
}

#[test]
fn exit_to_targets_scale_and_translate() {
    // `.exit_to` configures opacity/scale/translation targets and a default
    // exit timing; removal still leaves a tracked, in-flight ghost.
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();

    let view = |present: bool| -> Element<()> {
        let mut kids: Vec<Element<()>> = vec![div::<()>().id("keep").w(100.0).h(20.0).shrink0()];
        if present {
            kids.push(
                div::<()>()
                    .id("toast")
                    .w(100.0)
                    .h(20.0)
                    .shrink0()
                    .exit_to(0.0, 0.96, 0.0, 8.0),
            );
        }
        col::<()>().w(200.0).h(400.0).children(kids)
    };

    let f1 = build_frame(&view(true), &theme, &mut fonts, &mut state, SIZE, 1.0);
    let toast = f1.get(&by::id("toast")).id;
    let f2 = build_frame(&view(false), &theme, &mut fonts, &mut state, SIZE, 1.0);
    assert_eq!(state.exiting_settled(toast), Some(false));
    assert!(f2.animating);
    // Mid-flight paint exercises the scale/translate ghost path without panic.
    state.tick(0.05);
    let _ = f2.paint(&mut fonts, &mut state);
    assert!(state.has_exiting(toast));
}

// ---------------------------------------------------------- FLIP + exit

/// An element that both slides (`animate_layout`) and fades (`exit`), removed
/// while its FLIP slide is in flight: the ghost must inherit the slid offset, so
/// it animates out from where the element actually was — not from its settled
/// layout rect. Guards two halves of the fix together: the snapshot is taken
/// after the FLIP pass, and the ghost painter replays that frozen translate.
#[test]
fn exit_ghost_inherits_a_mid_flip_slide() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new(); // motion on

    // "k" both slides and fades; a leading spacer of height `top_h` pushes it.
    let view = |top_h: f32, present: bool| -> Element<()> {
        let mut kids: Vec<Element<()>> = vec![div::<()>().id("top").w(100.0).h(top_h).shrink0()];
        if present {
            kids.push(
                div::<()>()
                    .id("k")
                    .w(100.0)
                    .h(20.0)
                    .shrink0()
                    .animate_layout()
                    .exit(Transition::all()),
            );
        }
        col::<()>().w(200.0).h(400.0).children(kids)
    };

    let f1 = build_frame(&view(10.0, true), &theme, &mut fonts, &mut state, SIZE, 1.0);
    let k = f1.get(&by::id("k")).id;
    // A real move: "k" slides down 50px, so its FLIP translate this frame is
    // -50 in y (painted back at the old position, springing toward 0).
    let _f2 = build_frame(&view(60.0, true), &theme, &mut fonts, &mut state, SIZE, 1.0);
    assert!(state.has_anim(k), "the move injected a FLIP spring");

    // Remove it on the very next frame, mid-slide. The ghost is snapshotted
    // after FLIP, so it carries the slid translate, not (0, 0).
    let f3 = build_frame(
        &view(60.0, false),
        &theme,
        &mut fonts,
        &mut state,
        SIZE,
        1.0,
    );
    assert!(
        state.has_exiting(k),
        "removing the sliding element starts an exit"
    );
    let t = state
        .exiting_ghost_translate(k)
        .expect("a removed exit-tagged element is exiting");
    assert!(
        t.1 < -40.0,
        "the exit ghost inherits the mid-FLIP slide (translate.y ~= -50), got {t:?}"
    );
    // Painting the transformed ghost exercises the frozen-transform path.
    let _ = f3.paint(&mut fonts, &mut state);
}
