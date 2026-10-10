mod items;
mod native;
mod name_filter;

use crate::ui::app::{ContextMenuContent, DragTarget, OxydeApp};
use gpui::prelude::*;
use gpui::*;

impl OxydeApp {
    pub(super) fn render_context_menus(
        &self,
        mut root: Div,
        handle: Entity<Self>,
        window_handle: AnyWindowHandle,
        win_h: f32,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        // CONTEXT MENU OVERLAY
        for (m_idx, instance) in self.context_menus.iter().enumerate() {

            let h = handle.clone();
            let pos = instance.position;
            let scroll_handle = instance.scroll_handle.clone();
            let is_v_dragging = matches!(self.drag_target, Some(DragTarget::ContextMenuV { menu_idx, .. }) if menu_idx == m_idx);
            let _menu_width = instance.width;

            let content_element = match &instance.content {
                ContextMenuContent::Items(items, header_type) => self.render_items_menu(
                    items, header_type, instance, m_idx, h, pos, scroll_handle, is_v_dragging, window_handle, win_h, cx,
                ),
                ContextMenuContent::NativeItems { path, is_background, items } => self.render_native_menu(
                    path, is_background, items, instance, m_idx, h, scroll_handle, is_v_dragging, window_handle, win_h,
                ),
                ContextMenuContent::NameFilter { input } => self.render_name_filter_menu(
                    input, instance, m_idx, h, scroll_handle, win_h, _window, cx,
                ),
            };

            root = root.child(deferred(if instance.open_left {
                // Snap (no corner-switch) so the submenu extends left from parent_left
                anchored()
                    .position(pos)
                    .anchor(Anchor::TopRight)
                    .snap_to_window()
                    .child(content_element)
            } else {
                anchored()
                    .position(pos)
                    .snap_to_window()
                    .child(content_element)
            }));
        }

        root
    }
}
