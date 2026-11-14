mod items;
mod native;
mod column_filter;

use crate::ui::app::{ContextMenuContent, DragTarget, OxydeApp};
use gpui::prelude::*;
use gpui::*;

impl OxydeApp {
    pub(super) fn render_context_menus(
        &self,
        root: Div,
        handle: Entity<Self>,
        window_handle: AnyWindowHandle,
        _win_h: f32,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        if crate::ui::components::popup::supported(_window, cx) {
            for (index, instance) in self.context_menus.iter().enumerate() {
                let parent = window_handle;
                let scroll = instance.scroll_handle.clone();
                let alive_scroll = scroll.clone();
                let anchor = index.checked_sub(1).and_then(|parent_index| {
                    let parent = &self.context_menus[parent_index];
                    let row = crate::ui::components::popup::row_bounds(window_handle, format!("context-menu-{parent_index}"), parent.hovered_item?, cx)?;
                    let bounds = parent.native_bounds.unwrap_or(row);
                    Some(crate::ui::components::popup::PopupAnchor::beside(Bounds::new(point(bounds.left(), row.top()), size(bounds.size.width, row.size.height)), false))
                }).unwrap_or_else(|| crate::ui::components::popup::PopupAnchor::point(instance.position, instance.open_left).above(instance.open_up))
                    .parent(if let Some(parent_index) = index.checked_sub(1) {
                        crate::ui::components::popup::surface(window_handle, format!("context-menu-{parent_index}"), cx)
                    } else if self.sidebar_collapsed && self.pinned_rail_open {
                        crate::ui::components::popup::surface(window_handle, "pinned-rail", cx)
                    } else {
                        None
                    });
                crate::ui::components::popup::show(
                    format!("context-menu-{index}"),
                    anchor,
                    handle.clone(), _window, cx,
                    move |view| view.context_menus.get(index).is_some_and(|menu| std::rc::Rc::ptr_eq(&menu.scroll_handle.0, &alive_scroll.0)),
                    move |view, window, cx| {
                        if let Some(instance) = view.context_menus.get_mut(index) {
                            instance.native_bounds = Some(crate::ui::components::popup::body_bounds(window, cx));
                        }
                        let Some(instance) = view.context_menus.get(index) else { return div().into_any_element(); };
                        let h = cx.entity();
                        let height = crate::ui::components::popup::available_size(window, cx).height.as_f32();
                        let dragging = matches!(view.drag_target, Some(DragTarget::ContextMenuV { menu_idx, .. }) if menu_idx == index);
                        let content = match &instance.content {
                            ContextMenuContent::Items(items) => view.render_items_menu(items, index, h, scroll.clone(), dragging, parent, height),
                            ContextMenuContent::NativeItems { path, is_background, items } => view.render_native_menu(path, is_background, items, index, h, scroll.clone(), dragging, parent, height),
                            ContextMenuContent::ColumnFilter(panel) => view.render_column_filter(panel, index, h, scroll.clone(), dragging, height, window, cx),
                        };
                        content.into_any_element()
                    },
                    move |view, _, cx| { view.note_menu_dismissed(index); view.context_menus.truncate(index); view.submenu_timer = None; view.submenu_close_timer = None; cx.notify(); },
                );
            }
            return root;
        }
        {
        if self.context_menus.iter().any(|menu| menu.native_bounds.is_some()) {
            let source = handle.clone();
            cx.defer(move |cx| source.update(cx, |view, cx| { for menu in &mut view.context_menus { menu.native_bounds = None; } cx.notify(); }));
        }
        let mut root = root;
        let win_h = _win_h;
        // CONTEXT MENU OVERLAY
        for (m_idx, instance) in self.context_menus.iter().enumerate() {

            let h = handle.clone();
            let pos = instance.position;
            let scroll_handle = instance.scroll_handle.clone();
            let is_v_dragging = matches!(self.drag_target, Some(DragTarget::ContextMenuV { menu_idx, .. }) if menu_idx == m_idx);
            let _menu_width = instance.width;

            let content_element = match &instance.content {
                ContextMenuContent::Items(items) => self.render_items_menu(
                    items, m_idx, h, scroll_handle, is_v_dragging, window_handle, win_h,
                ),
                ContextMenuContent::NativeItems { path, is_background, items } => self.render_native_menu(
                    path, is_background, items, m_idx, h, scroll_handle, is_v_dragging, window_handle, win_h,
                ),
                ContextMenuContent::ColumnFilter(panel) => self.render_column_filter(
                    panel, m_idx, h, scroll_handle, is_v_dragging, win_h, _window, cx,
                ),
            };

            root = root.child(deferred(if instance.open_up {
                anchored()
                    .position(pos)
                    .anchor(Anchor::BottomLeft)
                    .snap_to_window()
                    .child(content_element)
            } else if instance.open_left {
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
}
