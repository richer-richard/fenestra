use fenestra_core::{Element, Overlay, col, div, image_rgba8, row, text};

#[derive(Arbitrary, Debug)]
enum Plan {
    Empty,
    Text(String),
    Image,
    Sized(f32, f32),
    Node {
        is_row: bool,
        pad: f32,
        gap: f32,
        width: Option<f32>,
        height: Option<f32>,
        grow: bool,
        wrap: bool,
        scroll: bool,
        overlay: bool,
        opacity: f32,
        children: Vec<Plan>,
    },
}

fn materialize(plan: &Plan, depth: usize, seq: &mut u32) -> Element<()> {
    if depth > 5 {
        return div();
    }
    match plan {
        Plan::Empty => div(),
        Plan::Text(s) => text(s.clone()),
        Plan::Image => image_rgba8(2, 2, vec![200; 16]),
        Plan::Sized(w, h) => div().w(*w).h(*h),
        Plan::Node {
            is_row,
            pad,
            gap,
            width,
            height,
            grow,
            wrap,
            scroll,
            overlay,
            opacity,
            children,
        } => {
            let mut el = if *is_row { row() } else { col() };
            el = el.p(*pad).gap(*gap).opacity(*opacity);
            if let Some(w) = width {
                el = el.w(*w);
            }
            if let Some(h) = height {
                el = el.h(*h);
            }
            if *grow {
                el = el.grow();
            }
            if *wrap {
                el = el.wrap();
            }
            if *scroll {
                *seq += 1;
                el = el.scroll_y().id(&format!("s{seq}"));
            }
            if *overlay {
                el = el.overlay(Overlay::modal());
            }
            el.children(
                children
                    .iter()
                    .take(6)
                    .map(|c| materialize(c, depth + 1, seq)),
            )
        }
    }
}

