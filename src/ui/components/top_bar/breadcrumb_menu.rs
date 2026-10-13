//! Breadcrumb choices are anchored to their chevron, not to the pointer.
use crate::ui::app::{DragTarget, OxydeApp};
use crate::ui::components::dropdown::{DropdownItem, GenericDropdown, IconName};
use crate::ui::scale::logical;
use gpui::*;
use std::rc::Rc;

pub(super) fn render(view: &OxydeApp, handle: Entity<OxydeApp>, pane: &str, path: &str, index: usize, _: &mut Window, _: &mut Context<OxydeApp>) -> AnyElement {
    let Some((current_pane, current_path, current_index, entries)) = &view.breadcrumb_menu else { return div().into_any_element(); };
    if current_pane != pane || current_path != path || *current_index != index { return div().into_any_element(); }
    let items = entries.iter().map(|entry| {
        let choose = handle.clone(); let pane = pane.to_owned(); let path = entry.path.clone();
        DropdownItem {
            label: entry.name.clone(), badge: None,
            icon: Some(if view.use_system_icons() { IconName::NativeFolder(entry.path.clone()) } else { IconName::FolderOpen }),
            stay_open: false, has_submenu: false, is_dimmed: entry.is_hidden || entry.is_system,
            on_click: Rc::new(move |_, cx| choose.update(cx, |view, cx| { view.navigate_to(&pane, path.clone(), cx); view.breadcrumb_menu = None; cx.notify(); })),
            on_hover: None,
        }
    }).collect();
    let close = handle.clone();
    GenericDropdown {
        id: "breadcrumb-dropdown".into(), h_app: handle, items, reserve_icon_space: true, is_context_menu: false, bottom_padding: 0.0,
        scroll_handle: view.breadcrumb_scroll.clone(), width: None, max_height: logical(360.0), header: None,
        on_close: Rc::new(move |_, cx| close.update(cx, |view, cx| { view.breadcrumb_menu = None; cx.notify(); })),
        on_outside: None, on_drag: Rc::new(|view, track_id, viewport_h, content_h, _| { view.drag_target = Some(DragTarget::BreadcrumbV { track_id, viewport_h, content_h }); }),
        is_dragging: matches!(view.drag_target, Some(DragTarget::BreadcrumbV { .. })),
        on_item_hover: None, on_width_calculated: None, on_hover_change: None,
    }.into_any_element()
}
