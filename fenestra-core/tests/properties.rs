//! Property tests for the core invariants the verification story rests
//! on: layout and paint are total (no tree shape panics), Tab visits
//! every enabled focusable exactly once per cycle, and widget ids are
//! unique within a frame.

use std::cell::RefCell;

use fenestra_core::{
    AccessNode, Element, Fonts, FrameState, InputEvent, Overlay, Theme, build_frame, col, dispatch,
    div, image_rgba8, row, text,
};
use proptest::prelude::*;

thread_local! {
    static FONTS: RefCell<Fonts> = RefCell::new(Fonts::embedded());
}

/// One styling tweak; trees compose several per node.
#[derive(Debug, Clone)]
enum Mod {
    Pad(f32),
    Gap(f32),
    Width(f32),
    Height(f32),
    Grow,
    Shrink0,
    Wrap,
    ItemsCenter,
    JustifyBetween,
    Rounded(f32),
    Opacity(f32),
    Scroll,
}

/// Mostly sane values, sometimes hostile ones (NaN/infinite/negative)
/// — the boundary must sanitize, never panic.
fn arb_len() -> impl Strategy<Value = f32> {
    prop_oneof![
        8 => 0f32..1.0e6,
        1 => Just(f32::NAN),
        1 => Just(f32::INFINITY),
        1 => Just(f32::NEG_INFINITY),
        1 => Just(-100.0f32),
        1 => Just(3.2e38f32),
    ]
}

fn arb_mod() -> impl Strategy<Value = Mod> {
    prop_oneof![
        (0f32..200.0).prop_map(Mod::Pad),
        arb_len().prop_map(Mod::Pad),
        (0f32..200.0).prop_map(Mod::Gap),
        arb_len().prop_map(Mod::Gap),
        arb_len().prop_map(Mod::Width),
        arb_len().prop_map(Mod::Height),
        Just(Mod::Grow),
        Just(Mod::Shrink0),
        Just(Mod::Wrap),
        Just(Mod::ItemsCenter),
        Just(Mod::JustifyBetween),
        (0f32..100.0).prop_map(Mod::Rounded),
        (-1f32..2.0).prop_map(Mod::Opacity),
        Just(Mod::Scroll),
    ]
}

fn apply(mut el: Element<()>, mods: Vec<Mod>, seq: u32) -> Element<()> {
    for m in mods {
        el = match m {
            Mod::Pad(v) => el.p(v),
            Mod::Gap(v) => el.gap(v),
            Mod::Width(v) => el.w(v),
            Mod::Height(v) => el.h(v),
            Mod::Grow => el.grow(),
            Mod::Shrink0 => el.shrink0(),
            Mod::Wrap => el.wrap(),
            Mod::ItemsCenter => el.items_center(),
            Mod::JustifyBetween => el.justify_between(),
            Mod::Rounded(v) => el.rounded(v),
            Mod::Opacity(v) => el.opacity(v),
            // Scroll containers need stable identity; derive a unique key.
            Mod::Scroll => el.scroll_y().id(&format!("s{seq}")),
        };
    }
    el
}

/// A plain-data recipe for a tree — proptest needs Clone + Debug, and
/// shrunk counterexamples print readably; the Element materializes from
/// it inside each case.
#[derive(Debug, Clone)]
enum Plan {
    Empty,
    Text(String),
    Image,
    Sized(f32, f32),
    Node {
        is_row: bool,
        mods: Vec<Mod>,
        overlay: bool,
        seq: u32,
        children: Vec<Plan>,
    },
}

fn materialize(plan: &Plan) -> Element<()> {
    match plan {
        Plan::Empty => div(),
        Plan::Text(s) => text(s.clone()),
        Plan::Image => image_rgba8(2, 2, vec![128; 16]),
        Plan::Sized(w, h) => div().w(*w).h(*h),
        Plan::Node {
            is_row,
            mods,
            overlay,
            seq,
            children,
        } => {
            let container = if *is_row { row() } else { col() };
            let mut el = apply(
                container.children(children.iter().map(materialize)),
                mods.clone(),
                *seq,
            );
            if *overlay {
                el = el.overlay(Overlay::modal());
            }
            el
        }
    }
}

fn arb_tree() -> impl Strategy<Value = Plan> {
    let leaf = prop_oneof![
        Just(Plan::Empty),
        "[a-zA-Z0-9 .,!?]{0,60}".prop_map(Plan::Text),
        Just(Plan::Image),
        (0f32..4000.0, 0f32..4000.0).prop_map(|(w, h)| Plan::Sized(w, h)),
    ];
    leaf.prop_recursive(4, 48, 6, |inner| {
        (
            proptest::collection::vec(inner, 0..6),
            proptest::collection::vec(arb_mod(), 0..5),
            any::<bool>(),
            any::<u32>(),
            // A few nodes float as open overlays (the modal layout path).
            proptest::num::u8::ANY,
        )
            .prop_map(|(children, mods, is_row, seq, overlay_die)| Plan::Node {
                is_row,
                mods,
                overlay: overlay_die < 8,
                seq,
                children,
            })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// Any tree, any viewport: building and painting never panic.
    #[test]
    fn layout_and_paint_are_total(
        plan in arb_tree(),
        w in 0f32..2400.0,
        h in 0f32..2400.0,
    ) {
        let tree = materialize(&plan);
        FONTS.with(|fonts| {
            let mut fonts = fonts.borrow_mut();
            let mut state = FrameState::new();
            state.reduced_motion = true;
            let frame = build_frame(&tree, &Theme::light(), &mut fonts, &mut state, (w, h), 1.0);
            let _scene = frame.paint(&mut fonts, &mut state);
        });
    }

    /// Tab from a clean state visits every enabled focusable exactly
    /// once before wrapping (a permutation, never a trap or a skip).
    #[test]
    fn tab_visits_each_enabled_focusable_once(
        groups in proptest::collection::vec(
            proptest::collection::vec((any::<bool>(), any::<bool>()), 0..5),
            0..5,
        ),
    ) {
        let enabled_total: usize = groups
            .iter()
            .flatten()
            .filter(|(focusable, disabled)| *focusable && !disabled)
            .count();
        let view: Element<()> = col().children(groups.iter().map(|group| {
            row().children(group.iter().map(|(focusable, disabled)| {
                div()
                    .w(20.0)
                    .h(20.0)
                    .focusable(*focusable)
                    .disabled(*disabled)
            }))
        }));
        FONTS.with(|fonts| {
            let mut fonts = fonts.borrow_mut();
            let mut state = FrameState::new();
            state.reduced_motion = true;
            let mut visited = Vec::new();
            for _ in 0..enabled_total {
                let frame = build_frame(
                    &view, &Theme::light(), &mut fonts, &mut state, (600.0, 400.0), 1.0,
                );
                dispatch(&view, &frame, &mut state, &mut fonts, InputEvent::Tab);
                let focus = state.focused().expect("tab focuses something");
                prop_assert!(!visited.contains(&focus), "tab revisited {focus:?} early");
                visited.push(focus);
            }
            prop_assert_eq!(visited.len(), enabled_total);
            if enabled_total > 0 {
                // One more Tab wraps around to the first.
                let frame = build_frame(
                    &view, &Theme::light(), &mut fonts, &mut state, (600.0, 400.0), 1.0,
                );
                dispatch(&view, &frame, &mut state, &mut fonts, InputEvent::Tab);
                prop_assert_eq!(state.focused(), Some(visited[0]));
            }
            Ok(())
        })?;
    }

    /// Every node in a frame has a unique id — retained state (focus,
    /// scroll, editors) and the platform accessibility tree key off it.
    #[test]
    fn widget_ids_are_unique_per_frame(plan in arb_tree()) {
        let tree = materialize(&plan);
        FONTS.with(|fonts| {
            let mut fonts = fonts.borrow_mut();
            let mut state = FrameState::new();
            state.reduced_motion = true;
            let frame = build_frame(
                &tree, &Theme::light(), &mut fonts, &mut state, (800.0, 600.0), 1.0,
            );
            fn walk(n: &AccessNode, out: &mut Vec<fenestra_core::WidgetId>) {
                out.push(n.id);
                for c in &n.children {
                    walk(c, out);
                }
            }
            let mut ids = Vec::new();
            walk(&frame.access_tree(), &mut ids);
            let total = ids.len();
            ids.sort_unstable();
            ids.dedup();
            prop_assert_eq!(ids.len(), total, "duplicate widget ids in one frame");
            Ok(())
        })?;
    }
}

/// Non-finite style values must not reach the text pipeline: parley's
/// line breaker hard-asserts on inconsistent max_advance (found by the
/// layout fuzzer, 2026-06-12). The boundary sanitizes them instead.
#[test]
fn non_finite_dimensions_never_panic() {
    FONTS.with(|fonts| {
        let mut fonts = fonts.borrow_mut();
        for w in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -5.0,
            3.2e38,
            1.9e28,
        ] {
            for case in 0..4 {
                let tree: Element<()> = match case {
                    0 => col().w(w).children([text("wrap me across lines please")]),
                    1 => col().p(w).children([text("padded text body")]),
                    2 => row().gap(w).children([text("a"), text("b c d e f g")]),
                    _ => col()
                        .w(200.0)
                        .children([div().w(w).h(w).child(text("nested hostile width"))]),
                };
                let mut state = FrameState::new();
                state.reduced_motion = true;
                let frame = build_frame(
                    &tree,
                    &Theme::light(),
                    &mut fonts,
                    &mut state,
                    (300.0, 200.0),
                    1.0,
                );
                let _ = frame.paint(&mut fonts, &mut state);
            }
        }
    });
}
