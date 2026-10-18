use crate::ui::components::tooltip::NativeTooltipExt;
use super::super::{ContextMenuContent, ContextMenuInstance};
use super::AdvancedSearchWindow;
use crate::ui::{
    components::{
        dropdown::DropdownItem,
        modal::{action_button, form_row_with_label_width},
    },
    i18n::{I18nContext, Message},
    theme::ThemeContext,
};
use gpui::prelude::*;
use gpui::*;

fn form_row(label: SharedString, value: impl IntoElement, cx: &App) -> impl IntoElement {
    form_row_with_label_width(label, value, rems(9.0), cx)
}

impl AdvancedSearchWindow {
    pub(super) fn render_advanced_settings(
        &self,
        mut body: Div,
        handle: Entity<Self>,
        units: [&'static str; 5],
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let d = self.advanced_search.as_ref().unwrap();
        let theme = *cx.theme();
        for (index, label, placeholder) in [
            (
                0,
                Message::AdvancedSearchFor,
                Message::AdvancedSearchForPlaceholder,
            ),
            (1, Message::AdvancedSearchIn, Message::PathPlaceholder),
            (
                2,
                Message::AdvancedFindText,
                Message::AdvancedFindTextPlaceholder,
            ),
        ] {
            d.inputs[index].update(cx, |input, cx| {
                input.set_placeholder(cx.i18n().text(placeholder), cx)
            });
            let mut field = crate::ui::components::input::field_frame(&d.inputs[index], window, cx);
            if index != 1 {
                field = field.child(self.advanced_toggle(
                    if index == 0 { 0 } else { 1 },
                    Message::AdvancedUseRegex,
                    true,
                    handle.clone(),
                    cx,
                ));
            } else {
                field = field.child(
                    svg()
                        .path("icons/folder.svg")
                        .size(rems(1.0))
                        .ml(rems(0.625))
                        .text_color(theme.text),
                );
            }
            let regex_enabled = match index {
                0 => d.flags[0],
                2 => d.flags[1],
                _ => false,
            };
            field = field.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .font_family(if regex_enabled {
                        "JetBrains Mono"
                    } else {
                        "Inter"
                    })
                    .child(d.inputs[index].clone()),
            );
            body = body.child(form_row(cx.i18n().text(label), field, cx));
            if index == 1 {
                body = body.child(form_row(
                    "".into(),
                    div()
                        .flex()
                        .gap(rems(0.75))
                        .child(self.advanced_toggle(
                            2,
                            Message::AdvancedSearchSubfolders,
                            false,
                            handle.clone(),
                            cx,
                        ))
                        .child(self.advanced_toggle(
                            3,
                            Message::AdvancedSearchInArchives,
                            false,
                            handle.clone(),
                            cx,
                        )),
                    cx,
                ));
            }
        }
        let unit_handle = handle.clone();
        let mut unit = Some(
            action_button("advanced-unit")
                .h(rems(2.25))
                .min_w(rems(5.0))
                .flex()
                .items_center()
                .justify_between()
                .gap(rems(0.5))
                .border(rems(0.0625))
                .border_color(theme.border)
                .bg(theme.surface)
                .track_focus(&d.controls[8])
                .child(units[d.unit])
                .child(
                    svg()
                        .path("icons/chevron-down.svg")
                        .size(rems(0.75))
                        .text_color(theme.text),
                )
                .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                    unit_handle.update(cx, |this, cx| {
                        let items = units
                            .into_iter()
                            .enumerate()
                            .map(|(index, label)| {
                                let h = unit_handle.downgrade();
                                DropdownItem::new(label, move |_, cx| {
                                    let _ = h.update(cx, |this, cx| {
                                        if let Some(d) = &mut this.advanced_search {
                                            d.unit = index;
                                        }
                                        cx.notify();
                                    });
                                })
                            })
                            .collect();
                        this.context_menus.clear();
                        this.context_menus.push(ContextMenuInstance {
            native_bounds: None,
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
        for (start, label) in [(3, Message::AdvancedSize), (5, Message::AdvancedDate)] {
            for (index, placeholder) in [
                (start, Message::AdvancedMin),
                (start + 1, Message::AdvancedMax),
            ] {
                d.inputs[index].update(cx, |input, cx| {
                    input.set_placeholder(
                        if start == 5 {
                            cx.i18n().text(Message::AdvancedDatePlaceholder)
                        } else {
                            cx.i18n().text(placeholder)
                        },
                        cx,
                    )
                });
            }
            let mut fields = div().flex().items_center().gap(rems(0.5));
            for index in start..=start + 1 {
                if index != start {
                    fields = fields.child("—");
                }
                let mut field = crate::ui::components::input::field_frame(&d.inputs[index], window, cx)
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(d.inputs[index].clone()),
                    );
                if start == 5 {
                    let h = handle.clone();
                    field = field.child(
                        div()
                            .id(SharedString::from(format!("advanced-calendar-{index}")))
                            .size(rems(1.5))
                            .flex()
                            .items_center()
                            .justify_center()
                            .track_focus(&d.controls[if index == 5 { 9 } else { 10 }])
                            .child(
                                svg()
                                    .path("icons/calendar-days.svg")
                                    .size(rems(1.0))
                                    .text_color(theme.text),
                            )
                            .on_click(move |_, window, cx| {
                                h.update(cx, |this, cx| {
                                    if let Some(d) = &mut this.advanced_search {
                                        d.calendar = Some((
                                            index,
                                            chrono::NaiveDate::parse_from_str(
                                                &d.values[index],
                                                "%Y-%m-%d",
                                            )
                                            .unwrap_or_else(|_| chrono::Local::now().date_naive()),
                                        ));
                                        window.focus(&d.focus, cx);
                                    }
                                    cx.notify();
                                });
                            }),
                    );
                }
                fields = fields.child(field);
            }
            if start == 3 {
                fields = fields.child(unit.take().unwrap());
            }
            body = body.child(form_row(cx.i18n().text(label), fields, cx));
        }
        body = body.child(form_row(
            cx.i18n().text(Message::AdvancedGlobalOptions),
            div()
                .flex()
                .gap(rems(0.75))
                .child(self.advanced_toggle(
                    4,
                    Message::AdvancedCaseSensitive,
                    false,
                    handle.clone(),
                    cx,
                ))
                .child(self.advanced_toggle(
                    5,
                    Message::AdvancedIgnoreAccents,
                    false,
                    handle.clone(),
                    cx,
                )),
            cx,
        ));
        body
    }

    fn advanced_toggle(
        &self,
        index: usize,
        message: Message,
        regex: bool,
        handle: Entity<Self>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = *cx.theme();
        let checked = self.advanced_search.as_ref().unwrap().flags[index];
        if regex {
            return div()
                .id(SharedString::from(format!("advanced-flag-{index}")))
                .track_focus(&self.advanced_search.as_ref().unwrap().controls[index])
                .flex_none()
                .w(rems(2.375))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(if checked { theme.accent } else { theme.header })
                .border_r(rems(0.0625))
                .border_color(theme.border)
                .focus(move |style| style.border_color(theme.accent))
                .child(
                    svg()
                        .path("icons/regex.svg")
                        .size(rems(0.875))
                        .text_color(if checked {
                            theme.text_on_accent
                        } else {
                            theme.text
                        }),
                )
                .native_tooltip(crate::ui::components::tooltip::Tooltip::text(
                    cx.i18n().text(message),
                ))
                .on_click(move |_, _, cx| {
                    handle.update(cx, |this, cx| {
                        if let Some(d) = &mut this.advanced_search {
                            d.flags[index] = !d.flags[index];
                        }
                        cx.notify();
                    });
                })
                .into_any_element();
        }
        div()
            .id(SharedString::from(format!("advanced-flag-{index}")))
            .flex()
            .items_center()
            .gap(rems(0.375))
            .track_focus(&self.advanced_search.as_ref().unwrap().controls[index])
            .focus(move |style| style.bg(theme.control_hover))
            .child(
                div()
                    .size(rems(if regex { 1.75 } else { 1.0 }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .border(rems(0.0625))
                    .border_color(theme.border)
                    .bg(if checked { theme.accent } else { theme.header })
                    .text_color(if checked {
                        theme.text_on_accent
                    } else {
                        theme.text_muted
                    })
                    .when(regex || checked, |e| {
                        e.child(
                            svg()
                                .path(if regex {
                                    "icons/regex.svg"
                                } else {
                                    "icons/check.svg"
                                })
                                .size(rems(if regex { 0.875 } else { 0.75 }))
                                .text_color(if checked {
                                    theme.text_on_accent
                                } else {
                                    theme.text
                                }),
                        )
                    }),
            )
            .when(!regex, |e| e.child(cx.i18n().text(message)))
            .native_tooltip(crate::ui::components::tooltip::Tooltip::text(
                cx.i18n().text(message),
            ))
            .on_click(move |_, _, cx| {
                handle.update(cx, |this, cx| {
                    if let Some(d) = &mut this.advanced_search {
                        d.flags[index] = !d.flags[index];
                    }
                    cx.notify();
                });
            })
            .into_any_element()
    }
}
