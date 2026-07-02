//! The shipped demos, verified structurally — each one stresses a different
//! claim of the crate, and each proves itself with the verification layer
//! rather than a human eye: lints run clean, staggers stagger, ranks swap.

use fenestra_motion::verify::{Direction, discontinuities, monotone, settled};
use fenestra_motion::{Frames, Prop, demos};

#[test]
fn all_demos_lint_clean() {
    for (name, comp) in [
        ("lower_third", demos::lower_third()),
        ("title_stagger", demos::title_stagger()),
        ("chart_race", demos::chart_race()),
    ] {
        let problems = discontinuities(&comp, None);
        assert!(problems.is_empty(), "{name}: {problems:?}");
    }
}

#[test]
fn title_stagger_words_enter_in_order_and_settle() {
    let comp = demos::title_stagger();
    let ids = comp.clip_ids();
    assert!(ids.len() >= 3, "one clip per word: {ids:?}");

    // Every word fades in monotonically over its entrance.
    for id in &ids {
        let problems = monotone(&comp, id, Prop::Opacity, 0..comp.total_frames().0, {
            Direction::Increasing
        });
        assert!(problems.is_empty(), "{id}: {problems:?}");
    }

    // Later words start invisible while the first is already moving.
    let early = comp.sample(Frames(4));
    let first = early.resolve(ids[0]).unwrap();
    let last = early.resolve(ids[ids.len() - 1]).unwrap();
    assert!(first.visible, "the first word has entered");
    assert!(
        !last.visible || last.props.opacity < first.props.opacity,
        "the last word trails the first"
    );

    // And the line eventually holds still.
    let dur = comp.total_frames();
    assert!(settled(&comp, Frames(dur.0 - 10)).is_empty());

    // Words sit side by side: bboxes don't overlap once settled.
    let scene = comp.sample(Frames(dur.0 - 5));
    let mut boxes: Vec<kurbo::Rect> = ids
        .iter()
        .map(|id| scene.resolve(id).unwrap().bbox.expect("laid out"))
        .collect();
    boxes.sort_by(|a, b| a.x0.total_cmp(&b.x0));
    for pair in boxes.windows(2) {
        assert!(
            pair[0].x1 <= pair[1].x0 + 1.0,
            "words don't overlap: {pair:?}"
        );
    }
}

#[test]
fn chart_race_ranks_swap_mid_race() {
    let comp = demos::chart_race();
    // The race is a dynamic clip driven by interpolated data: the leader at
    // the start is no longer the leader at the end (that's the race).
    let start = comp.sample(Frames(0)).resolve("race").unwrap();
    let end = comp
        .sample(Frames(comp.total_frames().0 - 1))
        .resolve("race")
        .unwrap();
    assert!(start.visible && end.visible);
    // Structural probe: the chart relabels as ranks change, which shows up
    // in the accessibility description of the sampled trees.
    let first = demos::chart_race_leader(&comp, Frames(0));
    let last = demos::chart_race_leader(&comp, Frames(comp.total_frames().0 - 1));
    assert_ne!(first, last, "the lead changes hands: {first} vs {last}");
}
