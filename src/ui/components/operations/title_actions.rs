use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::elements::button::toolbar_button_with_tooltip;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

pub(super) fn scroll_top(
    enabled: bool,
    cx: &App,
    action: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    toolbar_button_with_tooltip(
        "operations-top".into(),
        Tooltip::text(cx.i18n().text(Message::SidebarScrollTop)),
        Some("icons/arrow-up-to-line.svg"),
        None,
        *cx.theme(),
        enabled,
        action,
    )
}

pub(super) fn panel(app: Entity<OxydeApp>, cx: &App) -> Div {
    let owner = app.read(cx);
    let can_collapse = !owner.expanded_operations.is_empty();
    let can_top = crate::ui::components::scrollbar::scrolled_from_top(&owner.operations_panel_scroll);
    let collapse = app.clone();
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(rems(0.25))
        .mr(rems(0.375))
        .child(toolbar_button_with_tooltip(
            "operations-collapse-all".into(),
            Tooltip::text(cx.i18n().text(Message::SidebarCollapseAll)),
            Some("icons/chevrons-up.svg"),
            None,
            *cx.theme(),
            can_collapse,
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
