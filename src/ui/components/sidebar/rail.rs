use super::pinned_display_name;
use super::toolbar::{sidebar_button, sidebar_toolbar};
use crate::ui::app::{ContextMenuContent, ContextMenuInstance, OxydeApp};
use crate::ui::components::dropdown::{DropdownItem, IconName};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::scale::rem_pixels;
use crate::ui::theme::ThemeContext;
use crate::ui::workspace::SidebarItem;
use gpui::prelude::*;
use gpui::*;

pub(super) fn render_sidebar_rail(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> Div {
    let theme = *cx.theme();
    let mut rail = div()
        .size_full()
        .flex()
        .flex_col()
        .bg(theme.sidebar)
        .child(sidebar_toolbar(view, handle.clone(), cx));
    let h = handle.clone();
    let pinned: Vec<_> = view
        .state
        .pinned_items
        .iter()
        .filter(|item| view.show_hidden || !item.name.starts_with('.'))
        .cloned()
        .collect();
    rail = rail.child(div().px_2().py_2().child(sidebar_button(
        "sidebar-pinned".into(),
        cx.i18n().text(Message::SidebarQuickAccess),
        Some("icons/pin.svg"),
        None,
        theme,
        true,
        move |e, _, cx| {
            let items = pinned
                .iter()
                .map(|item| {
                    let h = h.clone();
                    let path = item.path.clone();
                    DropdownItem {
                        label: pinned_display_name(item, cx.i18n()),
                        icon: Some(IconName::Pin),
                        badge: None,
                        stay_open: false,
                        has_submenu: false,
                        is_dimmed: false,
                        on_hover: None,
                        on_click: std::rc::Rc::new(move |_, cx| {
                            h.update(cx, |this, cx| {
                                this.context_menus.clear();
                                let id = this.state.active_pane_id.clone();
                                this.navigate_to(&id, path.clone(), cx);
                            });
                        }),
                    }
                })
                .collect();
            h.update(cx, |this, cx| {
                if !this.context_menus.is_empty() {
                    this.context_menus.clear();
                } else {
                    this.context_menus.push(ContextMenuInstance {
                        position: point(rem_pixels(rems(3.0)), e.position.y),
                        content: ContextMenuContent::Items(items, None),
                        scroll_handle: UniformListScrollHandle::new(),
                        width: px(0.0),
                        bottom_padding: 3.0,
                        open_left: false,
                        hovered_item: None,
                        native_request_id: None,
                    });
                }
                cx.notify();
            });
        },
    )));
    let mut entries = div()
        .id("sidebar-rail-scroll")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .items_center()
        .gap_2();
    for item in &view.state.sidebar_items {
        let (path, label, icon, text) = match item {
            SidebarItem::Drive { drive, .. } => (
                drive.path.clone(),
                crate::ui::format::drive_label(&drive.label, &drive.path, cx.i18n()),
                None,
                Some(drive.path.chars().next().unwrap_or('/').to_string()),
            ),
            SidebarItem::Pinned { item }
                if item.path == "__network_vincinity__" || item.path == "trash://" =>
            {
                (
                    item.path.clone(),
                    pinned_display_name(item, cx.i18n()),
                    Some(if item.path == "trash://" {
                        "icons/trash-sidebar.svg"
                    } else {
                        "icons/globe.svg"
                    }),
                    None,
                )
            }
            _ => continue,
        };
        let h = handle.clone();
        let is_drive = matches!(item, SidebarItem::Drive { .. });
        let selected = view
            .state
            .layout
            .find_pane(&view.state.active_pane_id)
            .is_some_and(|pane| {
                let current_path = &pane.active_tab().path;
                if is_drive {
                    std::path::Path::new(current_path).ancestors().any(|ancestor| {
                        super::super::paths_match(&ancestor.to_string_lossy(), &path)
                    })
                } else {
                    super::super::paths_match(current_path, &path)
                }
            });
        entries = entries.child(
            sidebar_button(
                format!("rail-{path}"),
                label.into(),
                icon,
                text,
                theme,
                true,
                move |_, _, cx| {
                    h.update(cx, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.navigate_to(&id, path.clone(), cx);
                    });
                },
            )
            .when(is_drive, |d| d.font_weight(FontWeight::BOLD).text_size(rems(0.75)))
            .when(selected, |d| {
                d.bg(theme.accent.opacity(0.1)).text_color(theme.text)
            }),
        );
    }
    rail.child(entries)
}
