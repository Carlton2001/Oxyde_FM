use super::OperationsPanelView;
use crate::ui::app::OxydeApp;
use crate::ui::i18n::{I18nContext, Message};
use gpui::*;

pub(crate) fn close_operations_windows(cx: &mut App) {
    for handle in cx.windows() {
        if let Some(handle) = handle.downcast::<OperationsPanelView>() {
            let _ = handle.update(cx, |_, window, cx| {
                super::super::tool_window::close_transients(window, cx);
                window.remove_window();
            });
        }
    }
}

pub(crate) fn open_operations_window(app: Entity<OxydeApp>, cx: &mut App) {
    if app.read(cx).file_ops.list_operations().is_empty() {
        return;
    }
    for handle in cx.windows() {
        if let Some(handle) = handle.downcast::<OperationsPanelView>() {
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, _| window.activate_window());
            });
            return;
        }
    }
    let owner = app.downgrade();
    let title = cx.i18n().text(Message::OpsShowPanel);
    cx.defer(move |cx| {
        let Some(owner) = owner.upgrade() else { return };
        if owner.read(cx).file_ops.list_operations().is_empty() {
            return;
        }
        for handle in cx.windows() {
            if let Some(handle) = handle.downcast::<OperationsPanelView>() {
                let _ = handle.update(cx, |_, window, _| window.activate_window());
                return;
            }
        }
        let Some(view) = owner.read(cx).operations_panel_view.clone() else {
            return;
        };
        if let Err(error) = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(160.0), px(140.0)),
                    size(px(720.0), px(560.0)),
                ))),
                window_min_size: Some(size(px(520.0), px(320.0))),
                titlebar: Some(TitlebarOptions {
                    appears_transparent: true,
                    title: Some(title),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |window, cx| {
                super::super::tool_window::install_icon("icons/download.svg", window);
                let focus = view.read(cx).focus.clone();
                window.focus(&focus, cx);
                view
            },
        ) {
            log::error!("Cannot open operations window: {error}");
        }
    });
}
