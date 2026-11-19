//! The locations a right click on Back or Forward offers, nearest first.
use crate::ui::app::{ContextMenuContent, ContextMenuInstance, OxydeApp};
use crate::ui::components::dropdown::{DropdownItem, IconName};
use crate::ui::i18n::{I18n, I18nContext};
use gpui::*;
use std::rc::Rc;

/// Opens the menu of the active tab's back (`forward == false`) or forward history.
pub(super) fn open(view: &mut OxydeApp, handle: Entity<OxydeApp>, pane_id: &str, forward: bool, position: Point<Pixels>, cx: &mut Context<OxydeApp>) {
    let Some(pane) = view.state.find_pane(pane_id) else { return };
    let tab = pane.active_tab();
    let stack = if forward { &tab.forward_stack } else { &tab.back_stack };
    if stack.is_empty() { return; }
    let system_icons = view.use_system_icons();
    let items = stack.iter().rev().enumerate().map(|(index, path)| {
        let (h, pane_id) = (handle.clone(), pane_id.to_owned());
        DropdownItem {
            label: location_label(path, cx.i18n()),
            icon: Some(if system_icons && !is_special(path) { IconName::NativeFolder(path.clone()) } else { IconName::Folder }),
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            is_faded: false,
            on_click: Rc::new(move |_, app| {
                h.update(app, |this, cx| {
                    this.context_menus.clear();
                    this.go_through_history(&pane_id, forward, index + 1, cx);
                });
            }),
            on_hover: None,
        }
    }).collect();
    view.context_menus.clear();
    view.context_menus.push(ContextMenuInstance {
        native_bounds: None,
        position,
        content: ContextMenuContent::Items(items),
        scroll_handle: gpui::UniformListScrollHandle::new(),
        width: px(0.0),
        open_left: false,
        hovered_item: None,
        native_request_id: None,
        open_up: false,
    });
    cx.notify();
}

fn is_special(path: &str) -> bool {
    path == "trash://" || path.starts_with("trash:\\\\") || path == "__network_vincinity__"
        || path.starts_with("search://") || path.starts_with("::{")
}

/// The name the navigation bar gives a location.
fn location_label(path: &str, i18n: &I18n) -> String {
    super::special_location_label(path, i18n).unwrap_or_else(|| path.to_owned())
}
