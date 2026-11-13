use crate::ui::scale::{logical, rem_pixels};
use gpui::prelude::*;
use gpui::*;

#[cfg(test)]
#[path = "tests/grid.rs"]
mod grid_tests;

const COLUMNS: usize = 12;
const ROWS: usize = 4;

/// Where samples are drawn: as wide as the progress bar below, inset vertically
/// so the line's peaks stay inside.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Plot {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl Plot {
    fn within(bounds: Bounds<Pixels>) -> Self {
        let x = bounds.origin.x.as_f32();
        let y = bounds.origin.y.as_f32();
        Self {
            left: x,
            right: x + bounds.size.width.as_f32(),
            top: y + logical(4.0),
            bottom: y + bounds.size.height.as_f32() - logical(4.0),
        }
    }

    /// Grid lines framing the plot exactly: verticals, then horizontals.
    fn grid(&self) -> (Vec<f32>, Vec<f32>) {
        let step = |from: f32, to: f32, cells: usize| {
            (0..=cells)
                .map(|index| from + (to - from) * index as f32 / cells as f32)
                .collect::<Vec<_>>()
        };
        (
            step(self.left, self.right, COLUMNS),
            step(self.top, self.bottom, ROWS),
        )
    }
}

/// Speed over time on a grid that fades downward, as a trace on paper.
pub(super) fn render(samples: Vec<u64>, color: Hsla, grid_color: Hsla) -> impl IntoElement {
    canvas(
        |bounds, _, _| bounds,
        move |_, bounds: Bounds<Pixels>, window, _| {
            if samples.is_empty()
                || bounds.size.width.as_f32() <= logical(18.0)
                || bounds.size.height.as_f32() <= logical(10.0)
            {
                return;
            }
            let plot = Plot::within(bounds);
            let line = px(logical(1.0));
            let (columns, rows) = plot.grid();
            let fade = |at: f32| {
                let depth = (at - plot.top) / (plot.bottom - plot.top);
                grid_color.opacity(0.28 * (1.0 - depth * 0.85))
            };
            for x in columns {
                window.paint_quad(fill(
                    Bounds::new(
                        point(px(x) - line / 2.0, px(plot.top)),
                        size(line, px(plot.bottom - plot.top)),
                    ),
                    linear_gradient(
                        180.0,
                        linear_color_stop(fade(plot.top), 0.0),
                        linear_color_stop(fade(plot.bottom), 1.0),
                    ),
                ));
            }
            for y in rows {
                window.paint_quad(fill(
                    Bounds::new(
                        point(px(plot.left), px(y) - line / 2.0),
                        size(px(plot.right - plot.left), line),
                    ),
                    fade(y),
                ));
            }
            let max_speed = samples.iter().copied().max().unwrap_or(0).max(1) as f64;
            let height = plot.bottom - plot.top;
            let points: Vec<_> = if samples.len() == 1 {
                let y = plot.bottom - (samples[0] as f64 / max_speed) as f32 * height;
                vec![point(px(plot.left), px(y)), point(px(plot.right), px(y))]
            } else {
                samples
                    .iter()
                    .enumerate()
                    .map(|(index, speed)| {
                        let x = plot.left
                            + (plot.right - plot.left) * index as f32 / (samples.len() - 1) as f32;
                        let y = plot.bottom - (*speed as f64 / max_speed) as f32 * height;
                        point(px(x), px(y))
                    })
                    .collect()
            };
            let mut area = PathBuilder::fill();
            area.move_to(point(px(plot.left), px(plot.bottom)));
            for point in &points {
                area.line_to(*point);
            }
            area.line_to(point(px(plot.right), px(plot.bottom)));
            area.close();
            if let Ok(path) = area.build() {
                window.paint_path(path, color.opacity(0.14));
            }
            let mut trace = PathBuilder::stroke(rem_pixels(rems(0.09375)));
            trace.move_to(points[0]);
            for point in points.iter().skip(1) {
                trace.line_to(*point);
            }
            if let Ok(path) = trace.build() {
                window.paint_path(path, color);
            }
        },
    )
    .w_full()
    .h(rems(6.0))
}
