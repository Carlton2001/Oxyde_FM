use crate::ui::date_groups::{display_rows, row_height, DisplayRow};
use crate::ui::scale::rem_pixels;
use crate::ui::workspace::{TabState, ViewMode};
use gpui::*;

#[cfg(test)]
#[path = "scroll/tests/resize.rs"]
mod resize_tests;

impl TabState {
    pub(crate) fn keep_cursor_visible(&mut self, pane_size: Size<f32>) {
        let Some(path) = self
            .keyboard_cursor
            .as_ref()
            .or(self.selection_anchor.as_ref())
        else {
            return;
        };
        let Some(index) = self
            .filtered_files()
            .iter()
            .position(|file| &file.path == path)
        else {
            return;
        };
        let grid = self.view_mode == ViewMode::Grid;
        let columns = if grid {
            crate::ui::style::GridGeometry::for_viewport(
                pane_size.width,
                crate::ui::components::scrollbar::zed_track_width(),
            )
            .columns
        } else {
            1
        };
        let rows = display_rows(
            self.filtered_files(),
            self.grouping(),
            &self.collapsed_groups,
            columns,
        );
        let viewport = (pane_size.height
            - rem_pixels(crate::ui::style::TAB_BAR_HEIGHT).as_f32()
            - rem_pixels(crate::ui::style::NAV_BAR_HEIGHT).as_f32()
            - rem_pixels(rems(2.0)).as_f32()
            - if self.quick_search_query().is_some() {
                rem_pixels(rems(2.0)).as_f32()
            } else {
                0.0
            }
            - if grid {
                0.0
            } else {
                crate::ui::style::column_header_height()
                    + crate::ui::components::scrollbar::zed_track_width()
            })
        .max(1.0);
        let mut top = 0.0;
        for row in &rows {
            let height = row_height(row, grid);
            if matches!(row, DisplayRow::Files(indices) if indices.contains(&index)) {
                let scroll = &self.list_scroll.0.borrow().base_handle;
                let visible_top = -scroll.offset().y.as_f32();
                let target = if top < visible_top {
                    top
                } else if top + height > visible_top + viewport {
                    (top + height - viewport).max(0.0)
                } else {
                    visible_top
                };
                scroll.set_offset(point(scroll.offset().x, px(-target)));
                break;
            }
            top += height;
        }
    }
}
