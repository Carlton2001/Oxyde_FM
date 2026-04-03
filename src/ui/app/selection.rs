use gpui::*;
use crate::ui::app::{OxydeApp, DragTarget};
use crate::ui::theme::PixelsExt;
use crate::ui::style::ROW_HEIGHT;

impl OxydeApp {
    pub fn handle_selection_drag(&mut self, pane_id: String, current_pos: Point<Pixels>, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(&pane_id) {
            let tab = pane.active_tab_mut();
            let bounds = self.list_bounds.get(&pane_id).cloned().unwrap_or(Bounds { origin: Point { x: 0.0, y: 0.0 }, size: Size { width: 1000.0, height: 800.0 } });
            let scroll_y = tab.list_scroll.0.borrow().base_handle.offset().y.as_f32();
            
            // --- Auto-Scroll (Intuitive & Proportional) ---
            let list_top = bounds.origin.y;
            let list_bottom = bounds.origin.y + bounds.size.height;
            let mut new_scroll_y = scroll_y;
            let mut scrolled = false;

            if current_pos.y.as_f32() < list_top {
                let dist = (list_top - current_pos.y.as_f32()).max(1.0);
                let speed = (dist / 10.0).powf(1.2).min(40.0); 
                new_scroll_y += speed;
            } else if current_pos.y.as_f32() > list_bottom {
                let dist = (current_pos.y.as_f32() - list_bottom).max(1.0);
                let speed = (dist / 10.0).powf(1.2).min(40.0);
                new_scroll_y -= speed;
            }
            
            if new_scroll_y != scroll_y {
                let count = tab.filtered_files().len() as f32;
                let visible_list_h = (bounds.size.height - ROW_HEIGHT).max(1.0);
                let content_h = (count * ROW_HEIGHT).max(1.0);
                let max_scroll = (content_h - visible_list_h).max(0.0);
                
                new_scroll_y = new_scroll_y.clamp(-max_scroll, 0.0);
                if new_scroll_y != scroll_y {
                    tab.list_scroll.0.borrow().base_handle.set_offset(Point::new(px(0.0), px(new_scroll_y)));
                    scrolled = true;
                }
            }
            let scroll_y = new_scroll_y;

            // --- Selection Logic ---
            if let Some(DragTarget::SelectionRectangle { start_pos, modifiers, initial_selection, .. }) = &self.drag_target {
                let start_pos = *start_pos;
                let modifiers = *modifiers;
                let initial_selection_cap = initial_selection.clone();

                let rect_y_start = start_pos.y.as_f32().min(current_pos.y.as_f32());
                let rect_y_end = start_pos.y.as_f32().max(current_pos.y.as_f32());
                
                let files = tab.filtered_files();
                let list_origin_y = bounds.origin.y + ROW_HEIGHT;
                
                let start_idx_f = (rect_y_start - list_origin_y - scroll_y) / ROW_HEIGHT;
                let end_idx_f = (rect_y_end - list_origin_y - scroll_y) / ROW_HEIGHT;
                
                let start_idx = (start_idx_f.floor() as i32).max(0) as usize;
                let end_idx = (end_idx_f.ceil() as i32).max(0) as usize;
                
                let files_len = files.len();
                let start_idx = start_idx.min(files_len);
                let end_idx = end_idx.min(files_len);
                
                let current_rect_paths: std::collections::HashSet<String> = if start_idx < end_idx {
                    files[start_idx..end_idx].iter().map(|f| f.path.clone()).collect()
                } else {
                    std::collections::HashSet::new()
                };

                if modifiers.control {
                    tab.selected_paths.clear();
                    for path in initial_selection_cap.iter() {
                        if !current_rect_paths.contains(path) { tab.selected_paths.insert(path.clone()); }
                    }
                    for path in current_rect_paths.iter() {
                        if !initial_selection_cap.contains(path) { tab.selected_paths.insert(path.clone()); }
                    }
                } else if modifiers.shift {
                    for path in initial_selection_cap.iter() { tab.selected_paths.insert(path.clone()); }
                    for path in current_rect_paths.iter() { tab.selected_paths.insert(path.clone()); }
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

    pub fn toggle_selection(&mut self, pane_id: &str, path: String, multi: bool, shift: bool, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            if shift {
                let filtered = tab.filtered_files();
                let anchor_idx = tab.selection_anchor.as_ref()
                    .and_then(|a| filtered.iter().position(|f| &f.path == a));
                let target_idx = filtered.iter().position(|f| f.path == path);

                if let (Some(start_idx), Some(end_idx)) = (anchor_idx, target_idx) {
                    let range_start = start_idx.min(end_idx);
                    let range_end = start_idx.max(end_idx);
                    
                    let paths_to_add: Vec<String> = filtered[range_start..=range_end].iter().map(|f| f.path.clone()).collect();

                    if !multi {
                        tab.selected_paths.clear();
                    }
                    for p in paths_to_add {
                        tab.selected_paths.insert(p);
                    }
                } else {
                    if !multi { tab.selected_paths.clear(); }
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
}
