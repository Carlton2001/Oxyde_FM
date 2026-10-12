use crate::ui::date_groups::{row_height, DisplayRow};
use std::ops::Range;

pub(super) struct VisibleGridRows {
    pub(super) range: Range<usize>,
    pub(super) top: f32,
    pub(super) bottom: f32,
}

/// Keep uniform grids O(1); grouped grids retain their variable header heights.
pub(super) fn visible_rows(
    rows: &[DisplayRow],
    grouped: bool,
    offset: f32,
    viewport: f32,
) -> VisibleGridRows {
    let overscan = crate::ui::style::grid_card_row_height();
    if !grouped {
        let start = ((offset.max(0.0) / overscan).floor() as usize).min(rows.len());
        let end = (((offset + viewport + overscan) / overscan).ceil() as usize)
            .min(rows.len())
            .max(start);
        return VisibleGridRows {
            range: start..end,
            top: start as f32 * overscan,
            bottom: end as f32 * overscan,
        };
    }
    let mut start = 0;
    let mut top = 0.0;
    while start < rows.len() && top + row_height(&rows[start], true) < offset {
        top += row_height(&rows[start], true);
        start += 1;
    }
    let mut end = start;
    let mut bottom = top;
    while end < rows.len() && bottom < offset + viewport + overscan {
        bottom += row_height(&rows[end], true);
        end += 1;
    }
    VisibleGridRows {
        range: start..end,
        top,
        bottom,
    }
}

#[cfg(test)]
#[path = "grid_scroll/tests/geometry.rs"]
mod tests;
