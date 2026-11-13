use super::pinned_display_name;
use super::toolbar::{sidebar_button, sidebar_button_with_tooltip, sidebar_toolbar};
use crate::ui::components::tooltip::Tooltip;
use crate::ui::app::{ContextMenuContent, ContextMenuInstance, OxydeApp};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use crate::ui::workspace::SidebarItem;
use gpui::prelude::*;
use gpui::*;

/// Height of a rail entry: square cells that touch, as the bars' buttons, as wide
/// as the rail.
const RAIL_ROW: Rems = super::RAIL_WIDTH;

pub(super) fn render_sidebar_rail(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> Div {
    let theme = *cx.theme();
    let rail_hover = handle.clone();
    let rail = div()
        .size_full()
        .relative()
        .flex()
        .flex_col()
        .bg(theme.sidebar)
        .rounded_bl(view.window_corners.bottom_left)
        // Resting on another entry closes the pinned menu; its button stops the move.
        .on_mouse_move(move |_, window, cx| super::pinned::close_soon(&rail_hover, window, cx))
        .child(sidebar_toolbar(view, handle.clone(), false, cx));
    let h = handle.clone();
    let mut pinned = Some(
        div().w_full().child(
            sidebar_button(
                "sidebar-pinned".into(),
                cx.i18n().text(Message::SidebarQuickAccess),
                Some("icons/pin.svg"),
                None,
                theme,
                true,
                move |_, _, cx| {
                    crate::ui::components::popup::hover::cancel(cx);
                    h.update(cx, |this, cx| {
                        this.context_menus.clear();
                        // The click that follows a hover keeps the menu it opened.
                        let by_hover = std::mem::take(&mut this.pinned_rail_by_hover);
                        if by_hover && this.pinned_rail_open {
                            this.pinned_rail_kept = true;
                        } else {
                            this.pinned_rail_open = !this.pinned_rail_open;
                        }
                        cx.notify();
                    })
                },
            )
            .w_full()
            .h(RAIL_ROW)
            .relative()
            .on_mouse_move({
                let hover = handle.clone();
                move |_, window, cx| {
                    cx.stop_propagation();
                    if hover.read(cx).pinned_rail_open {
                        crate::ui::components::popup::hover::cancel(cx);
                        return;
                    }
                    let open = hover.clone();
                    crate::ui::components::popup::hover::intend("pinned-rail-open", window, cx, move |_, cx| {
                        open.update(cx, |this, cx| {
                            this.context_menus.clear();
                            this.pinned_rail_open = true;
                            this.pinned_rail_by_hover = true;
                            cx.notify();
                        });
                    });
                }
            })
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
        .pb_2();
    let current_path = view
        .state
        .layout
        .find_pane(&view.state.active_pane_id)
        .map(|pane| pane.active_tab().path.clone());
    // Volumes and Home stand for everything beneath them: only the deepest one that
    // holds the current folder lights, so the root does not light with every mount.
    let owner = current_path.as_deref().and_then(|current| {
        let roots = view.state.sidebar_items.iter().filter_map(|item| match item {
            SidebarItem::Drive { drive, .. } => Some(drive.path.as_str()),
            SidebarItem::Pinned { item } if item.name.is_empty() => Some(item.path.as_str()),
            _ => None,
        });
        deepest_root(current, roots).map(str::to_string)
    });
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
                drive_letter(&drive.path).is_none().then(|| crate::ui::components::icons::drive_icon_path(drive)),
                drive_letter(&drive.path),
            ),
            SidebarItem::OneDrive { item } => (item.path.clone(), item.name.clone(), Some("icons/cloudy.svg"), None),
            SidebarItem::Linux { .. } => (
                oxyde_lib::platform::WSL_ROOT.to_string(),
                cx.i18n().text(Message::SidebarLinux).to_string(),
                Some("icons/box.svg"),
                None,
            ),
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
        let is_home = matches!(item, SidebarItem::Pinned { item } if item.name.is_empty());
        // Places come first, as in the expanded tree: Home, then the pinned folders.
        if !is_home {
            if let Some(pinned) = pinned.take() {
                entries = entries.child(pinned);
            }
        }
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
        let selected = if is_drive || is_home {
            owner.as_deref().is_some_and(|owner| owner == path)
        } else {
            current_path.as_deref().is_some_and(|current| crate::ui::paths::paths_match(current, &path))
        };
        entries = entries.child(
            sidebar_button_with_tooltip(
                format!("rail-{path}"),
                tooltip,
                icon,
                text,
                theme,
                true,
                selected,
                move |_, _, cx| {
                    h.update(cx, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.navigate_to(&id, path.clone(), cx);
                    });
                },
            )
            .w_full()
            .h(RAIL_ROW)
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
                    SidebarItem::Linux { .. } => super::menus::linux_context_menu(cx.i18n(), menu.clone()),
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
                        open_left: false,
                        hovered_item: None,
                        native_request_id: None, open_up: false,
                    });
                    cx.notify();
                });
            })
            .when(is_drive && icon.is_none(), |d| {
                d.font_weight(FontWeight::BOLD).text_size(rems(0.75))
            }),
        );
    }
    if let Some(pinned) = pinned {
        entries = entries.child(pinned);
    }
    // Mark the fixed header edge only once entries have scrolled beneath it.
    let scrolled = crate::ui::components::scrollbar::scrolled_from_top(&view.sidebar_rail_scroll);
    // Leaving the rail closes the pinned menu too, unless the pointer reaches the menu.
    let leave = handle.clone();
    let rail = rail.child(div().id("sidebar-rail-hover").absolute().inset_0().on_hover(move |hovered, window, cx| {
        if !hovered {
            super::pinned::close_soon(&leave, window, cx);
        }
    }));
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

/// The deepest of `roots` that holds `path`, as a mount table resolves a file's volume.
fn deepest_root<'a>(path: &str, roots: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let path = std::path::Path::new(path);
    roots
        .into_iter()
        .filter(|root| crate::ui::paths::path_within(path, std::path::Path::new(root)))
        .max_by_key(|root| std::path::Path::new(root).components().count())
}

/// Windows drives show their letter; volumes without one show their type icon.
fn drive_letter(path: &str) -> Option<String> {
    let mut chars = path.chars();
    match (chars.next(), chars.next()) {
        (Some(letter), Some(':')) if letter.is_ascii_alphabetic() => Some(letter.to_ascii_uppercase().to_string()),
        _ => None,
    }
}

#[cfg(test)]
#[path = "tests/rail_tests.rs"]
mod tests;
