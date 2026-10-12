use super::{menus, pinned_display_name};
use crate::ui::app::{ContextMenuContent, ContextMenuInstance, OxydeApp};
use crate::ui::i18n::I18nContext;
use crate::ui::scale::rem_pixels;
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

pub(crate) fn render_pinned_menu(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> AnyElement {
    let theme = *cx.theme();
    let origin = view
        .context_anchor_points
        .get("sidebar-pinned")
        .copied()
        .unwrap_or_default();
    let button_bounds = Bounds::new(origin, size(rem_pixels(rems(2.0)), rem_pixels(rems(2.0))));
    let mut menu = div()
        .id("pinned-rail-menu")
        .occlude()
        .w(rems(17.0))
        .max_h(rems(30.0))
        .overflow_y_scroll()
        .py(rems(0.25))
        .bg(theme.menu_surface)
        .border(rems(0.0625))
        .border_color(theme.menu_border)
        .shadow(crate::ui::style::rem_shadow(false));
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
        let context = handle.clone();
        let context_path = path.clone();
        menu = menu.child(
            div()
                .id(("pinned-rail-item", index))
                .px(rems(0.75))
                .h(rems(1.625))
                .flex()
                .items_center()
                .gap(rems(0.5))
                .min_w_0()
                .hover(|row| row.bg(theme.menu_hover))
                .child(svg().path("icons/pin.svg").size(rems(0.875)).flex_none().text_color(theme.text_muted))
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
                .on_mouse_down(MouseButton::Right, move |event, _, cx| {
                    cx.stop_propagation();
                    let items = menus::pinned_context_menu(
                        cx.i18n(),
                        context.clone(),
                        context_path.clone(),
                    );
                    context.update(cx, |this, cx| {
                        this.pinned_rail_open = false;
                        this.context_menus.clear();
                        this.context_menus.push(ContextMenuInstance {
                            position: event.position,
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
                }),
        );
    }
    menu = menu.on_mouse_down_out(move |event, _, cx| {
        if button_bounds.contains(&event.position) {
            return;
        }
        handle.update(cx, |this, cx| {
            this.pinned_rail_open = false;
            cx.notify();
        });
    });
    deferred(
        anchored()
            .position(point(origin.x + rem_pixels(rems(2.5)), origin.y))
            .snap_to_window()
            .child(menu),
    )
    .into_any_element()
}
