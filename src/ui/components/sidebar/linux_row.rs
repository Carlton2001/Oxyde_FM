use super::menus::linux_context_menu;
use super::{sidebar_paths_equal, sidebar_row_h};
use crate::ui::app::{ContextMenuContent, ContextMenuInstance, OxydeApp};
use crate::ui::components::tooltip::hover::{HoverRegion, SharedTooltipExt};
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::platform::WSL_ROOT;

/// Stays off the distributions themselves: only their rows can start one.
pub(super) fn render(
    is_expanded: bool,
    row_w: f32,
    show_tooltips: bool,
    selected: Option<&str>,
    h: Entity<OxydeApp>,
    theme: OxydeTheme,
    cx: &mut App,
) -> AnyElement {
    let is_sel = selected.is_some_and(|path| sidebar_paths_equal(path, WSL_ROOT));
    let hov_bg = if is_sel { theme.selection_hover } else { theme.hover };
    let name = cx.i18n().text(Message::SidebarLinux).to_string();
    let toggle = |h: &Entity<OxydeApp>, cx: &mut App| {
        h.update(cx, |this, cx| this.toggle_sidebar_path(WSL_ROOT.to_string(), cx));
    };
    div()
        .id("sb-linux")
        .w(px(row_w))
        .h(px(sidebar_row_h()))
        .px_3()
        .flex()
        .items_center()
        .relative()
        .when(is_sel, |d| d.bg(theme.selected).child(div().absolute().left_0().top_0().bottom_0().w(rems(0.1875)).bg(theme.accent)))
        .hover(move |s| s.bg(hov_bg))
        .when(show_tooltips, |d| d.shared_tooltip(h.clone(), HoverRegion::DirectoryTree, "linux".to_string(), cx, Tooltip::text(name.clone())))
        .on_mouse_down(MouseButton::Left, {
            let h = h.clone();
            move |event, _, cx| {
                if event.click_count == 2 {
                    toggle(&h, cx);
                    return;
                }
                h.update(cx, |this, cx| {
                    this.sidebar_selected_path = Some(WSL_ROOT.to_string());
                    let id = this.state.active_pane_id.clone();
                    this.navigate_to(&id, WSL_ROOT.to_string(), cx);
                });
            }
        })
        .on_mouse_down(MouseButton::Middle, {
            let h = h.clone();
            move |_, _, cx| {
                h.update(cx, |this, cx| {
                    let id = this.state.active_pane_id.clone();
                    this.add_tab(&id, WSL_ROOT.to_string(), false, cx);
                });
            }
        })
        .on_mouse_down(MouseButton::Right, {
            let h = h.clone();
            move |event, _, cx| {
                let items = linux_context_menu(cx.i18n(), h.clone());
                h.update(cx, |this, cx| {
                    this.sidebar_selected_path = Some(WSL_ROOT.to_string());
                    this.context_menus.clear();
                    this.context_menus.push(ContextMenuInstance {
                        native_bounds: None,
                        position: event.position,
                        content: ContextMenuContent::Items(items, None),
                        scroll_handle: UniformListScrollHandle::new(),
                        width: px(0.0),
                        open_left: false,
                        hovered_item: None,
                        native_request_id: None,
                        open_up: false,
                    });
                    cx.notify();
                });
            }
        })
        .child(
            div()
                .w_4().h_4().mr_1()
                .flex().items_center().justify_center().flex_shrink_0()
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    toggle(&h, cx);
                })
                .child(svg()
                    .path(if is_expanded { "icons/chevron-down.svg" } else { "icons/chevron-right.svg" })
                    .size_full()
                    .text_color(theme.text)),
        )
        .child(
            div().w_4().h_4().mr_2().flex().items_center().justify_center().flex_shrink_0()
                .child(svg().path("icons/box.svg").size_4().text_color(theme.text)),
        )
        .child(
            div().flex_1().min_w_0().truncate()
                .text_color(theme.text)
                .font_weight(FontWeight::SEMIBOLD)
                .child(name),
        )
        .into_any_element()
}
