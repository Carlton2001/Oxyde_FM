use crate::ui::app::{DragTarget, OxydeApp};
use crate::ui::scale::logical;

use crate::ui::workspace::ViewMode;
use gpui::*;

impl OxydeApp {
    pub fn handle_selection_drag(
        &mut self,
        pane_id: String,
        current_pos: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if let Some(pane) = self.state.find_pane_mut(&pane_id) {
            let tab = pane.active_tab_mut();
            let bounds = self.list_bounds.get(&pane_id).cloned().unwrap_or(Bounds {
                origin: Point { x: 0.0, y: 0.0 },
                size: Size {
                    width: logical(1000.0),
                    height: logical(800.0),
                },
            });
            let scroll_y = tab.list_scroll.0.borrow().base_handle.offset().y.as_f32();
            let grid_mode = tab.view_mode == ViewMode::Grid;
            let grid = crate::ui::style::GridGeometry::for_viewport(
                bounds.size.width,
                crate::ui::components::scrollbar::zed_track_width(),
            );
            let grid_width = grid.content_width;
            let grid_columns = grid.columns;
            let rows = crate::ui::date_groups::display_rows(tab.filtered_files(), tab.group_by_date, &tab.collapsed_date_groups, if grid_mode { grid_columns } else { 1 });

            // --- Auto-Scroll (Intuitive & Proportional) ---
            let list_top = bounds.origin.y;
            let list_bottom = bounds.origin.y + bounds.size.height;
            let mut new_scroll_y = scroll_y;
            let mut scrolled = false;

            if current_pos.y.as_f32() < list_top {
                let dist = (list_top - current_pos.y.as_f32()).max(1.0);
                let speed = (dist / logical(10.0)).powf(1.2).min(logical(40.0));
                new_scroll_y += speed;
            } else if current_pos.y.as_f32() > list_bottom {
                let dist = (current_pos.y.as_f32() - list_bottom).max(1.0);
                let speed = (dist / logical(10.0)).powf(1.2).min(logical(40.0));
                new_scroll_y -= speed;
            }

            if new_scroll_y != scroll_y {
                let visible_list_h = if grid_mode { bounds.size.height.max(1.0) }
                    else { (bounds.size.height - crate::ui::style::column_header_height()).max(1.0) };
                let content_h = crate::ui::date_groups::content_height(&rows, grid_mode).max(1.0);                let max_scroll = (content_h - visible_list_h).max(0.0);

                new_scroll_y = new_scroll_y.clamp(-max_scroll, 0.0);
                if new_scroll_y != scroll_y {
                    tab.list_scroll
                        .0
                        .borrow()
                        .base_handle
                        .set_offset(Point::new(px(0.0), px(new_scroll_y)));
                    scrolled = true;
                }
            }
            let scroll_y = new_scroll_y;

            // --- Selection Logic ---
            if let Some(DragTarget::SelectionRectangle {
                start_pos,
                modifiers,
                initial_selection,
                ..
            }) = &self.drag_target
            {
                let start_pos = *start_pos;
                let modifiers = *modifiers;
                let initial_selection_cap = initial_selection.clone();

                let files = tab.filtered_files();
                let rect_left = start_pos.x.as_f32().min(current_pos.x.as_f32());
                let rect_right = start_pos.x.as_f32().max(current_pos.x.as_f32());
                let rect_top = start_pos.y.as_f32().min(current_pos.y.as_f32());
                let rect_bottom = start_pos.y.as_f32().max(current_pos.y.as_f32());
                let card_width = grid_width / grid_columns as f32;
                let mut top = bounds.origin.y + scroll_y + if grid_mode { 0.0 } else { crate::ui::style::column_header_height() };
                let mut current_rect_paths = std::collections::HashSet::new();
                for row in &rows {
                    let height = crate::ui::date_groups::row_height(row, grid_mode);
                    if let crate::ui::date_groups::DisplayRow::Files(indices) = row {
                        for (column, index) in indices.iter().enumerate() {
                            let left = bounds.origin.x + logical(8.0) + column as f32 * card_width;
                            let right = left + card_width - logical(8.0);
                            let bottom = top + height - if grid_mode { logical(8.0) } else { 0.0 };
                            if top < rect_bottom && bottom > rect_top && (!grid_mode || (left < rect_right && right > rect_left)) {
                                current_rect_paths.insert(files[*index].path.clone());
                            }
                        }
                    }
                    top += height;
                }
                if modifiers.control {
                    tab.selected_paths.clear();
                    for path in initial_selection_cap.iter() {
                        if !current_rect_paths.contains(path) {
                            tab.selected_paths.insert(path.clone());
                        }
                    }
                    for path in current_rect_paths.iter() {
                        if !initial_selection_cap.contains(path) {
                            tab.selected_paths.insert(path.clone());
                        }
                    }
                } else if modifiers.shift {
                    for path in initial_selection_cap.iter() {
                        tab.selected_paths.insert(path.clone());
                    }
                    for path in current_rect_paths.iter() {
                        tab.selected_paths.insert(path.clone());
                    }
                } else {
                    tab.selected_paths = current_rect_paths;
                    tab.selection_anchor = None;
                }
            }

            if scrolled {
                cx.notify();
            }
        }
    }

    pub fn toggle_selection(
        &mut self,
        pane_id: &str,
        path: String,
        multi: bool,
        shift: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            if shift {
                let rows = crate::ui::date_groups::display_rows(tab.filtered_files(), tab.group_by_date, &tab.collapsed_date_groups, 1);
                let filtered: Vec<_> = crate::ui::date_groups::visible_indices(&rows).into_iter().map(|index| &tab.filtered_files()[index]).collect();
                let anchor_idx = tab
                    .selection_anchor
                    .as_ref()
                    .and_then(|a| filtered.iter().position(|f| &f.path == a));
                let target_idx = filtered.iter().position(|f| f.path == path);

                if let (Some(start_idx), Some(end_idx)) = (anchor_idx, target_idx) {
                    let range_start = start_idx.min(end_idx);
                    let range_end = start_idx.max(end_idx);

                    let paths_to_add: Vec<String> = filtered[range_start..=range_end]
                        .iter()
                        .map(|f| f.path.clone())
                        .collect();

                    if !multi {
                        tab.selected_paths.clear();
                    }
                    for p in paths_to_add {
                        tab.selected_paths.insert(p);
                    }
                } else {
                    if !multi {
                        tab.selected_paths.clear();
                    }
                    tab.selected_paths.insert(path.clone());
                    tab.selection_anchor = Some(path);
                }
            } else if multi {
                if tab.selected_paths.contains(&path) {
                    tab.selected_paths.remove(&path);
                } else {
                    tab.selected_paths.insert(path.clone());
                    tab.selection_anchor = Some(path);
                }
            } else {
                tab.selected_paths.clear();
                tab.selected_paths.insert(path.clone());
                tab.selection_anchor = Some(path);
            }
            cx.notify();
        }
    }

    pub fn select_all(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            tab.selected_paths.clear();
            let all_paths: Vec<String> = tab
                .filtered_files()
                .iter()
                .map(|f| f.path.clone())
                .collect();
            for path in all_paths {
                tab.selected_paths.insert(path);
            }
            cx.notify();
        }
    }
}
