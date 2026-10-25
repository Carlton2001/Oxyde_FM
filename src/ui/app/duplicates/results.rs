use super::{presentation::Row, DuplicateWindow};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::FluentBuilder;
use gpui::*;

impl DuplicateWindow {
    pub(super) fn render_results(
        &self,
        theme: OxydeTheme,
        system_icons: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let mut results = div()
            .id("duplicate-results")
            .flex_1()
            .min_h_0()
            .p(rems(1.0))
            .flex()
            .flex_col()
            .gap(rems(0.5));
        if self.running {
            let progress = self.cancellation.1.lock().unwrap().clone();
            let mut a = FluentArgs::new();
            a.set(
                "current",
                crate::ui::format::format_number(progress.current as u64, cx.i18n().language()),
            );
            a.set(
                "total",
                crate::ui::format::format_number(progress.total as u64, cx.i18n().language()),
            );
            results = results.child(cx.i18n().text_with_args(
                if progress.stage == "Scanning" {
                    Message::DuplicateScanning
                } else {
                    Message::DuplicateHashing
                },
                &a,
            ));
        }
        if let Some(error) = &self.error {
            let mut a = FluentArgs::new();
            a.set("error", error.as_str());
            results = results.child(
                div()
                    .text_color(theme.error)
                    .child(cx.i18n().text_with_args(Message::DuplicateFailed, &a)),
            );
        }
        if self.filtered.is_empty() && !self.running && !self.filtering && self.error.is_none() {
            results = results.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(rems(1.0))
                    .text_color(theme.text_muted)
                    .child(
                        svg()
                            .path("icons/search.svg")
                            .size(rems(3.0))
                            .text_color(theme.icon_disabled),
                    )
                    .child(div().max_w(rems(18.75)).text_center().child(cx.i18n().text(
                        if self.searched {
                            Message::DuplicateEmpty
                        } else {
                            Message::DuplicateIdle
                        },
                    ))),
            );
        }

        if self.result_rows.len == 0 {
            return results;
        }
        let source = cx.entity();
        results.child(
            uniform_list(
                "duplicate-result-rows",
                self.result_rows.len,
                move |range, _, cx| {
                    source.update(cx, |view, cx| {
                        range
                            .filter_map(|index| view.result_rows.row(index))
                            .map(|row| match row {
                                Row::Header {
                                    group,
                                    min_size,
                                    max_size,
                                    expanded,
                                } => view.render_group_header(
                                    group, min_size, max_size, expanded, theme, cx,
                                ),
                                Row::File { group, file } => {
                                    view.render_file_row(group, file, theme, system_icons, cx)
                                }
                            })
                            .collect::<Vec<_>>()
                    })
                },
            )
            .flex_1()
            .min_h_0()
            .w_full()
            .track_scroll(&self.result_list),
        )
    }

    fn render_group_header(
        &self,
        index: usize,
        min_size: u64,
        max_size: u64,
        expanded: bool,
        theme: OxydeTheme,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let group = &self.groups[index];
        let mut args = FluentArgs::new();
        args.set("index", (index + 1) as i64);
        let min = crate::ui::format::format_size(min_size, false, true, cx.i18n().language());
        let size = if min_size == max_size {
            min
        } else {
            let mut range = FluentArgs::new();
            range.set("min", min);
            range.set(
                "max",
                crate::ui::format::format_size(max_size, false, true, cx.i18n().language()),
            );
            cx.i18n()
                .text_with_args(Message::DuplicateSizeRange, &range)
                .to_string()
        };
        div()
            .id(("group", index))
            .h(rems(4.0))
            .w_full()
            .flex_none()
            .tab_index(0)
            .focus_visible(move |s| s.border(rems(0.0625)).border_color(theme.accent))
            .px(rems(0.5))
            .bg(theme.header)
            .rounded_t(rems(0.5625))
            .when(!expanded, |header| header.rounded_b(rems(0.5625)))
            .border(rems(0.0625))
            .border_color(theme.border)
            .hover(move |s| s.bg(theme.hover))
            .flex()
            .items_center()
            .gap(rems(0.75))
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.collapsed.remove(&index) {
                    this.collapsed.insert(index);
                }
                this.prepare_results(false, cx);
                cx.notify();
            }))
            .child(
                svg()
                    .path(if expanded {
                        "icons/chevron-down.svg"
                    } else {
                        "icons/chevron-right.svg"
                    })
                    .size(rems(1.0))
                    .text_color(theme.text),
            )
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .child(cx.i18n().text_with_args(Message::DuplicateGroup, &args)),
            )
            .child(
                div()
                    .rounded(rems(0.75))
                    .px(rems(0.5))
                    .py(rems(0.125))
                    .min_w(rems(1.5))
                    .bg(theme.accent)
                    .text_color(theme.text_on_accent)
                    .text_size(rems(0.75))
                    .font_weight(FontWeight::MEDIUM)
                    .child(crate::ui::format::format_number(
                        group.files.len() as u64,
                        cx.i18n().language(),
                    )),
            )
            .child(div().flex_1())
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .px(rems(0.6))
                    .py(rems(0.2))
                    .rounded(rems(0.375))
                    .border(rems(0.0625))
                    .border_color(theme.border)
                    .bg(theme.active_background)
                    .text_size(rems(0.75))
                    .child(size),
            )
    }

    fn render_file_row(
        &self,
        index: usize,
        file_index: usize,
        theme: OxydeTheme,
        system_icons: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let file = &self.groups[index].files[file_index];
        let target = file.path.clone();
        let parent = std::path::Path::new(&target)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let destination = parent.clone();
        let hover_path = target.clone();
        let hover_name = file.name.clone();
        let hover_parent = parent.clone();
        let tooltip = self
            .result_tooltip
            .as_ref()
            .filter(|(path, _)| path == &target)
            .map(|(_, view)| view.clone());

        div()
            .h(rems(4.0))
            .flex_none()
            .w_full()
            .bg(theme.surface)
            .id(ElementId::Name(
                format!("duplicate-file-{index}-{file_index}").into(),
            ))
            .relative()
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if !*hovered && this.hovered_result.as_ref() != Some(&hover_path) {
                    return;
                }
                this.tooltip_task = None;
                this.result_tooltip = None;
                if *hovered {
                    this.hovered_result = Some(hover_path.clone());
                    let path = hover_path.clone();
                    let name = hover_name.clone();
                    let parent = hover_parent.clone();
                    this.tooltip_task = Some(cx.spawn(async move |this, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(500))
                            .await;
                        let _ = this.update(cx, |this, cx| {
                            if this.hovered_result.as_ref() == Some(&path) {
                                let view =
                                    crate::ui::components::tooltip::Tooltip::duplicate_result(
                                        name, parent, cx,
                                    );
                                this.result_tooltip = Some((path, view));
                                cx.notify();
                            }
                        });
                    }));
                } else if this.hovered_result.as_ref() == Some(&hover_path) {
                    this.hovered_result = None;
                }
                cx.notify();
            }))
            .child(
                canvas(
                    move |bounds, window, _| {
                        if let Some(view) = tooltip {
                            window.set_tooltip(AnyTooltip {
                                view,
                                mouse_position: point(
                                    bounds.left(),
                                    bounds.bottom() + crate::ui::scale::rem_pixels(rems(0.25)),
                                ),
                                check_visible_and_update: std::rc::Rc::new(move |_, window, _| {
                                    bounds.contains(&window.mouse_position())
                                }),
                            });
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .tab_index(0)
            .focus_visible(move |s| s.border(rems(0.0625)).border_color(theme.accent))
            .px(rems(1.0))
            .py(rems(0.75))
            .rounded(rems(0.625))
            .flex()
            .items_center()
            .gap(rems(0.75))
            .hover(move |s| s.bg(theme.hover))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.tooltip_task = None;
                this.hovered_result = None;
                this.result_tooltip = None;
                cx.notify();
                let _ = this.owner.update(cx, |owner, cx| {
                    let pane = owner.state.active_pane_id.clone();
                    owner.navigate_to(&pane, destination.clone(), cx);
                    if let Some(pane) = owner.state.find_pane_mut(&pane) {
                        let tab = pane.active_tab_mut();
                        tab.selected_paths.clear();
                        tab.selected_paths.insert(target.clone());
                        tab.selection_anchor = Some(target.clone());
                        tab.pending_reveal = Some(target.clone());
                    }
                    cx.notify();
                });
            }))
            .child(crate::ui::components::icons::render_file_icon(
                file.path.clone(),
                crate::ui::components::icons::entry_icon(file),
                rems(1.75),
                &theme,
                system_icons,
                true,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(file.name.clone()),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!(
                                "duplicate-parent-{index}-{file_index}"
                            )))
                            .text_color(theme.text_muted)
                            .text_size(rems(0.75))
                            .truncate()
                            .child(parent),
                    ),
            )
    }
}
