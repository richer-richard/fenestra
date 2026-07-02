//! The chart-race demo: a dynamic clip rebuilding a fenestra-charts bar
//! chart every frame from rank-sorted, track-interpolated data. Pass
//! `-- --mp4` to also encode a video (needs ffmpeg on PATH; PNG output
//! never does).
//!
//!   cargo run -p fenestra-motion --example chart_race
//!   cargo run -p fenestra-motion --example chart_race -- --mp4

use fenestra_motion::{Frames, demos};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let comp = demos::chart_race();

    // The race's claim, verified structurally: the lead changes hands.
    let first = demos::chart_race_leader(&comp, Frames(0));
    let last = demos::chart_race_leader(&comp, Frames(comp.total_frames().0 - 1));
    assert_ne!(first, last, "somebody overtakes somebody");

    let dir = std::path::Path::new("target/motion-demos/chart_race");
    std::fs::create_dir_all(dir)?;
    comp.contact_sheet(15, 240)?.save(dir.join("sheet.png"))?;
    println!(
        "{first} leads at the start; {last} takes it. sheet.png in {}",
        dir.display()
    );

    if std::env::args().any(|a| a == "--mp4") {
        let out = dir.join("chart_race.mp4");
        comp.render_video(0..comp.total_frames().0, &out)?;
        println!("encoded {}", out.display());
    }
    Ok(())
}
