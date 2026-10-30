use crate::ui::components::tooltip::NativeTooltipExt;
use super::super::{ContextMenuContent, ContextMenuInstance};
use super::AdvancedSearchWindow;
use crate::ui::{
    components::{
        dropdown::DropdownItem,
        modal::{action_button, responsive_form_row},
    },
    i18n::{I18nContext, Message},
    theme::ThemeContext,
};
use gpui::prelude::*;
use gpui::*;

fn form_row(label: SharedString, value: impl IntoElement, compact: bool, cx: &App) -> impl IntoElement {
    responsive_form_row(label, value, rems(9.0), compact, cx)
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
        let compact = window.viewport_size().width < rems(40.0).to_pixels(window.rem_size());
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
            body = body.child(form_row(cx.i18n().text(label), field, compact, cx));
            if index == 1 {
                body = body.child(form_row(
                    "".into(),
                    div()
                        .flex()
                        .flex_wrap()
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
                    compact, cx,
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
                .focus(move |style| style.border_color(theme.accent))
                .child(units[d.unit])
                .child(
                    svg()
                        .path("icons/chevron-down.svg")
                        .size(rems(0.75))
                        .text_color(theme.text),
                )
                .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                    unit_handle.update(cx, |this, cx| {
                        let dismissed = this.unit_menu_dismissed.take().is_some_and(|at| at.elapsed() < std::time::Duration::from_millis(300));
                        if dismissed || this.unit_menu.take().is_some_and(|scroll| this.context_menus.first().is_some_and(|menu| std::rc::Rc::ptr_eq(&menu.scroll_handle.0, &scroll.0))) {
                            this.context_menus.clear();
                            cx.notify();
                            return;
                        }
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
                            open_left: false,
                            hovered_item: None,
                            native_request_id: None, open_up: false,
                        });
                        this.unit_menu = this.context_menus.first().map(|menu| menu.scroll_handle.clone());
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
            let mut fields = div().flex().items_center().gap(rems(0.5))
                .when(compact, |fields| fields.flex_col().items_stretch());
            for index in start..=start + 1 {
                if index != start && !compact {
                    fields = fields.child("—");
                }
                let mut field = crate::ui::components::input::field_frame(&d.inputs[index], window, cx)
                    .flex_1()
                    .when(compact, |field| field.flex_none().w_full())
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
                            .flex_none()
                            .rounded_sm()
                            .border(rems(0.0625))
                            .border_color(transparent_black())
                            .focus(move |style| style.border_color(theme.accent).bg(theme.control_hover))
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
                fields = if compact {
                    fields.child(div().flex().flex_col().gap(rems(0.25))
                        .child(div().text_color(theme.text_muted).child(cx.i18n().text(
                            if index == start { Message::AdvancedMin } else { Message::AdvancedMax })))
                        .child(field))
                } else { fields.child(field) };
            }
            if start == 3 {
                fields = fields.child(unit.take().unwrap());
            }
            body = body.child(form_row(cx.i18n().text(label), fields, compact, cx));
        }
        body = body.child(form_row(
            cx.i18n().text(Message::AdvancedGlobalOptions),
            div()
                .flex()
                .flex_wrap()
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
            compact, cx,
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
            .child(if !regex {
                crate::ui::elements::checkbox::checkbox(theme, checked, false)
            } else {
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
                    })
            })
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
