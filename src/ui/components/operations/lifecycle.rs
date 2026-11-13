use super::OperationsPanelView;
use crate::ui::app::OxydeApp;
use crate::ui::i18n::{I18nContext, Message};
use gpui::*;

pub(crate) fn close_operations_windows(cx: &mut App) -> bool {
    let mut closed = false;
    for handle in cx.windows() {
        if let Some(handle) = handle.downcast::<OperationsPanelView>() {
            closed |= handle.update(cx, |_, window, cx| {
                super::super::tool_window::close_transients(window, cx);
                window.remove_window();
            }).is_ok();
        }
    }
    closed
}

/// Opens the operations window, or brings it forward, on `operation` when given,
/// else on the newest running one; with none running, the selection stays.
pub(crate) fn open_operations_window(app: Entity<OxydeApp>, operation: Option<String>, cx: &mut App) {
    if app.read(cx).file_ops.operation_counts().0 == 0 {
        return;
    }
    let operation = operation.or_else(|| {
        app.read(cx)
            .file_ops
            .list_summaries()
            .into_iter()
            .rev()
            .find(|summary| !super::is_final(&summary.progress.status))
            .map(|summary| summary.progress.id)
    });
    for handle in cx.windows() {
        if let Some(handle) = handle.downcast::<OperationsPanelView>() {
            cx.defer(move |cx| {
                let _ = handle.update(cx, |view, window, cx| {
                    if let Some(operation) = operation {
                        view.select(operation, cx);
                    }
                    window.activate_window();
                });
            });
            return;
        }
    }
    let owner = app.downgrade();
    let title = cx.i18n().text(Message::OpsShowPanel);
    cx.defer(move |cx| {
        let Some(owner) = owner.upgrade() else { return };
        if owner.read(cx).file_ops.operation_counts().0 == 0 {
            return;
        }
        for handle in cx.windows() {
            if let Some(handle) = handle.downcast::<OperationsPanelView>() {
                let _ = handle.update(cx, |view, window, cx| {
                    if let Some(operation) = operation {
                        view.select(operation, cx);
                    }
                    window.activate_window();
                });
                return;
            }
        }
        let Some(view) = owner.read(cx).operations_panel_view.clone() else {
            return;
        };
        if let Err(error) = cx.open_window(
            WindowOptions {
                app_id: Some(crate::ui::APP_ID.into()),
                window_decorations: crate::ui::window_decorations(),
                window_bounds: Some(crate::ui::app::tool_geometry::restore_sized("icons/download.svg", Bounds::new(
                    point(px(160.0), px(140.0)),
                    size(px(1000.0), px(640.0)),
                ), cx)),
                // The list beside a details list keeping its name column readable.
                window_min_size: Some(size(px(crate::ui::scale::logical(760.0)), px(crate::ui::scale::logical(460.0)))),
                titlebar: Some(TitlebarOptions {
                    appears_transparent: true,
                    title: Some(title),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |window, cx| {
                super::super::tool_window::install_icon("icons/download.svg", window);
                view.update(cx, |view, cx| view.attach(operation, window, cx));
                view
            },
        ) {
            log::error!("Cannot open operations window: {error}");
        }
    });
}
