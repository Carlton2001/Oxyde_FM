use crate::ui::app::OxydeApp;
use crate::ui::scale::logical;
use gpui::*;

impl OxydeApp {
    pub fn clamp_scrolls(&mut self) {
        // Clamp Sidebar horizontal scroll
        let sidebar_viewport_w = self.sidebar_display_width();
        let sidebar_content_w = self.sidebar_content_width;
        let sidebar_max_offset_x = (sidebar_content_w - sidebar_viewport_w).max(0.0);
        let current_sidebar_h_offset = self.sidebar_scroll_h.offset().x.as_f32().abs();

        if current_sidebar_h_offset > sidebar_max_offset_x {
            let new_x = (-sidebar_max_offset_x).min(0.0);
            self.sidebar_scroll_h
                .set_offset(Point::new(px(new_x), self.sidebar_scroll_h.offset().y));
        }

        // Clamp FileList horizontal scrolls for all panes
        let p_sizes = &self.pane_sizes;
        let l_w = self.list_width;
        let l_h = self.list_height;

        self.state.layout.visit_panes_mut(&mut |pane| {
            let pane_id = pane.id.clone();
            let active_tab = pane.active_tab_mut();
            let pane_size = p_sizes.get(&pane_id).cloned().unwrap_or(Size {
                width: l_w,
                height: l_h,
            });

            let columns = crate::ui::style::DetailsColumnWidths::for_viewport_with_search(
                pane_size.width - if active_tab.path == "trash://" { logical(active_tab.deleted_col_width) } else { 0.0 },
                active_tab.type_col_width,
                active_tab.size_col_width,
                active_tab.date_col_width,
                active_tab.quick_search.is_some() || active_tab.path == "trash://",
            );
            let total_w = columns.content_width() + if active_tab.path == "trash://" { logical(active_tab.deleted_col_width) } else { 0.0 };

            let max_h_offset = (total_w - pane_size.width).max(0.0);
            let current_h_offset = active_tab.list_scroll_h.offset().x.as_f32().abs();

            if current_h_offset > max_h_offset {
                let new_x = (-max_h_offset).min(0.0);
                active_tab
                    .list_scroll_h
                    .set_offset(Point::new(px(new_x), active_tab.list_scroll_h.offset().y));
            }
        });

        // Clamp Breadcrumb scroll
        if let Some((_, _, _, dirs)) = &self.breadcrumb_menu {
            let item_count = dirs.len();
            let content_h = item_count as f32 * logical(28.0);
            let viewport_h = content_h.min(logical(360.0));
            let max_v_offset = (content_h - viewport_h).max(0.0);
            let current_v_offset = self
                .breadcrumb_scroll
                .0
                .borrow()
                .base_handle
                .offset()
                .y
                .as_f32()
                .abs();
            if current_v_offset > max_v_offset {
                self.breadcrumb_scroll
                    .0
                    .borrow()
                    .base_handle
                    .set_offset(Point::new(px(0.0), px(-max_v_offset)));
            }
        } else {
            self.breadcrumb_scroll
                .0
                .borrow()
                .base_handle
                .set_offset(Point::new(px(0.0), px(0.0)));
        }

        // Clamp ContextMenu scrolls
        for instance in &self.context_menus {
            let item_count = match &instance.content {
                crate::ui::app::ContextMenuContent::Items(items, _) => items.iter().filter(|item| !matches!(item.icon, Some(crate::ui::components::dropdown::IconName::Separator))).count(),
                crate::ui::app::ContextMenuContent::NativeItems { items, .. } => items.len(),
                crate::ui::app::ContextMenuContent::NameFilter { .. } => 0,
            };
            if item_count > 0 {
                let content_h = item_count as f32 * logical(28.0);
                let viewport_h = content_h.min(logical(480.0));
                let max_v_offset = (content_h - viewport_h).max(0.0);
                let current_v_offset = instance
                    .scroll_handle
                    .0
                    .borrow()
                    .base_handle
                    .offset()
                    .y
                    .as_f32()
                    .abs();
                if current_v_offset > max_v_offset {
                    instance
                        .scroll_handle
                        .0
                        .borrow()
                        .base_handle
                        .set_offset(Point::new(px(0.0), px(-max_v_offset)));
                }
            } else {
                instance
                    .scroll_handle
                    .0
                    .borrow()
                    .base_handle
                    .set_offset(Point::new(px(0.0), px(0.0)));
            }
        }
    }

}
