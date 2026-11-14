//! Sorting and the column filters of the details view. The filter panel's state and its
//! opening live in `panel`, its drawing in `context_menu_render::column_filter`.
use crate::ui::app::OxydeApp;
use crate::ui::workspace::{DateRange, FilterColumn, FilterValue, SizeRange, TabState};
use gpui::*;

mod panel;
pub(crate) use panel::{ColumnFilterPanel, RangeEditor, TYPE_SEARCH_THRESHOLD};

impl OxydeApp {
    pub fn toggle_sort(
        &mut self,
        pane_id: &str,
        column: crate::ui::workspace::SortColumn,
        cx: &mut Context<Self>,
    ) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            if tab.sort_column == column {
                tab.sort_direction = match tab.sort_direction {
                    crate::ui::workspace::SortDirection::Ascending => {
                        crate::ui::workspace::SortDirection::Descending
                    }
                    crate::ui::workspace::SortDirection::Descending => {
                        crate::ui::workspace::SortDirection::Ascending
                    }
                };
            } else {
                tab.sort_column = column;
                tab.sort_direction = crate::ui::workspace::SortDirection::Ascending;
            }
            let id = tab.id.clone();
            self.prepare_current_tab(id, cx);
            self.save_workspace(cx);
            cx.notify();
        }
    }

    /// Applies `change` to the filters of the active tab of `pane_id`, then filters it again
    /// off the UI thread.
    fn change_filters(&mut self, pane_id: &str, cx: &mut Context<Self>, change: impl FnOnce(&mut TabState)) {
        let Some(pane) = self.state.find_pane_mut(pane_id) else { return };
        let tab = pane.active_tab_mut();
        change(tab);
        let id = tab.id.clone();
        self.prepare_current_tab(id, cx);
        cx.notify();
    }

    pub(crate) fn toggle_filter_value(&mut self, pane_id: &str, value: FilterValue, cx: &mut Context<Self>) {
        self.change_filters(pane_id, cx, |tab| {
            let checked = tab.is_value_checked(&value);
            tab.set_value_checked(value, !checked);
        });
        self.empty_range_fields(cx);
    }

    /// Checks every value of `values`, or unchecks them when `checked` is false.
    pub(crate) fn set_filter_values(&mut self, pane_id: &str, values: Vec<FilterValue>, checked: bool, cx: &mut Context<Self>) {
        self.change_filters(pane_id, cx, |tab| {
            for value in values {
                tab.set_value_checked(value, checked);
            }
        });
        self.empty_range_fields(cx);
    }

    pub(crate) fn set_name_filter(&mut self, pane_id: &str, text: String, cx: &mut Context<Self>) {
        self.change_filters(pane_id, cx, |tab| tab.filter_name = text);
    }

    pub(crate) fn toggle_name_regex(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        self.change_filters(pane_id, cx, |tab| tab.filter_name_regex = !tab.filter_name_regex);
    }

    pub(crate) fn set_location_filter(&mut self, pane_id: &str, text: String, cx: &mut Context<Self>) {
        self.change_filters(pane_id, cx, |tab| tab.filter_original_location = text);
    }

    pub(crate) fn set_size_range(&mut self, pane_id: &str, range: SizeRange, cx: &mut Context<Self>) {
        self.change_filters(pane_id, cx, |tab| tab.set_size_range(range));
    }

    pub(crate) fn set_date_range(&mut self, pane_id: &str, column: FilterColumn, range: DateRange, cx: &mut Context<Self>) {
        self.change_filters(pane_id, cx, |tab| tab.set_date_range(column, range));
    }

    pub(crate) fn toggle_keep_filters(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            tab.keep_filters = !tab.keep_filters;
            cx.notify();
        }
    }

    /// Removes the filter of `column`, and empties the open panel's fields with it.
    pub(crate) fn clear_column_filter(&mut self, pane_id: &str, column: FilterColumn, cx: &mut Context<Self>) {
        self.change_filters(pane_id, cx, |tab| tab.clear_column_filter(column));
        self.reset_filter_panel(cx);
    }

    /// Removes every column filter of the active tab of `pane_id`.
    pub(crate) fn clear_column_filters(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        self.change_filters(pane_id, cx, TabState::clear_column_filters);
        self.reset_filter_panel(cx);
    }
}
