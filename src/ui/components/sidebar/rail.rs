use super::pinned_display_name;
use super::toolbar::{sidebar_button, sidebar_button_with_tooltip, sidebar_toolbar};
use crate::ui::components::tooltip::Tooltip;
use crate::ui::app::{ContextMenuContent, ContextMenuInstance, OxydeApp};
use crate::ui::i18n::{I18nContext, Message};
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
        .child(sidebar_toolbar(view, handle.clone(), false, cx));
    let h = handle.clone();
    rail = rail.child(
        div().px_2().py_2().child(
            sidebar_button(
                "sidebar-pinned".into(),
                cx.i18n().text(Message::SidebarQuickAccess),
                Some("icons/pin.svg"),
                None,
                theme,
                true,
                move |_, _, cx| {
                    h.update(cx, |this, cx| {
                        this.context_menus.clear();
                        this.pinned_rail_open = !this.pinned_rail_open;
                        cx.notify();
                    })
                },
            )
            .relative()
            .child(
                canvas(|bounds, _, _| bounds, {
                    let measure = handle.clone();
                    move |_, bounds: Bounds<Pixels>, _, cx| {
                        measure.update(cx, |this, _| {
                            this.context_anchor_points
                                .insert("sidebar-pinned".into(), bounds.origin);
                        })
                    }
                })
                .absolute()
                .left_0()
                .top_0()
                .size_full(),
            ),
        ),
    );
    let mut entries = div()
        .id("sidebar-rail-scroll")
        .track_scroll(&view.sidebar_rail_scroll)
        .size_full()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .items_center()
        .gap_2();
    for item in &view.state.sidebar_items {
        if !super::sidebar_item_visible(
            item,
            view.show_hidden,
            view.show_system,
            view.show_network(),
        ) {
            continue;
        }
        let (path, label, icon, text) = match item {
            SidebarItem::Drive { drive, .. } => (
                drive.path.clone(),
                crate::ui::format::drive_name(drive, cx.i18n()),
                None,
                Some(drive.path.chars().next().unwrap_or('/').to_string()),
            ),
            SidebarItem::OneDrive { item } => (item.path.clone(), item.name.clone(), Some("icons/cloudy.svg"), None),
            SidebarItem::Pinned { item }
                if item.path == "__network_vincinity__" || item.path == "trash://" || item.name.is_empty() =>
            {
                (
                    item.path.clone(),
                    pinned_display_name(item, cx.i18n()),
                    Some(if item.path == "trash://" {
                        "icons/trash-sidebar.svg"
                    } else if item.name.is_empty() {
                        "icons/house.svg"
                    } else {
                        "icons/globe.svg"
                    }),
                    None,
                )
            }
            _ => continue,
        };
        let h = handle.clone();
        let middle = handle.clone();
        let middle_path = path.clone();
        let menu = handle.clone();
        let menu_path = path.clone();
        let menu_item = item.clone();
        let is_drive = matches!(item, SidebarItem::Drive { .. });
        let tooltip: Box<dyn Fn(&mut Window, &mut App) -> AnyView> = match item {
            SidebarItem::Drive { drive, .. } => {
                Box::new(Tooltip::for_drive(label, drive.clone()))
            }
            _ => Box::new(Tooltip::text(label)),
        };
        let selected = view
            .state
            .layout
            .find_pane(&view.state.active_pane_id)
            .is_some_and(|pane| {
                let current_path = &pane.active_tab().path;
                if is_drive {
                    std::path::Path::new(current_path)
                        .ancestors()
                        .any(|ancestor| {
                            super::super::paths_match(&ancestor.to_string_lossy(), &path)
                        })
                } else {
                    super::super::paths_match(current_path, &path)
                }
            });
        entries = entries.child(
            sidebar_button_with_tooltip(
                format!("rail-{path}"),
                tooltip,
                icon,
                text,
                crate::ui::theme::OxydeTheme {
                    control_hover: if selected { theme.selection_hover } else { theme.control_hover },
                    text: if selected { theme.accent } else { theme.text },
                    ..theme
                },
                true,
                move |_, _, cx| {
                    h.update(cx, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.navigate_to(&id, path.clone(), cx);
                    });
                },
            )
            .on_mouse_down(MouseButton::Middle, move |_, _, cx| {
                cx.stop_propagation();
                middle.update(cx, |this, cx| {
                    this.pinned_rail_open = false;
                    this.context_menus.clear();
                    let pane = this.state.active_pane_id.clone();
                    this.add_tab(&pane, middle_path.clone(), false, cx);
                });
            })
            .on_mouse_down(MouseButton::Right, move |event, _, cx| {
                cx.stop_propagation();
                let items = match &menu_item {
                    SidebarItem::Drive { drive, .. } => super::menus::drive_context_menu(
                        cx.i18n(),
                        menu.clone(),
                        menu_path.clone(),
                        oxyde_lib::platform::platform().volumes().actions(drive),
                        drive.drive_type == "remote",
                    ),
                    _ => super::menus::location_context_menu(
                        cx.i18n(),
                        menu.clone(),
                        menu_path.clone(),
                    ),
                };
                let expand = cx.i18n().text(Message::ExpandAll);
                let collapse = cx.i18n().text(Message::CollapseAll);
                // Re-section after filtering so no empty section remains.
                let items = super::super::context_menu_sections(items
                    .into_iter()
                    .filter(|item| item.label != expand.as_ref() && item.label != collapse.as_ref())
                    .collect(), cx.i18n());
                menu.update(cx, |this, cx| {
                    this.pinned_rail_open = false;
                    this.context_menus.clear();
                    this.context_menus.push(ContextMenuInstance {
            native_bounds: None,
                        position: event.position,
                        content: ContextMenuContent::Items(items, None),
                        scroll_handle: UniformListScrollHandle::new(),
                        width: px(0.0),
                        bottom_padding: 0.0,
                        open_left: false,
                        hovered_item: None,
                        native_request_id: None, open_up: false,
                    });
                    cx.notify();
                });
            })
            .when(is_drive, |d| {
                d.font_weight(FontWeight::BOLD).text_size(rems(0.75))
            })
            .when(selected, |d| {
                d.bg(theme.selected)
                    .text_color(theme.accent)
            }),
        );
    }
    // Mark the fixed header edge only once entries have scrolled beneath it.
    let scrolled = crate::ui::components::scrollbar::scrolled_from_top(&view.sidebar_rail_scroll);
    rail.child(
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(entries)
            .when(scrolled, |d| {
                d.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .h(rems(0.0625))
                        .bg(theme.border),
                )
            }),
    )
}
