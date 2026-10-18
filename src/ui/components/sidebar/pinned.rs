use super::{menus, pinned_display_name};
use crate::ui::app::{ContextMenuContent, ContextMenuInstance, OxydeApp};
use crate::ui::i18n::I18nContext;
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

pub(crate) fn render_pinned_menu(view: &OxydeApp, handle: Entity<OxydeApp>, window: &mut Window, cx: &mut Context<OxydeApp>) -> AnyElement {
    #[cfg(target_os = "windows")]
    if crate::ui::components::popup::supported(window, cx) {
        let origin = view.context_anchor_points.get("sidebar-pinned").copied().unwrap_or_default();
        // Include the rail's trailing padding so the menu starts outside
        // the sidebar, while clicks on the button still reach its toggle.
        let bounds = Bounds::new(origin, size(window.rem_size() * 2.5, window.rem_size() * 2.0));
        crate::ui::components::popup::show("pinned-rail", crate::ui::components::popup::PopupAnchor::beside(bounds, false), handle.clone(), window, cx,
            |view| view.sidebar_collapsed && view.pinned_rail_open,
            |view, window, cx| render_pinned_contents(view, cx.entity(), window, cx),
            |view, _, cx| { view.pinned_rail_open = false; cx.notify(); });
        return div().into_any_element();
    }
    render_pinned_contents(view, handle, window, cx)
}

// Align with the rail's outer edge, including its half-rem trailing padding.
fn menu_position(origin: Point<Pixels>, rem: Pixels) -> Point<Pixels> {
    point(origin.x + rem * 2.5, origin.y)
}

fn render_pinned_contents(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> AnyElement {
    let theme = *cx.theme();
    let origin = view
        .context_anchor_points
        .get("sidebar-pinned")
        .copied()
        .unwrap_or_default();
    let button_bounds = Bounds::new(origin, size(window.rem_size() * 2.0, window.rem_size() * 2.0));
    let menu = div()
        .id("pinned-rail-menu")
        .occlude()
        .w(rems(17.0))
        .max_h(rems(30.0))
        .p(rems(0.4375))
        .text_size(rems(0.8125))
        .text_color(theme.text)
        .bg(theme.menu_surface)
        .border(rems(0.0625))
        .border_color(theme.menu_border)
        .rounded(rems(0.5))
        .shadow(crate::ui::style::rem_shadow(true));
    let mut contents = div().id("pinned-rail-scroll").max_h(rems(29.0)).overflow_y_scroll();
    for (index, item) in view
        .state
        .pinned_items
        .iter()
        .filter(|item| view.show_hidden || !item.name.starts_with('.'))
        .enumerate()
    {
        let path = item.path.clone();
        let open = handle.clone();
        let middle = handle.clone();
        let middle_path = path.clone();
        let key_open = handle.clone(); let key_path = path.clone();
        let key_action = std::rc::Rc::new(move |_: &mut Window, cx: &mut App| { key_open.update(cx, |this, cx| { this.pinned_rail_open = false; let pane = this.state.active_pane_id.clone(); this.navigate_to(&pane, key_path.clone(), cx); }); });
        let context = handle.clone();
        let context_path = path.clone();
        contents = contents.child(
            crate::ui::components::popup::keyboard::item(div()
                .id(("pinned-rail-item", index))
                .px(rems(0.5))
                .h(rems(1.625))
                .flex()
                .items_center()
                .gap(rems(0.5))
                .min_w_0()
                .rounded(rems(0.3125))
                .hover(|row| row.bg(theme.menu_hover))
                .child(svg().path("icons/pin.svg").size(rems(1.0)).flex_none().text_color(theme.text))
                .child(div().flex_1().min_w_0().truncate().child(pinned_display_name(item, cx.i18n())))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    open.update(cx, |this, cx| {
                        this.pinned_rail_open = false;
                        let pane = this.state.active_pane_id.clone();
                        this.navigate_to(&pane, path.clone(), cx);
                    });
                })
                .on_mouse_down(MouseButton::Middle, move |_, _, cx| {
                    cx.stop_propagation();
                    middle.update(cx, |this, cx| {
                        this.pinned_rail_open = false;
                        let pane = this.state.active_pane_id.clone();
                        this.add_tab(&pane, middle_path.clone(), false, cx);
                    });
                })
                .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                    cx.stop_propagation();
                    let position = crate::ui::components::popup::owner_point(window, cx, event.position);
                    let items = menus::pinned_context_menu(
                        cx.i18n(),
                        context.clone(),
                        context_path.clone(),
                    );
                    context.update(cx, |this, cx| {
                        this.context_menus.clear();
                        this.context_menus.push(ContextMenuInstance {
            native_bounds: None,
                            position,
                            content: ContextMenuContent::Items(items, None),
                            scroll_handle: UniformListScrollHandle::new(),
                            width: px(0.0),
                            bottom_padding: 0.0,
                            open_left: false,
                            hovered_item: None,
                            native_request_id: None,
                        });
                        cx.notify();
                    });
                }), key_action, None, None),
        );
    }
    let menu = menu.child(contents).on_mouse_down_out(move |event, window, cx| {
        let position = crate::ui::components::popup::owner_point(window, cx, event.position);
        if button_bounds.contains(&position) {
            return;
        }
        handle.update(cx, |this, cx| {
            this.pinned_rail_open = false;
            cx.notify();
        });
    });
    if crate::ui::components::popup::is_menu(window, cx) { return menu.into_any_element(); }
    deferred(
        anchored()
            .position(menu_position(origin, window.rem_size()))
            .snap_to_window()
            .child(menu),
    )
    .into_any_element()
}

#[cfg(test)]
#[path = "tests/pinned_tests.rs"]
mod tests;
