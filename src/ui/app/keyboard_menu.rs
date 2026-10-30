//! The Menu key and Shift+F10 open at the selection, where a right click on it would.
use super::selection::scroll_row_into_view;
use super::{ContextMenuContent, ContextMenuInstance, OxydeApp};
use crate::ui::components::{oxyde_context_menu_items, pane_context_menu_items, paths_match, with_grid_sort};
use crate::ui::date_groups::{display_rows, row_height, DisplayRow};
use crate::ui::i18n::I18nContext;
use crate::ui::scale::logical;
use crate::ui::style::GridGeometry;
use crate::ui::workspace::ViewMode;
use gpui::*;

/// The top, height and column of the row holding a file.
fn file_place(rows: &[DisplayRow], index: usize, grid: bool) -> Option<(f32, f32, usize)> {
    let mut top = 0.0;
    for row in rows {
        let height = row_height(row, grid);
        if let DisplayRow::Files(indices) = row {
            if let Some(column) = indices.iter().position(|file| *file == index) {
                return Some((top, height, column));
            }
        }
        top += height;
    }
    None
}

impl OxydeApp {
    pub(super) fn open_keyboard_context_menu(&mut self, cx: &mut Context<Self>) {
        let Some(handle) = self.handle.clone() else { return };
        let pane_id = self.state.active_pane_id.clone();
        let width = self.list_bounds.get(&pane_id).map_or(self.list_width, |bounds| bounds.size.width);
        let Some(pane) = self.state.find_pane(&pane_id) else { return };
        let tab = pane.active_tab();
        let grid = tab.view_mode == ViewMode::Grid;
        let geometry = GridGeometry::for_viewport(width, crate::ui::components::scrollbar::zed_track_width());
        let rows = display_rows(tab.filtered_files(), tab.grouping(), &tab.collapsed_groups, if grid { geometry.columns } else { 1 });
        let target = tab.keyboard_cursor.as_ref().filter(|path| tab.selected_paths.contains(*path))
            .or(tab.selection_anchor.as_ref().filter(|path| tab.selected_paths.contains(*path)))
            .and_then(|path| tab.filtered_files().iter().position(|file| &file.path == path))
            .or_else(|| rows.iter().find_map(|row| match row {
                DisplayRow::Files(indices) => indices.iter().copied()
                    .find(|index| tab.selected_paths.contains(&tab.filtered_files()[*index].path)),
                _ => None,
            }));
        let scroll = tab.list_scroll.0.borrow().base_handle.clone();
        let place = target.and_then(|index| file_place(&rows, index, grid));
        if let Some(index) = target {
            scroll_row_into_view(tab, &rows, index);
        }
        let bounds = scroll.bounds();
        let inset = px(logical(32.0));
        let position = place.map_or(bounds.origin + point(inset, px(logical(8.0))), |(top, height, column)| {
            let cell = if grid { column as f32 * geometry.cell_width } else { 0.0 };
            let below = top + if grid { height / 2.0 } else { height };
            point(
                bounds.left() + px(cell) + inset,
                (bounds.top() + scroll.offset().y + px(below)).min(bounds.bottom()),
            )
        });
        let file = target.map(|index| tab.filtered_files()[index].clone());
        let items = match file {
            Some(file) => {
                let pinned = self.state.pinned_items.iter().any(|item| paths_match(&item.path, &file.path));
                oxyde_context_menu_items(cx.i18n(), handle.clone(), file.is_dir, pinned, file.path, self)
            }
            None => pane_context_menu_items(cx.i18n(), handle.clone(), pane_id.clone(), self),
        };
        let items = with_grid_sort(items, self, handle, &pane_id, cx.i18n());
        self.context_menus.clear();
        self.context_menus.push(ContextMenuInstance {
            native_bounds: None,
            position,
            content: ContextMenuContent::Items(items, None),
            scroll_handle: UniformListScrollHandle::new(),
            width: px(0.0),
            open_left: false,
            hovered_item: None,
            native_request_id: None,
            open_up: false,
        });
        cx.notify();
    }
}

#[cfg(test)]
#[path = "keyboard_menu/tests/place.rs"]
mod place_tests;
