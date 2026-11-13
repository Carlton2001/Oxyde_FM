use super::OperationsPanelView;
use crate::ui::app::OxydeApp;
use crate::ui::components::scrollbar::{drag_scrolled, ScrollbarState};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::systems::file_ops::{FileOperation, OperationSummary};

/// Scrollbar tracks, told apart by their name's end.
pub(super) const LIST_TRACK: &str = "operations-list";
pub(super) const ITEMS_TRACK: &str = "operation-items";

impl ScrollbarState for OperationsPanelView {
    fn scrollbar_origins(&mut self) -> &mut std::collections::HashMap<String, Point<f32>> {
        &mut self.scrollbar_origins
    }
    fn set_grab_offset(&mut self, offset: f32) {
        self.grab_offset = offset;
    }
}

impl OperationsPanelView {
    pub fn new(app: Entity<OxydeApp>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&app, |_, _, cx| cx.notify());
        let list_width = app.read(cx).operations_list_width();
        Self {
            app: app.downgrade(),
            _app_subscription: subscription,
            focus: cx.focus_handle(),
            selected: None,
            list_scroll: ScrollHandle::new(),
            list_width,
            rem_px: 16.0,
            resizing_list: false,
            items: Default::default(),
            scrollbar_origins: Default::default(),
            grab_offset: 0.0,
            scroll_drag: None,
        }
    }

    /// Prepares the view for a new window, showing `operation` when one was asked for.
    pub(super) fn attach(&mut self, operation: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        if operation.is_some() {
            self.selected = operation;
        }
        self.attach_items_input(window, cx);
        // The rows are listed again for the new window's filter.
        self.items.operation = None;
        window.focus(&self.focus, cx);
    }

    /// Shows `operation` in a window already open.
    pub(super) fn select(&mut self, operation: String, cx: &mut Context<Self>) {
        self.selected = Some(operation);
        cx.notify();
    }

    /// Keeps the selection on an operation still listed: else the newest running
    /// one, else the newest.
    fn settle_selection(&mut self, summaries: &[OperationSummary]) {
        let listed = |id: &str| summaries.iter().any(|summary| summary.progress.id == id);
        if self.selected.as_deref().is_some_and(listed) {
            return;
        }
        self.selected = summaries
            .iter()
            .rev()
            .find(|summary| !super::is_final(&summary.progress.status))
            .or_else(|| summaries.last())
            .map(|summary| summary.progress.id.clone());
    }

    fn move_scrollbar(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some((track, viewport_h, content_h)) = self.scroll_drag.clone() else { return };
        if !event.dragging() {
            self.scroll_drag = None;
            cx.notify();
            return;
        }
        let Some(origin) = self.scrollbar_origins.get(&track) else { return };
        let scrolled = drag_scrolled(viewport_h, content_h, viewport_h, event.position.y.as_f32() - origin.y, self.grab_offset);
        let handle = if track.ends_with(LIST_TRACK) {
            self.list_scroll.clone()
        } else {
            self.items.list.0.borrow().base_handle.clone()
        };
        handle.set_offset(point(px(0.0), px(-scrolled)));
        cx.notify();
    }

    /// The secondary windows' footer: removing finished operations, then the selected
    /// one's commands and the window's closing.
    fn render_footer(&self, summaries: &[OperationSummary], operation: Option<&FileOperation>, cx: &mut Context<Self>) -> Div {
        let any_finished = summaries.iter().any(|summary| super::is_final(&summary.progress.status));
        let manager = self.app.upgrade().map(|app| app.read(cx).file_ops.clone());
        let app = self.app.clone();
        let dismiss = super::detail::controls::button(
            "operations-dismiss-finished",
            "icons/broom.svg",
            cx.i18n().text(Message::OpsDismissFinished),
            any_finished,
            false,
            cx,
        )
        .when(any_finished, |button| {
            button.on_click(move |_, _, cx| {
                if let Some(manager) = &manager {
                    manager.dismiss_finished_operations();
                }
                let _ = app.update(cx, |_, cx| cx.notify());
            })
        });
        crate::ui::components::tool_window::footer(cx)
            .child(dismiss)
            .child(div().flex_1())
            .when_some(operation, |footer, operation| {
                footer.child(super::detail::controls::render(operation, self.app.clone(), cx))
            })
            .child(
                crate::ui::components::tool_window::action_button("operations-close", cx.i18n().text(Message::Close), true, cx)
                    .on_click(|_, window, cx| {
                        crate::ui::components::tool_window::close_transients(window, cx);
                        window.remove_window();
                    }),
            )
    }

    fn render_empty(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_color(cx.theme().text_muted)
            .child(cx.i18n().text(Message::OpsEmpty))
    }
}

impl Render for OperationsPanelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(app) = self.app.upgrade() else {
            return div().into_any_element();
        };
        let theme = *cx.theme();
        window.set_rem_size(px(app.read(cx).ui_rem_size() as f32));
        self.rem_px = window.rem_size().as_f32();
        let manager = app.read(cx).file_ops.clone();
        let system = app.read(cx).use_system_icons();
        let summaries = manager.list_summaries();
        self.settle_selection(&summaries);
        let mut args = FluentArgs::new();
        args.set("count", summaries.len() as i64);
        let title = cx.i18n().text_with_args(Message::OpsTitle, &args);
        window.set_window_title(title.as_ref());
        let placeholder = cx.i18n().text(Message::OpsItemsFilter);
        if let Some(input) = &self.items.input {
            input.update(cx, |input, cx| input.set_placeholder(placeholder, cx));
        }
        let operation = self.selected.as_deref().and_then(|id| manager.get_operation(id));
        if let Some(operation) = &operation {
            self.follow_items(operation, cx);
        }
        self.measure_items(window, cx);
        let detail = match &operation {
            Some(operation) => div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .child(self.render_heading(operation, cx))
                .child(self.render_overview(operation, cx))
                .child(self.render_items(operation, system, window, cx)),
            None => self.render_empty(cx),
        };
        let root = div()
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.active_background)
            .text_color(theme.text)
            .child(crate::ui::components::tool_window::pointer_drag(cx.entity(),
                |this, event, cx| if this.resizing_list { this.resize_list(event, cx) } else { this.move_scrollbar(event, cx) },
                |this, cx| {
                    this.scroll_drag = None;
                    this.finish_list_resize(cx);
                    cx.notify();
                }))
            .font_family("Inter")
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .text_size(rems(0.8125))
            .line_height(relative(1.25))
            .capture_key_down(|event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => {
                        crate::ui::components::tool_window::close_transients(window, cx);
                        window.remove_window();
                        cx.stop_propagation();
                    }
                    "tab" => {
                        if event.keystroke.modifiers.shift {
                            window.focus_prev(cx);
                        } else {
                            window.focus_next(cx);
                        }
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            })
            .child(crate::ui::components::tool_window::header(
                "icons/download.svg",
                title,
                window,
                true,
                cx,
                |window, cx| {
                    crate::ui::components::tool_window::close_transients(window, cx);
                    window.remove_window();
                },
            ))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .when(!summaries.is_empty(), |body| {
                        let width = self.list_display_width(window);
                        body.child(self.render_list(&summaries, width, cx)).child(self.render_list_edge(window, cx))
                    })
                    .child(detail),
            )
            .child(self.render_footer(&summaries, operation.as_ref(), cx));
        crate::ui::components::client_decorations::frame(root, theme.border, window, cx)
    }
}
