use super::OperationsPanelView;
use crate::ui::app::OxydeApp;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::*;

impl OperationsPanelView {
    pub fn new(app: Entity<OxydeApp>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&app, |_, _, cx| cx.notify());
        Self {
            app: app.downgrade(),
            _app_subscription: subscription,
            focus: cx.focus_handle(),
        }
    }

    fn move_scrollbar(&self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let _ = self.app.update(cx, |app, cx| {
            if !matches!(
                app.drag_target,
                Some(crate::ui::app::DragTarget::OperationsPanelV { .. })
            ) {
                return;
            }
            if !event.dragging() {
                app.finish_pointer_drag(cx);
                return;
            }
            let Some(crate::ui::app::DragTarget::OperationsPanelV {
                viewport_h,
                content_h,
                track_id,
            }) = app.drag_target.as_ref()
            else {
                return;
            };
            let Some(origin) = app.scrollbar_origins.get(track_id) else {
                return;
            };
            let thumb = (viewport_h * (viewport_h / content_h.max(1.0)).min(1.0))
                .max(crate::ui::scale::logical(40.0));
            let travel = (viewport_h - thumb).max(1.0);
            let position =
                (event.position.y.as_f32() - origin.y - app.grab_offset).clamp(0.0, travel);
            app.operations_panel_scroll.set_offset(point(
                px(0.0),
                px(-position / travel * (content_h - viewport_h).max(0.0)),
            ));
            cx.notify();
        });
    }
}

impl Render for OperationsPanelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(app) = self.app.upgrade() else {
            return div().into_any_element();
        };
        let theme = *cx.theme();
        window.set_rem_size(px(app.read(cx).ui_rem_size() as f32));
        let mut args = FluentArgs::new();
        args.set(
            "count",
            app.read(cx).file_ops.operation_counts().0 as i64,
        );
        let title = cx.i18n().text_with_args(Message::OpsTitle, &args);
        window.set_window_title(title.as_ref());
        let body = app.update(cx, |owner, cx| {
            super::panel::render_operations_panel(owner, app.clone(), cx)
        });
        let root = div()
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.active_background)
            .text_color(theme.text)
            .child(super::super::tool_window::pointer_drag(cx.entity(),
                |this, event, cx| this.move_scrollbar(event, cx), |this, cx| {
                let _ = this.app.update(cx, |app, cx| app.finish_pointer_drag(cx));
            }))
            .font_family("Inter")
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .text_size(rems(0.8125))
            .line_height(relative(1.25))
            .on_key_down(|event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    super::super::tool_window::close_transients(window, cx);
                    window.remove_window();
                }
            })
            .child(super::super::tool_window::header_with_actions(
                "icons/download.svg",
                title,
                window,
                true,
                cx,
                super::title_actions::panel(app.clone(), cx),
                |window, cx| {
                    super::super::tool_window::close_transients(window, cx);
                    window.remove_window();
                },
            ))
            .child(body);
        crate::ui::components::client_decorations::frame(root, theme.border, window)
    }
}
