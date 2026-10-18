use super::AdvancedSearchWindow;
use crate::ui::{
    components::modal::{action_button, Modal},
    i18n::{I18nContext, Message},
    theme::ThemeContext,
};
use gpui::prelude::*;
use gpui::*;

impl AdvancedSearchWindow {
    pub(super) fn render_advanced_calendar(
        &self,
        mut layer: Div,
        handle: Entity<Self>,
        cx: &mut Context<Self>,
    ) -> Div {
        let d = self.advanced_search.as_ref().unwrap();
        let theme = *cx.theme();
        if let Some((index, month)) = d.calendar {
            use chrono::Datelike;
            let first = month.with_day(1).unwrap();
            let mut calendar = div().flex().flex_col().gap(rems(0.5));
            let mut navigation = div().flex().items_center().justify_between();
            for delta in [-1, 1] {
                let h = handle.clone();
                navigation = navigation.child(
                    action_button(if delta < 0 {
                        "calendar-prev"
                    } else {
                        "calendar-next"
                    })
                    .child(
                        svg()
                            .path(if delta < 0 {
                                "icons/chevron-left.svg"
                            } else {
                                "icons/chevron-right.svg"
                            })
                            .size(rems(1.0))
                            .text_color(theme.text),
                    )
                    .on_click(move |_, _, cx| {
                        h.update(cx, |this, cx| {
                            if let Some(d) = &mut this.advanced_search {
                                if let Some((_, date)) = &mut d.calendar {
                                    let months = chrono::Months::new(1);
                                    *date = if delta < 0 {
                                        date.checked_sub_months(months)
                                    } else {
                                        date.checked_add_months(months)
                                    }
                                    .unwrap_or(*date);
                                }
                            }
                            cx.notify();
                        });
                    }),
                );
                if delta < 0 {
                    navigation = navigation.child(month.format("%Y-%m").to_string());
                }
            }
            calendar = calendar.child(navigation);
            let mut weekdays = div().flex().gap(rems(0.125));
            for day in cx.i18n().text(Message::AdvancedWeekdays).split_whitespace() {
                weekdays = weekdays.child(
                    div()
                        .w(rems(2.1))
                        .text_color(theme.text_muted)
                        .child(day.to_string()),
                );
            }
            calendar = calendar.child(weekdays);
            let offset = first.weekday().num_days_from_monday() as i64;
            for week in 0..6 {
                let mut row = div().flex().gap(rems(0.125));
                for day in 0..7 {
                    let date = first + chrono::Duration::days(week * 7 + day - offset);
                    let h = handle.clone();
                    row = row.child(
                        action_button(SharedString::from(format!("calendar-day-{date}")))
                            .w(rems(2.1))
                            .px(rems(0.25))
                            .text_color(if date.month() == month.month() {
                                theme.text
                            } else {
                                theme.text_muted
                            })
                            .bg(if date == month {
                                theme.selected
                            } else {
                                theme.surface
                            })
                            .hover(move |s| s.bg(theme.control_hover))
                            .child(date.day().to_string())
                            .on_click(move |_, _, cx| {
                                h.update(cx, |this, cx| {
                                    if let Some(d) = &mut this.advanced_search {
                                        let value = date.format("%Y-%m-%d").to_string();
                                        d.values[index] = value.clone();
                                        d.inputs[index]
                                            .update(cx, |input, cx| input.set_content(value, cx));
                                        d.calendar = None;
                                    }
                                    cx.notify();
                                });
                            }),
                    );
                }
                calendar = calendar.child(row);
            }
            let h = handle.clone();
            layer = layer.child(
                Modal::new(
                    "advanced-date-picker",
                    cx.i18n().text(Message::AdvancedDate),
                    calendar,
                )
                .title_icon("icons/calendar-days.svg")
                .width(rems(18.0))
                .action(
                    action_button("calendar-cancel")
                        .child(cx.i18n().text(Message::Cancel))
                        .on_click(move |_, _, cx| {
                            h.update(cx, |this, cx| {
                                if let Some(d) = &mut this.advanced_search {
                                    d.calendar = None;
                                }
                                cx.notify();
                            });
                        }),
                ),
            );
        }
        layer
    }
}
