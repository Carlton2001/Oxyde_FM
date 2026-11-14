use crate::ui::components::tooltip::NativeTooltipExt;
use super::super::{ContextMenuContent, ContextMenuInstance};
use super::AdvancedSearchWindow;
use crate::ui::{
    components::{
        dropdown::DropdownItem,
        input::{self, FieldEdge},
        modal::responsive_form_row,
    },
    i18n::{I18nContext, Message},
    theme::ThemeContext,
};
use gpui::prelude::*;
use gpui::*;

fn form_row(label: SharedString, value: impl IntoElement, compact: bool, cx: &App) -> impl IntoElement {
    responsive_form_row(label, value, rems(6.5), compact, cx)
}

/// A muted title followed by a rule, separating groups of rows.
fn section(label: SharedString, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .flex()
        .items_center()
        .gap(rems(0.5))
        .pt(rems(0.25))
        .text_size(rems(0.75))
        .text_color(theme.text_muted)
        .child(label)
        .child(div().flex_1().h(rems(0.0625)).bg(theme.border))
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
        let compact = window.viewport_size().width < rems(30.0).to_pixels(window.rem_size());
        for (index, label, placeholder) in [
            (0, Message::AdvancedSearchFor, Message::AdvancedSearchForPlaceholder),
            (2, Message::AdvancedFindText, Message::AdvancedFindTextPlaceholder),
        ] {
            let field = self.advanced_text_field(index, placeholder, handle.clone(), window, cx);
            body = body.child(form_row(cx.i18n().text(label), field, compact, cx));
        }
        body = body.child(form_row("".into(), self.advanced_toggles(
            [(4, Message::AdvancedCaseSensitive), (5, Message::AdvancedIgnoreAccents)],
            handle.clone(), window, cx,
        ), compact, cx));
        body = body.child(section(cx.i18n().text(Message::AdvancedSectionLocation), cx));
        let folder = self.advanced_text_field(1, Message::PathPlaceholder, handle.clone(), window, cx);
        body = body.child(form_row(cx.i18n().text(Message::AdvancedSearchIn), folder, compact, cx));
        body = body.child(form_row("".into(), self.advanced_toggles(
            [(2, Message::AdvancedSearchSubfolders), (3, Message::AdvancedSearchInArchives)],
            handle.clone(), window, cx,
        ), compact, cx));
        body = body.child(section(cx.i18n().text(Message::AdvancedSectionFilters), cx));
        let unit_handle = handle.clone();
        let mut unit = Some(
            crate::ui::components::select_menu::trigger("advanced-unit", units[d.unit], d.controls[8].is_focused(window), true, cx)
                .track_focus(&d.controls[8])
                .child(canvas(|bounds, _, _| bounds, {
                    let measure = handle.clone();
                    move |bounds, _, _, cx| measure.update(cx, |this, _| this.unit_bounds = Some(bounds))
                }).absolute().inset_0().size_full())
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
                        // The list opens under the button, at least as wide.
                        let (position, width) = this.unit_bounds
                            .map_or((event.position, px(0.0)), |bounds| (crate::ui::components::select_menu::list_origin(bounds), bounds.size.width));
                        this.context_menus.push(ContextMenuInstance {
            native_bounds: None,
                            position,
                            content: ContextMenuContent::Items(items, None),
                            scroll_handle: UniformListScrollHandle::new(),
                            width,
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
                    fields = fields.child("–");
                }
                // Short values keep a width fitting them; stacked rows take the full width.
                let mut field = input::field_frame(&d.inputs[index], window, cx)
                    .flex_none()
                    .w(if start == 5 { rems(9.5) } else { rems(5.0) })
                    .when(compact, |field| field.w_full())
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
                        input::field_button(
                            SharedString::from(format!("advanced-calendar-{index}")),
                            FieldEdge::Trailing,
                            false,
                            &theme,
                        )
                            .track_focus(&d.controls[if index == 5 { 9 } else { 10 }])
                            .child(input::field_button_icon("icons/calendar-days.svg", false, &theme))
                            // On press, as the unit's list: a second press closes it.
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                h.update(cx, |this, cx| {
                                    let dismissed = this.calendar_dismissed.take().is_some_and(|(shown, at)| {
                                        shown == index && at.elapsed() < std::time::Duration::from_millis(300)
                                    });
                                    let open = this.advanced_search.as_ref().is_some_and(|d| d.calendar.is_some_and(|(shown, _)| shown == index));
                                    if open {
                                        if let Some(d) = &mut this.advanced_search {
                                            d.calendar = None;
                                        }
                                    } else if !dismissed {
                                        this.open_calendar(index);
                                    }
                                    cx.notify();
                                });
                            }),
                    );
                    let measure = handle.clone();
                    // The picker opens under the whole field, as a list under its button.
                    field = field.relative().child(
                        canvas(|bounds, _, _| bounds, move |bounds, _, _, cx| {
                            measure.update(cx, |this, _| {
                                if let Some(d) = &mut this.advanced_search {
                                    d.calendar_anchors[index - 5] = Some(bounds);
                                }
                            })
                        })
                        .absolute()
                        .inset_0(),
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
        body
    }

    fn advanced_text_field(
        &self,
        index: usize,
        placeholder: Message,
        handle: Entity<Self>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let d = self.advanced_search.as_ref().unwrap();
        let theme = *cx.theme();
        d.inputs[index].update(cx, |input, cx| {
            input.set_placeholder(cx.i18n().text(placeholder), cx)
        });
        let regex = match index {
            0 => Some(0),
            2 => Some(1),
            _ => None,
        };
        let field = input::field_frame(&d.inputs[index], window, cx);
        let field = match regex {
            Some(flag) => field.child(self.advanced_toggle(flag, Message::AdvancedUseRegex, true, handle, window, cx)),
            None => field.child(input::field_icon("icons/folder.svg", FieldEdge::Leading, &theme)),
        };
        let mono = regex.is_some_and(|flag| d.flags[flag]);
        field.child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .font_family(if mono { "JetBrains Mono" } else { "Inter" })
                .child(d.inputs[index].clone()),
        )
    }

    fn advanced_toggles(
        &self,
        toggles: [(usize, Message); 2],
        handle: Entity<Self>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut row = div().flex().flex_wrap().gap_x(rems(1.25)).gap_y(rems(0.5));
        for (index, message) in toggles {
            row = row.child(self.advanced_toggle(index, message, false, handle.clone(), window, cx));
        }
        row
    }

    fn advanced_toggle(
        &self,
        index: usize,
        message: Message,
        regex: bool,
        handle: Entity<Self>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = *cx.theme();
        let checked = self.advanced_search.as_ref().unwrap().flags[index];
        if regex {
            return input::field_option(
                SharedString::from(format!("advanced-flag-{index}")),
                FieldEdge::Leading,
                checked,
                &theme,
            )
                .track_focus(&self.advanced_search.as_ref().unwrap().controls[index])
                .child(input::field_button_icon("icons/regex.svg", checked, &theme))
                .fixed_tooltip(crate::ui::components::tooltip::Tooltip::text(
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
        let focus = &self.advanced_search.as_ref().unwrap().controls[index];
        let ring = crate::ui::elements::focus_ring::keyboard_focused(focus, window);
        div()
            .id(SharedString::from(format!("advanced-flag-{index}")))
            .flex()
            .items_center()
            .gap(rems(0.5))
            .track_focus(focus)
            .child(crate::ui::elements::focus_ring::focus_ring(
                crate::ui::elements::checkbox::checkbox(theme, checked, false),
                ring,
                false,
                theme,
            ))
            .child(cx.i18n().text(message))
            .fixed_tooltip(crate::ui::components::tooltip::Tooltip::text(
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
