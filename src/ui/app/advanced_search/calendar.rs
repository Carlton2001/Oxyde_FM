//! The date picker of a date field, a popup under it as GTK's calendar popover: the
//! month between its arrows, the days of the week, then six weeks. Arrows move the day,
//! Page Up and Page Down the month, Enter or a click picks it, Escape or a click
//! outside closes it.
use super::AdvancedSearchWindow;
use crate::ui::{
    components::{popup, select_menu},
    i18n::{I18nContext, Message},
    theme::{OxydeTheme, ThemeContext},
};
use chrono::{Datelike, NaiveDate};
use gpui::prelude::*;
use gpui::*;

/// A day's cell, square, as wide as a weekday's label.
const CELL: Rems = rems(2.0);
const CELL_RADIUS: Rems = rems(0.3125);

impl AdvancedSearchWindow {
    /// Opens the picker of date field `index` on its date, today when it holds none.
    pub(super) fn open_calendar(&mut self, index: usize) {
        let Some(d) = &mut self.advanced_search else { return };
        let date = NaiveDate::parse_from_str(&d.values[index], "%Y-%m-%d")
            .unwrap_or_else(|_| chrono::Local::now().date_naive());
        d.calendar = Some((index, date));
    }

    /// The date under the keyboard moves by `days`, or by whole months.
    pub(super) fn move_calendar(&mut self, days: i64, months: i32) {
        let Some((_, date)) = self.advanced_search.as_mut().and_then(|d| d.calendar.as_mut()) else { return };
        let moved = match months {
            0 => date.checked_add_signed(chrono::Duration::days(days)),
            m if m < 0 => date.checked_sub_months(chrono::Months::new(m.unsigned_abs())),
            m => date.checked_add_months(chrono::Months::new(m as u32)),
        };
        *date = moved.unwrap_or(*date);
    }

    /// Writes `date` in its field and closes the picker.
    pub(super) fn pick_calendar(&mut self, date: NaiveDate, cx: &mut Context<Self>) {
        let Some(d) = &mut self.advanced_search else { return };
        let Some((index, _)) = d.calendar.take() else { return };
        let value = date.format("%Y-%m-%d").to_string();
        d.values[index] = value.clone();
        d.inputs[index].update(cx, |input, cx| input.set_content(value, cx));
        cx.notify();
    }

    fn close_calendar(&mut self, cx: &mut Context<Self>) {
        if let Some((index, _)) = self.advanced_search.as_mut().and_then(|d| d.calendar.take()) {
            self.calendar_dismissed = Some((index, std::time::Instant::now()));
        }
        cx.notify();
    }

    /// Calendar keys; true when one was handled.
    pub(super) fn calendar_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some((_, date)) = self.advanced_search.as_ref().and_then(|d| d.calendar) else { return false };
        match key {
            "left" => self.move_calendar(-1, 0),
            "right" => self.move_calendar(1, 0),
            "up" => self.move_calendar(-7, 0),
            "down" => self.move_calendar(7, 0),
            "pageup" => self.move_calendar(0, -1),
            "pagedown" => self.move_calendar(0, 1),
            "enter" | "space" => {
                self.pick_calendar(date, cx);
                return true;
            }
            "escape" => self.close_calendar(cx),
            _ => return false,
        }
        cx.notify();
        true
    }

    pub(super) fn render_advanced_calendar(
        &self,
        mut layer: Div,
        handle: Entity<Self>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let Some(d) = self.advanced_search.as_ref() else { return layer };
        let Some((index, _)) = d.calendar else { return layer };
        let anchor = d.calendar_anchors[index - 5].unwrap_or_default();
        if popup::supported(window, cx) {
            // One surface per field: closing one never closes the other's.
            let shown = move |view: &Self| view.advanced_search.as_ref().is_some_and(|d| d.calendar.is_some_and(|(open, _)| open == index));
            popup::show(format!("advanced-calendar-{index}"), popup::PopupAnchor::element(anchor), handle, window, cx,
                shown,
                |view, window, cx| view.calendar(cx.entity(), Some(window), cx).into_any_element(),
                move |view, _, cx| if shown(view) { view.close_calendar(cx) });
            return layer;
        }
        // Without popup windows (X11), the window's own keys drive it.
        let close = handle.clone();
        let surface = self.calendar(handle, None, cx).on_mouse_down_out(move |event, _, cx| {
            close.update(cx, |this, cx| {
                if !anchor.contains(&event.position) {
                    this.close_calendar(cx);
                }
            })
        });
        layer = layer.child(deferred(
            anchored().position(select_menu::list_origin(anchor)).snap_to_window().child(surface),
        ).with_priority(1));
        layer
    }

    /// The picker's surface. In its own window, it takes the keyboard there.
    fn calendar(&self, handle: Entity<Self>, window: Option<&mut Window>, cx: &mut Context<Self>) -> Div {
        let theme = *cx.theme();
        let Some((d, (_, date))) = self.advanced_search.as_ref().and_then(|d| Some((d, d.calendar?))) else { return div() };
        let mut surface = select_menu::surface(theme).p(rems(0.5)).gap(rems(0.25));
        if let Some(window) = window {
            if !d.calendar_focus.is_focused(window) {
                window.focus(&d.calendar_focus, cx);
            }
            let keys = handle.clone();
            surface = surface.track_focus(&d.calendar_focus).on_key_down(move |event, _, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                if keys.update(cx, |this, cx| this.calendar_key(&event.keystroke.key, cx)) {
                    cx.stop_propagation();
                }
            });
        }
        let months: Vec<SharedString> = cx.i18n().text(Message::AdvancedMonths).split_whitespace().map(|month| month.to_string().into()).collect();
        let month = months.get(date.month0() as usize).cloned().unwrap_or_default();
        let navigation = div()
            .flex()
            .items_center()
            .justify_between()
            .child(step("calendar-previous", "icons/chevron-left.svg", -1, &handle, theme))
            .child(div().font_weight(FontWeight::MEDIUM).child(format!("{month} {}", date.year())))
            .child(step("calendar-next", "icons/chevron-right.svg", 1, &handle, theme));
        let mut weekdays = div().flex();
        for day in cx.i18n().text(Message::AdvancedWeekdays).split_whitespace() {
            weekdays = weekdays.child(
                div().w(CELL).h(rems(1.5)).flex().items_center().justify_center()
                    .text_size(rems(0.75)).text_color(theme.text_muted).child(day.to_string()),
            );
        }
        surface = surface.child(navigation).child(weekdays);
        let first = date.with_day(1).unwrap_or(date);
        let offset = first.weekday().num_days_from_monday() as i64;
        let today = chrono::Local::now().date_naive();
        for week in 0..6 {
            let mut row = div().flex();
            for weekday in 0..7 {
                let day = first + chrono::Duration::days(week * 7 + weekday - offset);
                let pick = handle.clone();
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
                        .on_click(move |_, _, cx| pick.update(cx, |this, cx| this.pick_calendar(day, cx))),
                );
            }
            surface = surface.child(row);
        }
        surface
    }
}

/// A month back or forward, a square cell beside the month's name.
fn step(id: &'static str, icon: &'static str, months: i32, handle: &Entity<AdvancedSearchWindow>, theme: OxydeTheme) -> Stateful<Div> {
    let handle = handle.clone();
    div()
        .id(id)
        .size(CELL)
        .flex()
        .items_center()
        .justify_center()
        .rounded(CELL_RADIUS)
        .hover(|cell| cell.bg(theme.menu_hover))
        .child(svg().path(icon).size(rems(1.0)).text_color(theme.text))
        .on_click(move |_, _, cx| handle.update(cx, |this, cx| {
            this.move_calendar(0, months);
            cx.notify();
        }))
}
