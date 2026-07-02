//! The lower-third demo, end to end: load the shipped RON document, lint
//! it, render the straight-alpha PNG sequence, and tile a contact sheet —
//! the whole agent loop with no window.
//!
//!   cargo run -p fenestra-motion --example lower_third
//!
//! Outputs land in `target/motion-demos/lower_third/`. To deliver with
//! alpha, encode the sequence with the ProRes 4444 / VP9 recipes in this
//! crate's README.

use fenestra_motion::{demos, verify};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let comp = demos::lower_third();

    let problems = verify::discontinuities(&comp, None);
    assert!(problems.is_empty(), "the demo lints clean: {problems:?}");

    let dir = std::path::Path::new("target/motion-demos/lower_third");
    comp.render_png_sequence(0..comp.total_frames().0, dir)?;
    comp.contact_sheet(24, 240)?.save(dir.join("sheet.png"))?;

    println!(
        "wrote {} straight-alpha frames + sheet.png to {}",
        comp.total_frames(),
        dir.display()
    );
    println!("alpha delivery: see the ffmpeg recipes in fenestra-motion/README.md");
    Ok(())
}
