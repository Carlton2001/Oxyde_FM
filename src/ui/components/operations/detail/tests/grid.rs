use super::{Plot, COLUMNS, ROWS};
use gpui::{point, px, size, Bounds};

#[test]
fn grid_frames_the_plotted_area() {
    let bounds = Bounds::new(point(px(10.0), px(20.0)), size(px(400.0), px(52.0)));
    let plot = Plot::within(bounds);
    let (columns, rows) = plot.grid();
    assert_eq!(columns.len(), COLUMNS + 1);
    assert_eq!(rows.len(), ROWS + 1);
    assert_eq!((columns[0], *columns.last().unwrap()), (plot.left, plot.right));
    assert_eq!((rows[0], *rows.last().unwrap()), (plot.top, plot.bottom));
    assert_eq!((plot.left, plot.right), (10.0, 410.0), "as wide as the progress bar");
    assert!(plot.top > 20.0 && plot.bottom < 72.0);
}
