#[cfg(test)]
use crate::ui::date_groups::{row_height, DisplayRow};
use std::ops::Range;

pub(super) struct VisibleGridRows {
    pub(super) range: Range<usize>,
    pub(super) top: f32,
    pub(super) bottom: f32,
}

/// `row` is the height of a line of cards, which ungrouped rows all have.
pub(super) fn visible_prefix(heights: &[f32], grouped: bool, row: f32, offset: f32, viewport: f32) -> VisibleGridRows {
    let count = heights.len().saturating_sub(1);
    if !grouped {
        let start = ((offset.max(0.0) / row).floor() as usize).min(count);
        let end = (((offset + viewport + row) / row).ceil() as usize).min(count).max(start);
        return VisibleGridRows { range: start..end, top: start as f32 * row, bottom: end as f32 * row };
    }
    let start = heights[1..].partition_point(|height| *height < offset).min(count);
    let end = heights[..count].partition_point(|height| *height < offset + viewport + row).min(count).max(start);
    VisibleGridRows { range: start..end, top: heights[start], bottom: heights[end] }
}

/// Keep uniform grids O(1); grouped grids retain their variable header heights.
#[cfg(test)]
pub(super) fn visible_rows(
    rows: &[DisplayRow],
    layout: crate::ui::style::CardLayout,
    grouped: bool,
    offset: f32,
    viewport: f32,
) -> VisibleGridRows {
    let overscan = layout.row_height();
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
    while start < rows.len() && top + row_height(&rows[start], Some(layout)) < offset {
        top += row_height(&rows[start], Some(layout));
        start += 1;
    }
    let mut end = start;
    let mut bottom = top;
    while end < rows.len() && bottom < offset + viewport + overscan {
        bottom += row_height(&rows[end], Some(layout));
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
