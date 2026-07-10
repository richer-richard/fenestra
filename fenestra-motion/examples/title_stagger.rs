//! The per-word stagger demo: one clip per word, entrance offsets probed
//! from a layout pass instead of hand-tuned pixels — the manual form of the
//! pattern a text animator would generalize.
//!
//!   cargo run -p fenestra-motion --example title_stagger

use fenestra_motion::{Frames, demos, verify};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let comp = demos::title_stagger();

    let problems = verify::discontinuities(&comp, None);
    assert!(problems.is_empty(), "the demo lints clean: {problems:?}");
    // The line holds still once every word has landed.
    assert!(verify::settled(&comp, Frames(60)).is_empty());

    let dir = std::path::Path::new("target/motion-demos/title_stagger");
    std::fs::create_dir_all(dir)?;
    comp.contact_sheet(10, 240)?.save(dir.join("sheet.png"))?;
    for frame in [4u64, 12, 24, 60] {
        comp.render_frame_png(Frames(frame), dir.join(format!("f{frame:05}.png")))?;
    }
    comp.render_png_sequence(0..comp.total_frames().0, dir.join("frames"))?;

    println!(
        "wrote entrance frames + sheet.png + full sequence to {}",
        dir.display()
    );
    Ok(())
}
