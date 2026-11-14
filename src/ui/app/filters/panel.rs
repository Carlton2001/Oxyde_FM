//! The filter panel of a details column, opened from its header: its fields, the custom
//! range of Size and Date, and the values it lists.
use crate::ui::app::{ContextMenuContent, ContextMenuInstance, OxydeApp};
use crate::ui::components::file_list::FilterEntry;
use crate::ui::components::input::{TextInput, TextInputOptions};
use crate::ui::format::DateFormat;
use crate::ui::i18n::{I18n, I18nContext, Language, Message};
use crate::ui::workspace::{DateRange, FilterColumn, SizeRange, TabState};
use chrono::NaiveDate;
use gpui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[cfg(test)]
#[path = "tests/panel.rs"]
mod tests;

/// The Type list takes a search field past this many values.
pub(crate) const TYPE_SEARCH_THRESHOLD: usize = 8;

type EntriesKey = ((usize, u64), Language);

#[derive(Clone)]
pub(crate) struct ColumnFilterPanel {
    pub pane_id: String,
    pub column: FilterColumn,
    /// The name or location text, or the search among Type values.
    pub field: Option<Entity<TextInput>>,
    /// Custom bounds of Size and Date.
    pub range: Option<RangeEditor>,
    /// The field took the keyboard once; it then follows the user's clicks.
    pub focused: Rc<Cell<bool>>,
    /// Values counted for one state of the tab, kept while it does not change.
    entries: Rc<RefCell<Option<(EntriesKey, Rc<Vec<FilterEntry>>)>>>,
}

#[derive(Clone)]
pub(crate) struct RangeEditor {
    /// The fields are shown, from « Custom… » or because a range is set.
    pub open: bool,
    pub inputs: [Entity<TextInput>; 2],
    /// Texts of both fields, kept here: a field cannot be read while it reports a change.
    pub texts: [String; 2],
    /// Size unit as a power of 1024: 1 for KB, 2 for MB, 3 for GB.
    pub unit: u32,
    /// The calendar open under field 0 or 1, on its shown day.
    pub calendar: Option<(usize, NaiveDate)>,
}

impl ColumnFilterPanel {
    /// The values of the column for the tab's current files and filters.
    pub(crate) fn entries(&self, tab: &TabState, i18n: &I18n) -> Rc<Vec<FilterEntry>> {
        let key = (tab.filter_revision(), i18n.language());
        if let Some((cached, entries)) = self.entries.borrow().as_ref() {
            if *cached == key {
                return entries.clone();
            }
        }
        let entries = Rc::new(crate::ui::components::file_list::filter_entries(tab, self.column, i18n));
        *self.entries.borrow_mut() = Some((key, entries.clone()));
        entries
    }
}

impl RangeEditor {
    /// Bound `index` in bytes: `Ok(None)` when empty, `Err` when it does not read as a size.
    pub(crate) fn size_bound(&self, index: usize) -> Result<Option<u64>, ()> {
        crate::ui::format::size_in_bytes(&self.texts[index], self.unit as usize)
    }

    pub(crate) fn date_bound(&self, index: usize, format: DateFormat) -> Result<Option<NaiveDate>, ()> {
        let text = self.texts[index].trim();
        if text.is_empty() {
            return Ok(None);
        }
        crate::ui::format::parse_day(text, format).map(Some).ok_or(())
    }

    pub(crate) fn size_range(&self) -> Option<SizeRange> {
        Some(SizeRange { min: self.size_bound(0).ok()?, max: self.size_bound(1).ok()? })
    }

    pub(crate) fn date_range(&self, format: DateFormat) -> Option<DateRange> {
        Some(DateRange { from: self.date_bound(0, format).ok()?, to: self.date_bound(1, format).ok()? })
    }
}

/// The largest unit, up to GB, in which the bounds read as at least one.
fn size_unit(range: &SizeRange) -> u32 {
    let largest = range.min.max(range.max).unwrap_or(0);
    (1..=3).rev().find(|unit| largest >= 1024u64.pow(*unit)).unwrap_or(if largest == 0 { 2 } else { 1 })
}

/// `bytes` in units of 1024^`unit`, to two decimals at most, with the language's separator.
fn size_text(bytes: Option<u64>, unit: u32, language: Language) -> String {
    let Some(bytes) = bytes else { return String::new() };
    let hundredths = (u128::from(bytes) * 100 + u128::from(1024u64.pow(unit)) / 2) / u128::from(1024u64.pow(unit));
    let text = format!("{}.{:02}", hundredths / 100, hundredths % 100);
    let text = text.trim_end_matches('0').trim_end_matches('.').to_string();
    if language == Language::English { text } else { text.replace('.', ",") }
}

impl OxydeApp {
    pub(crate) fn filter_panel(&self) -> Option<&ColumnFilterPanel> {
        self.context_menus.iter().find_map(|menu| match &menu.content {
            ContextMenuContent::ColumnFilter(panel) => Some(panel.as_ref()),
            _ => None,
        })
    }

    fn filter_panel_mut(&mut self) -> Option<&mut ColumnFilterPanel> {
        self.context_menus.iter_mut().find_map(|menu| match &mut menu.content {
            ContextMenuContent::ColumnFilter(panel) => Some(panel.as_mut()),
            _ => None,
        })
    }

    /// Opens the filter panel of `column` under `anchor`, the header cell's bottom left.
    pub(crate) fn open_column_filter(
        &mut self,
        pane_id: &str,
        column: FilterColumn,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(handle) = self.handle.clone() else { return };
        let Some(tab) = self.state.find_pane(pane_id).map(|pane| pane.active_tab()) else { return };
        let field_text = match column {
            FilterColumn::Name => Some((tab.filter_name.clone(), Message::NameFilterPlaceholder)),
            FilterColumn::Location => Some((tab.filter_original_location.clone(), Message::OriginalLocationFilterPlaceholder)),
            FilterColumn::Type => Some((String::new(), Message::FilterSearchTypes)),
            _ => None,
        };
        let format = crate::ui::format::date_format(cx);
        let language = cx.i18n().language();
        let range = match column {
            FilterColumn::Size => {
                let range = tab.filter_size_range.unwrap_or_default();
                let unit = size_unit(&range);
                Some((tab.filter_size_range.is_some(), [size_text(range.min, unit, language), size_text(range.max, unit, language)], unit, [Message::FilterMin, Message::FilterMax]))
            }
            FilterColumn::Date => {
                let range = tab.filter_date_range.unwrap_or_default();
                let day = |date: Option<NaiveDate>| date.map(|date| date.format(crate::ui::format::date_pattern(format)).to_string()).unwrap_or_default();
                Some((tab.filter_date_range.is_some(), [day(range.from), day(range.to)], 2, [Message::FilterFrom, Message::FilterTo]))
            }
            _ => None,
        };

        let field = field_text.map(|(text, placeholder)| {
            let pane = pane_id.to_string();
            let change = handle.clone();
            text_input(handle.clone(), cx.i18n().text(placeholder).to_string(), text, window, cx, move |text, cx| {
                change.update(cx, |this, cx| match column {
                    FilterColumn::Name => this.set_name_filter(&pane, text.to_string(), cx),
                    FilterColumn::Location => this.set_location_filter(&pane, text.to_string(), cx),
                    // The Type search only narrows the list: it filters nothing.
                    _ => cx.notify(),
                });
            })
        });
        let range = range.map(|(open, texts, unit, placeholders)| {
            let inputs = [0, 1].map(|index| {
                let edit = handle.clone();
                text_input(handle.clone(), cx.i18n().text(placeholders[index]).to_string(), texts[index].clone(), window, cx, move |text, cx| {
                    edit.update(cx, |this, cx| this.edit_range_text(index, text.to_string(), cx));
                })
            });
            RangeEditor { open, inputs, texts, unit, calendar: None }
        });

        self.context_menus.clear();
        self.context_menus.push(ContextMenuInstance {
            native_bounds: None,
            position: anchor,
            content: ContextMenuContent::ColumnFilter(Box::new(ColumnFilterPanel {
                pane_id: pane_id.to_string(),
                column,
                field,
                range,
                focused: Rc::new(Cell::new(false)),
                entries: Default::default(),
            })),
            scroll_handle: UniformListScrollHandle::new(),
            width: px(0.0),
            open_left: false,
            hovered_item: None,
            native_request_id: None,
            open_up: false,
        });
        cx.notify();
    }

    /// After a filter is cleared, its fields read empty and the custom range folds.
    pub(super) fn reset_filter_panel(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = self.filter_panel_mut() else { return };
        let mut emptied = Vec::new();
        if matches!(panel.column, FilterColumn::Name | FilterColumn::Location) {
            emptied.extend(panel.field.clone());
        }
        if let Some(range) = &mut panel.range {
            range.open = false;
            range.calendar = None;
            range.texts = Default::default();
            emptied.extend(range.inputs.clone());
        }
        for input in emptied {
            input.update(cx, |input, cx| input.set_content(String::new(), cx));
        }
        cx.notify();
    }

    /// Checked values replace the custom range: its fields read empty again.
    pub(super) fn empty_range_fields(&mut self, cx: &mut Context<Self>) {
        let Some(range) = self.filter_panel_mut().and_then(|panel| panel.range.as_mut()) else { return };
        range.calendar = None;
        range.texts = Default::default();
        for input in range.inputs.clone() {
            input.update(cx, |input, cx| input.set_content(String::new(), cx));
        }
    }

    /// Shows or folds the custom range of the open panel.
    pub(crate) fn toggle_range_editor(&mut self, cx: &mut Context<Self>) {
        if let Some(range) = self.filter_panel_mut().and_then(|panel| panel.range.as_mut()) {
            range.open = !range.open;
            range.calendar = None;
        }
        cx.notify();
    }

    fn edit_range_text(&mut self, index: usize, text: String, cx: &mut Context<Self>) {
        if let Some(range) = self.filter_panel_mut().and_then(|panel| panel.range.as_mut()) {
            range.texts[index] = text;
        }
        self.apply_range(cx);
    }

    pub(crate) fn set_range_unit(&mut self, unit: u32, cx: &mut Context<Self>) {
        if let Some(range) = self.filter_panel_mut().and_then(|panel| panel.range.as_mut()) {
            range.unit = unit;
        }
        self.apply_range(cx);
    }

    /// Applies both bounds once they read; a bound being typed leaves the filter as it was.
    fn apply_range(&mut self, cx: &mut Context<Self>) {
        let format = crate::ui::format::date_format(cx);
        let Some(panel) = self.filter_panel() else { return };
        let Some(range) = &panel.range else { return };
        let pane_id = panel.pane_id.clone();
        match panel.column {
            FilterColumn::Size => if let Some(sizes) = range.size_range() { self.set_size_range(&pane_id, sizes, cx) },
            FilterColumn::Date => if let Some(days) = range.date_range(format) { self.set_date_range(&pane_id, days, cx) },
            _ => {}
        }
        cx.notify();
    }

    /// Opens the calendar under date field `index` on its day, today when it holds none, or
    /// closes it.
    pub(crate) fn toggle_range_calendar(&mut self, index: usize, cx: &mut Context<Self>) {
        let format = crate::ui::format::date_format(cx);
        if let Some(range) = self.filter_panel_mut().and_then(|panel| panel.range.as_mut()) {
            range.calendar = if range.calendar.is_some_and(|(open, _)| open == index) {
                None
            } else {
                let day = range.date_bound(index, format).ok().flatten().unwrap_or_else(|| chrono::Local::now().date_naive());
                Some((index, day))
            };
        }
        cx.notify();
    }

    pub(crate) fn step_range_calendar(&mut self, months: i32, cx: &mut Context<Self>) {
        if let Some((_, day)) = self.filter_panel_mut().and_then(|panel| panel.range.as_mut()).and_then(|range| range.calendar.as_mut()) {
            let moved = if months < 0 {
                day.checked_sub_months(chrono::Months::new(months.unsigned_abs()))
            } else {
                day.checked_add_months(chrono::Months::new(months as u32))
            };
            *day = moved.unwrap_or(*day);
        }
        cx.notify();
    }

    /// Writes the picked day in the field under the calendar, then closes it.
    pub(crate) fn pick_range_date(&mut self, day: NaiveDate, cx: &mut Context<Self>) {
        let text = day.format(crate::ui::format::date_pattern(crate::ui::format::date_format(cx))).to_string();
        let Some(range) = self.filter_panel_mut().and_then(|panel| panel.range.as_mut()) else { return };
        let Some((index, _)) = range.calendar.take() else { return };
        range.texts[index] = text.clone();
        let input = range.inputs[index].clone();
        input.update(cx, |input, cx| input.set_content(text, cx));
        self.apply_range(cx);
    }
}

/// A panel field: Enter and Escape close the panel, its context menu keeps it open.
fn text_input(
    handle: Entity<OxydeApp>,
    placeholder: String,
    text: String,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
    on_change: impl Fn(&str, &mut App) + 'static,
) -> Entity<TextInput> {
    cx.new(|cx| {
        let enter = handle.clone();
        let escape = handle.clone();
        TextInput::new(
            cx,
            placeholder,
            text,
            move |text, _, cx| on_change(text, cx),
            move |_, cx| enter.update(cx, |this, cx| { this.context_menus.clear(); cx.notify(); }),
            move |_, cx| escape.update(cx, |this, cx| { this.context_menus.clear(); cx.notify(); }),
            move |input, has_selection, position, window, cx| {
                handle.update(cx, |this, cx| this.open_text_context_menu(input, has_selection, position, window, cx))
            },
            TextInputOptions {
                appearance: crate::ui::components::input::InputAppearance::Embedded,
                clearable: true,
                ..Default::default()
            },
            window,
        )
    })
}
