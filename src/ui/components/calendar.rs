//! A month of days to pick one, as GTK's calendar: the month between its arrows, the days
//! of the week, then six weeks. The caller owns the shown date and the surface around it.
use crate::ui::i18n::{I18n, Message};
use crate::ui::theme::OxydeTheme;
use chrono::{Datelike, NaiveDate};
use gpui::prelude::*;
use gpui::*;
use std::rc::Rc;

/// A day's cell, square, as wide as a weekday's label.
const CELL: Rems = rems(2.0);
const CELL_RADIUS: Rems = rems(0.3125);

pub(crate) type Pick = Rc<dyn Fn(NaiveDate, &mut Window, &mut App)>;
/// Moves the shown month by the given number of months.
pub(crate) type Step = Rc<dyn Fn(i32, &mut Window, &mut App)>;

/// The month of `date`, `date` marked as chosen, today in the accent.
pub(crate) fn month(date: NaiveDate, theme: OxydeTheme, i18n: &I18n, pick: Pick, step: Step) -> Div {
    let months: Vec<SharedString> = i18n.text(Message::AdvancedMonths).split_whitespace().map(|month| month.to_string().into()).collect();
    let month = months.get(date.month0() as usize).cloned().unwrap_or_default();
    let navigation = div()
        .flex()
        .items_center()
        .justify_between()
        .child(arrow("calendar-previous", "icons/chevron-left.svg", -1, &step, theme))
        .child(div().font_weight(FontWeight::MEDIUM).child(format!("{month} {}", date.year())))
        .child(arrow("calendar-next", "icons/chevron-right.svg", 1, &step, theme));
    let mut weekdays = div().flex();
    for day in i18n.text(Message::AdvancedWeekdays).split_whitespace() {
        weekdays = weekdays.child(
            div().w(CELL).h(rems(1.5)).flex().items_center().justify_center()
                .text_size(rems(0.75)).text_color(theme.text_muted).child(day.to_string()),
        );
    }
    let mut grid = div().flex().flex_col().gap(rems(0.25)).child(navigation).child(weekdays);
    let first = date.with_day(1).unwrap_or(date);
    let offset = first.weekday().num_days_from_monday() as i64;
    let today = chrono::Local::now().date_naive();
    for week in 0..6 {
        let mut row = div().flex();
        for weekday in 0..7 {
            let day = first + chrono::Duration::days(week * 7 + weekday - offset);
            let pick = pick.clone();
            let chosen = day == date;
            let outside = day.month() != date.month();
            row = row.child(
                div()
                    .id(SharedString::from(format!("calendar-day-{day}")))
                    .size(CELL)
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(CELL_RADIUS)
                    .text_color(if chosen {
                        theme.text_on_accent
                    } else if outside {
                        theme.text_muted
                    } else if day == today {
                        theme.accent
                    } else {
                        theme.text
                    })
                    .when(day == today, |cell| cell.font_weight(FontWeight::SEMIBOLD))
                    .when(chosen, |cell| cell.bg(theme.accent))
                    .when(!chosen, |cell| cell.hover(|cell| cell.bg(theme.menu_hover)))
                    .child(day.day().to_string())
                    .on_click(move |_, window, cx| pick(day, window, cx)),
            );
        }
        grid = grid.child(row);
    }
    grid
}

/// A month back or forward, a square cell beside the month's name.
fn arrow(id: &'static str, icon: &'static str, months: i32, step: &Step, theme: OxydeTheme) -> Stateful<Div> {
    let step = step.clone();
    div()
        .id(id)
        .size(CELL)
        .flex()
        .items_center()
        .justify_center()
        .rounded(CELL_RADIUS)
        .hover(|cell| cell.bg(theme.menu_hover))
        .child(svg().path(icon).size(rems(1.0)).text_color(theme.text))
        .on_click(move |_, window, cx| step(months, window, cx))
}
