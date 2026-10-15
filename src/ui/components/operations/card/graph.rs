use crate::ui::scale::{logical, rem_pixels};
use gpui::prelude::*;
use gpui::*;

#[cfg(test)]
#[path = "tests/grid.rs"]
mod grid_tests;

pub(super) fn render(samples: Vec<u64>, accent: Hsla) -> impl IntoElement {
    canvas(
        |bounds, _, _| bounds,
        move |_, bounds: Bounds<Pixels>, window, _| {
            let width = bounds.size.width.as_f32();
            let height = bounds.size.height.as_f32();
            if samples.is_empty() || width <= logical(2.0) || height <= logical(4.0) {
                return;
            }
            let left = bounds.origin.x.as_f32() + logical(8.0);
            let right = bounds.origin.x.as_f32() + width - logical(8.0);
            let top = bounds.origin.y.as_f32() + logical(4.0);
            let bottom = bounds.origin.y.as_f32() + height - logical(4.0);
            let max_speed = samples.iter().copied().max().unwrap_or(0).max(1) as f64;
            let points: Vec<_> = if samples.len() == 1 {
                let y = bottom - (samples[0] as f64 / max_speed) as f32 * (bottom - top);
                vec![point(px(left), px(y)), point(px(right), px(y))]
            } else {
                samples
                    .iter()
                    .enumerate()
                    .map(|(index, speed)| {
                        let x = left + (right - left) * index as f32 / (samples.len() - 1) as f32;
                        let y = bottom - (*speed as f64 / max_speed) as f32 * (bottom - top);
                        point(px(x), px(y))
                    })
                    .collect()
            };
            let mut area = PathBuilder::fill();
            area.move_to(point(px(left), px(bottom)));
            for point in &points {
                area.line_to(*point);
            }
            area.line_to(point(px(right), px(bottom)));
            area.close();
            if let Ok(path) = area.build() {
                window.paint_path(path, accent.opacity(0.08));
            }
            let mut line = PathBuilder::stroke(rem_pixels(rems(0.09375)));
            line.move_to(points[0]);
            for point in points.iter().skip(1) {
                line.line_to(*point);
            }
            if let Ok(path) = line.build() {
                window.paint_path(path, accent.opacity(0.28));
            }
        },
    )
    .w_full()
    .h(rems(3.25))
}
