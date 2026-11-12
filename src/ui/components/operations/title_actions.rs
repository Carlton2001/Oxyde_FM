use crate::ui::app::OxydeApp;
use crate::ui::components::tool_window::title_button;
use crate::ui::i18n::{I18nContext, Message};
use gpui::prelude::*;
use gpui::*;

pub(super) fn scroll_top(
    enabled: bool,
    cx: &App,
    action: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    title_button(
        "operations-top",
        "icons/arrow-up-to-line.svg",
        cx.i18n().text(Message::SidebarScrollTop),
        enabled,
        cx,
        action,
    )
}

pub(super) fn panel(app: Entity<OxydeApp>, cx: &App) -> Div {
    let owner = app.read(cx);
    let can_collapse = !owner.expanded_operations.is_empty();
    let can_top = crate::ui::components::scrollbar::scrolled_from_top(&owner.operations_panel_scroll);
    let collapse = app.clone();
    div()
        .h_full()
        .flex()
        .flex_none()
        .child(title_button(
            "operations-collapse-all",
            "icons/chevrons-up.svg",
            cx.i18n().text(Message::SidebarCollapseAll),
            can_collapse,
            cx,
            move |_, _, cx| {
                collapse.update(cx, |app, cx| {
                    app.expanded_operations.clear();
                    cx.notify();
                });
            },
        ))
        .child(scroll_top(can_top, cx, move |_, _, cx| {
            app.update(cx, |app, cx| {
                let offset = app.operations_panel_scroll.offset();
                app.operations_panel_scroll
                    .set_offset(point(offset.x, px(0.0)));
                cx.notify();
            });
        }))
}
