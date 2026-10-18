use super::DuplicateWindow;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::*;
use oxyde_lib::commands::duplicates::cancel_find_duplicates;

impl DuplicateWindow {
    pub(super) fn button(id: impl Into<ElementId>, label: SharedString, cx: &App) -> Stateful<Div> {
        Self::styled_button(id, label, false, cx)
    }

    fn styled_button(id: impl Into<ElementId>, label: SharedString, primary: bool, cx: &App) -> Stateful<Div> {
        crate::ui::components::tool_window::action_button(id, label, primary, cx)
    }

}

impl Render for DuplicateWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        window.set_window_title(cx.i18n().text(Message::ToolDuplicates).as_ref());
        let system_icons = self
            .owner
            .upgrade()
            .is_some_and(|owner| owner.read(cx).use_system_icons());
        if let Some(owner) = self.owner.upgrade() {
            window.set_rem_size(px(owner.read(cx).ui_rem_size() as f32));
        }
        let placeholder = cx.i18n().text(Message::DuplicateFilter);
        self.filter
            .update(cx, |input, cx| input.set_placeholder(placeholder, cx));
        let sidebar = self.render_sources(theme, cx);
        let mut args = FluentArgs::new();
        args.set("groups", self.groups.len() as i64);
        args.set(
            "group_count",
            crate::ui::format::format_number(self.groups.len() as u64, cx.i18n().language()),
        );
        let file_count = self.groups.iter().map(|g| g.files.len()).sum::<usize>();
        args.set("file_count", file_count as i64);
        args.set(
            "file_number",
            crate::ui::format::format_number(file_count as u64, cx.i18n().language()),
        );
        args.set(
            "files",
            self.groups.iter().map(|g| g.files.len()).sum::<usize>() as i64,
        );
        let results = self.render_results(theme, system_icons, cx);
        let disabled = self.selected.is_empty() || !self.options.iter().any(|v| *v);
        div()
            .id("duplicate-window")
            .on_mouse_move(cx.listener(|this, event, _, cx| this.move_scrollbar(event, cx)))
            .capture_any_mouse_up(cx.listener(|this, _, _, cx| {
                this.scroll_drag = None;
                cx.notify();
            }))
            .track_focus(&self.focus)
            .tab_group()
            .tab_stop(false)
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "tab" => {
                        if event.keystroke.modifiers.shift {
                            window.focus_prev(cx);
                        } else {
                            window.focus_next(cx);
                        }
                        cx.stop_propagation();
                    }
                    "escape" => {
                        if this.running {
                            cancel_find_duplicates(&this.cancellation);
                            cx.notify();
                        } else {
                            window.remove_window();
                        }
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.focus.is_focused(window) && event.keystroke.key == "enter" {
                    this.search(cx);
                    cx.stop_propagation();
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.active_background)
            .text_color(theme.text)
            .font_family("Inter")
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .text_size(rems(0.8125))
            .line_height(relative(1.25))
            .child(crate::ui::components::tool_window::header_with_actions(
                "icons/copy.svg", cx.i18n().text(Message::ToolDuplicates), window.is_maximized(), true, cx,
                self.render_title_actions(cx),
                { let cancellation = self.cancellation.clone(); move |window, _| {
                    cancel_find_duplicates(&cancellation); window.remove_window();
                } },
            ))
            .child(
                div().flex_1().min_h_0().flex().child(
                    div().relative().w(rems(17.5)).flex_none().min_h_0()
                        .child(sidebar)
                        .child(self.render_scrollbar(self.sources_scroll.clone(), "duplicate-sources", cx)),
                ).child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .p(rems(1.0))
                                .border_b(rems(0.0625))
                                .border_color(theme.border)
                                .flex()
                                .items_center()
                                .gap(rems(1.0))
                                .child(
                                    crate::ui::components::input::field_frame(&self.filter, window, cx)
                                        .flex_1()
                                        .pl(rems(0.75))
                                        .child(
                                            svg()
                                                .path("icons/search.svg")
                                                .size(rems(0.875))
                                                .flex_none()
                                                .text_color(theme.text),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .h_full()
                                                .child(self.filter.clone()),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .flex()
                                        .flex_col()
                                        .items_end()
                                        .gap(rems(0.1))
                                        .child(
                                            div().font_weight(FontWeight::MEDIUM).child(
                                                cx.i18n().text_with_args(
                                                    Message::DuplicateGroups,
                                                    &args,
                                                ),
                                            ),
                                        )
                                        .child(
                                            div().text_color(theme.text_muted).child(
                                                cx.i18n()
                                                    .text_with_args(Message::DuplicateFiles, &args),
                                            ),
                                        ),
                                ),
                        )
                        .child(
                            div().relative().flex().flex_col().flex_1().min_h_0()
                                .child(results)
                                .child(self.render_scrollbar(self.results_scroll.clone(), "duplicate-results", cx)),
                        ),
                ),
            )
            .child(
                crate::ui::components::tool_window::footer(cx)
                    .child(
                        Self::button("duplicate-cancel", cx.i18n().text(Message::Cancel), cx)
                            .on_click(cx.listener(|this, _, w, cx| {
                                if this.running {
                                    cancel_find_duplicates(&this.cancellation);
                                    cx.notify();
                                } else {
                                    w.remove_window();
                                }
                            })),
                    )
                    .child(
                        Self::styled_button(
                            "duplicate-search",
                            "".into(),
                            !disabled && !self.running,
                            cx,
                        )
                        .bg(if disabled || self.running {
                            theme.header
                        } else {
                            theme.accent
                        })
                        .text_color(if disabled || self.running {
                            theme.text_muted
                        } else {
                            theme.text_on_accent
                        })
                        .child(svg().path("icons/search.svg").size(rems(0.875)).text_color(
                            if disabled || self.running {
                                theme.text_muted
                            } else {
                                theme.text_on_accent
                            },
                        ))
                        .child(cx.i18n().text(Message::AdvancedSearch))
                        .on_click(cx.listener(|this, _, _, cx| this.search(cx))),
                    ),
            )
    }
}
