//! The commands of the selected operation, as footer buttons: pause or resume, turbo
//! and cancel while it runs, remove once it is over.
use super::super::{is_final, OperationsPanelView};
use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::elements::button::{action, button_style, ButtonVariant};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::systems::file_ops::{FileOperation, FileOperationManager, OpStatus};
use std::sync::Arc;

/// A footer button with its icon and label; `active` keeps a toggle selected.
pub(in super::super) fn button(
    id: impl Into<ElementId>,
    icon: &'static str,
    label: SharedString,
    enabled: bool,
    active: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = *cx.theme();
    let color = if !enabled {
        theme.icon_disabled
    } else if active {
        theme.accent
    } else {
        theme.text
    };
    action(button_style(div().id(id), theme, ButtonVariant::Secondary, enabled))
        .when(active, |button| button.bg(theme.selected).text_color(theme.accent))
        .when(enabled, |button| button.tab_index(0))
        .child(svg().path(icon).size(rems(0.875)).flex_none().text_color(color))
        .child(label)
}

fn command(
    button: Stateful<Div>,
    manager: Arc<FileOperationManager>,
    app: WeakEntity<OxydeApp>,
    act: impl Fn(&FileOperationManager) + 'static,
) -> Stateful<Div> {
    button.on_click(move |_, _, cx| {
        act(&manager);
        let _ = app.update(cx, |_, cx| cx.notify());
    })
}

pub(in super::super) fn render(
    operation: &FileOperation,
    app: WeakEntity<OxydeApp>,
    cx: &mut Context<OperationsPanelView>,
) -> Div {
    let i18n = cx.i18n();
    let row = div().flex().items_center().gap(rems(0.5));
    let Some(manager) = app.upgrade().map(|app| app.read(cx).file_ops.clone()) else {
        return row;
    };
    let id = operation.id.clone();
    if is_final(&operation.status) {
        return row.child(command(
            button("operation-dismiss", "icons/x.svg", i18n.text(Message::OpsDismiss), true, false, cx),
            manager,
            app,
            move |manager| {
                manager.dismiss_operation(&id);
            },
        ));
    }
    let can_start = matches!(operation.status, OpStatus::Paused | OpStatus::Queued);
    let turbo = operation.turbo;
    let (pause_id, turbo_id) = (id.clone(), id.clone());
    row.child(command(
        button(
            "operation-pause",
            if can_start { "icons/play.svg" } else { "icons/pause.svg" },
            i18n.text(if can_start { Message::OpsResume } else { Message::OpsPause }),
            true,
            false,
            cx,
        ),
        manager.clone(),
        app.clone(),
        move |manager| {
            manager.set_paused(&pause_id, !can_start);
        },
    ))
    // Turbo is a toggle: its button stays selected while it is on.
    .child(command(
        button("operation-turbo", if turbo { "icons/zap.svg" } else { "icons/zap-off.svg" }, i18n.text(Message::OpsTurbo), true, turbo, cx)
            .fixed_tooltip(Tooltip::text(i18n.text(if turbo { Message::OpsFast } else { Message::OpsNormal }))),
        manager.clone(),
        app.clone(),
        move |manager| {
            manager.set_turbo(&turbo_id, !turbo);
        },
    ))
    .child(command(
        button("operation-cancel", "icons/square.svg", i18n.text(Message::OpsStop), true, false, cx),
        manager,
        app,
        move |manager| {
            manager.cancel_operation(&id);
        },
    ))
}
