use super::{CalendarFilterRequest, CalendarFilterResult, FileFilters, LayoutNode, SizeCategory, TabState};
mod criteria;
pub use criteria::{DateRange, FilterColumn, FilterValue, SizeRange};
pub(crate) use criteria::NameMatcher;
use crate::ui::date_groups::{CalendarReference, DateGroup};
use crate::ui::i18n::Message;
use oxyde_lib::models::FileEntry;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod calendar_refresh_tests;
#[cfg(test)]
#[path = "filters/tests/metadata_indicators.rs"]
mod metadata_indicator_tests;

#[cfg(test)]
#[path = "filters/tests/trash_search.rs"]
mod trash_search_tests;

#[cfg(test)]
#[path = "filters/tests/criteria.rs"]
mod criteria_tests;

/// Lit header indicators, one per filterable column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ColumnFilterFlags {
    pub name: bool,
    pub location: bool,
    pub types: bool,
    pub sizes: bool,
    pub permissions: bool,
    pub dates: bool,
    pub deleted: bool,
}

impl SizeCategory {
    pub const ALL: [Self; 5] = [Self::Minuscule, Self::Petit, Self::Moyen, Self::Grand, Self::Huge];

    pub fn message(&self) -> Message {
        match self {
            Self::Minuscule => Message::SizeTiny,
            Self::Petit => Message::SizeSmall,
            Self::Moyen => Message::SizeMedium,
            Self::Grand => Message::SizeLarge,
            Self::Huge => Message::SizeHuge,
        }
    }

    pub fn matches(&self, size: u64) -> bool {
        let kb = 1024;
        let mb = 1024 * kb;
        let gb = 1024 * mb;
        match self {
            Self::Minuscule => size < 10 * kb,
            Self::Petit => size >= 10 * kb && size < mb,
            Self::Moyen => size >= mb && size < 100 * mb,
            Self::Grand => size >= 100 * mb && size < gb,
            Self::Huge => size >= gb,
        }
    }
}

impl FileFilters {
    pub(super) fn from_tab(tab: &TabState) -> Self {
        Self {
            filter_types: tab.filter_types.clone(),
            filter_name: tab.filter_name.clone(),
            filter_name_regex: tab.filter_name_regex,
            trash_search: if tab.path == "trash://" { tab.search_text.trim().to_lowercase() } else { String::new() },
            filter_original_location: tab.filter_original_location.clone(),
            filter_sizes: tab.filter_sizes.clone(),
            filter_size_range: tab.filter_size_range,
            filter_permissions: tab.filter_permissions.clone(),
            filter_dates: tab.filter_dates.clone(),
            filter_date_range: tab.filter_date_range,
            filter_deleted: tab.filter_deleted.clone(),
            filter_deleted_range: tab.filter_deleted_range,
        }
    }
    fn is_empty(&self) -> bool {
        self.filter_types.is_empty()
            && self.filter_name.is_empty()
            && self.trash_search.is_empty()
            && self.filter_original_location.is_empty()
            && self.filter_sizes.is_empty()
            && self.filter_size_range.is_none()
            && self.filter_permissions.is_empty()
            && self.filter_dates.is_empty()
            && self.filter_date_range.is_none()
            && self.filter_deleted.is_empty()
            && self.filter_deleted_range.is_none()
    }
    pub(super) fn snapshot(&self, files: &std::sync::Arc<Vec<FileEntry>>, today: chrono::NaiveDate) -> std::sync::Arc<Vec<FileEntry>> {
        if self.is_empty() { files.clone() } else { self.filtered_files_on(files, today).into() }
    }
    pub(super) fn filtered_files_on(&self, files: &[FileEntry], today: chrono::NaiveDate) -> Vec<FileEntry> {
        if self.is_empty() {
            return files.to_vec();
        }
        let compiled = self.compile(today);
        files.iter().filter(|file| compiled.matches(file, None)).cloned().collect()
    }
    fn compile(&self, today: chrono::NaiveDate) -> CompiledFilters<'_> {
        CompiledFilters {
            filters: self,
            name: NameMatcher::new(&self.filter_name, self.filter_name_regex),
            search: self.trash_search.contains(['*', '?']).then(|| glob::Pattern::new(&self.trash_search)),
            location: self.filter_original_location.to_lowercase(),
            today,
        }
    }
}

/// Filters ready to test entries, their patterns compiled once.
struct CompiledFilters<'a> {
    filters: &'a FileFilters,
    name: NameMatcher,
    search: Option<Result<glob::Pattern, glob::PatternError>>,
    location: String,
    today: chrono::NaiveDate,
}

impl CompiledFilters<'_> {
    /// Whether `f` passes every filter but the one of `except`, whose values are being counted.
    fn matches(&self, f: &FileEntry, except: Option<FilterColumn>) -> bool {
        let filters = self.filters;
        let kept = |column| except != Some(column);
        let matches_type = !kept(FilterColumn::Type) || filters.filter_types.is_empty()
            || filters.filter_types.contains(&crate::ui::components::file_list::file_type_label(f));
        let matches_name = !kept(FilterColumn::Name) || self.name.matches(&f.name);
        let matches_size = !kept(FilterColumn::Size)
            || ((filters.filter_sizes.is_empty() || filters.filter_sizes.iter().any(|cat| cat.matches(f.size)))
                && filters.filter_size_range.is_none_or(|range| range.contains(f.size)));
        let matches_permissions = !kept(FilterColumn::Permissions) || filters.filter_permissions.is_empty()
            || f.mode.is_some_and(|mode| filters.filter_permissions.contains(&mode));
        let matches_date = !kept(FilterColumn::Date)
            || ((filters.filter_dates.is_empty() || filters.filter_dates.contains(&DateGroup::for_timestamp(f.modified, self.today)))
                && filters.filter_date_range.is_none_or(|range| range.contains(f.modified)));
        let matches_deleted = !kept(FilterColumn::Deleted)
            || (filters.filter_deleted.is_empty() && filters.filter_deleted_range.is_none())
            || deleted_ms(f).is_some_and(|ms| {
                (filters.filter_deleted.is_empty() || filters.filter_deleted.contains(&DateGroup::for_timestamp(ms, self.today)))
                    && filters.filter_deleted_range.is_none_or(|range| range.contains(ms))
            });
        // In the trash, where an entry was deleted from; in search results, its folder.
        let matches_location = !kept(FilterColumn::Location) || self.location.is_empty()
            || std::path::Path::new(f.original_path.as_deref().unwrap_or(&f.path))
                .parent()
                .is_some_and(|path| path.to_string_lossy().to_lowercase().contains(&self.location));
        let matches_search = match &self.search {
            Some(Ok(pattern)) => pattern.matches(&f.name.to_lowercase()),
            Some(Err(_)) => false,
            None => filters.trash_search.is_empty() || f.name.to_lowercase().contains(&filters.trash_search),
        };
        matches_type && matches_name && matches_search && matches_size && matches_permissions && matches_date && matches_deleted && matches_location
    }
}

/// When `f` went to the trash, in milliseconds since the epoch.
pub(crate) fn deleted_ms(f: &FileEntry) -> Option<u64> {
    f.deleted_time.and_then(|time| u64::try_from(time).ok())
}

impl CalendarFilterRequest {
    pub fn calculate(self) -> CalendarFilterResult {
        let files = self.filters.snapshot(&self.files, self.reference.today);
        CalendarFilterResult {
            tab_id: self.tab_id,
            revision: self.revision,
            reference: self.reference,
            files,
        }
    }
}

impl TabState {
    /// Which header indicators are lit. Checking every value present filters nothing, so it
    /// lights none; a range or a typed text always does.
    pub(crate) fn column_filter_flags(&self) -> ColumnFilterFlags {
        let permissions = !self.filter_permissions.is_empty() && self.files.iter()
            .any(|file| file.mode.is_some_and(|mode| !self.filter_permissions.contains(&mode)));
        let types = !self.filter_types.is_empty() && self.files.iter().any(|file| {
            !self.filter_types.contains(&crate::ui::components::file_list::file_type_label(file))
        });
        let sizes = self.filter_size_range.is_some() || !self.filter_sizes.is_empty() && self.files.iter().any(|file| {
            SizeCategory::ALL.iter().find(|category| category.matches(file.size))
                .is_some_and(|category| !self.filter_sizes.contains(category))
        });
        let dates = self.filter_date_range.is_some() || !self.filter_dates.is_empty() &&
            crate::ui::date_groups::counts_by_date(&self.files, chrono::Local::now().date_naive())
                .keys().any(|group| !self.filter_dates.contains(group));
        let deleted = self.filter_deleted_range.is_some() || !self.filter_deleted.is_empty() && {
            let today = chrono::Local::now().date_naive();
            self.files.iter().filter_map(deleted_ms).any(|ms| !self.filter_deleted.contains(&DateGroup::for_timestamp(ms, today)))
        };
        ColumnFilterFlags {
            name: !self.filter_name.is_empty(),
            location: !self.filter_original_location.is_empty(),
            types,
            sizes,
            permissions,
            dates,
            deleted,
        }
    }

    /// Whether any column filter is set, even one that keeps every entry.
    pub(crate) fn has_column_filters(&self) -> bool {
        !self.filter_name.is_empty()
            || !self.filter_original_location.is_empty()
            || !self.filter_types.is_empty()
            || !self.filter_sizes.is_empty()
            || self.filter_size_range.is_some()
            || !self.filter_permissions.is_empty()
            || !self.filter_dates.is_empty()
            || self.filter_date_range.is_some()
            || !self.filter_deleted.is_empty()
            || self.filter_deleted_range.is_some()
    }

    pub(crate) fn clear_column_filter(&mut self, column: FilterColumn) {
        match column {
            FilterColumn::Name => self.filter_name.clear(),
            FilterColumn::Location => self.filter_original_location.clear(),
            FilterColumn::Type => self.filter_types.clear(),
            FilterColumn::Size => {
                self.filter_sizes.clear();
                self.filter_size_range = None;
            }
            FilterColumn::Permissions => self.filter_permissions.clear(),
            FilterColumn::Date => {
                self.filter_dates.clear();
                self.filter_date_range = None;
            }
            FilterColumn::Deleted => {
                self.filter_deleted.clear();
                self.filter_deleted_range = None;
            }
        }
    }

    pub(crate) fn is_value_checked(&self, value: &FilterValue) -> bool {
        match value {
            FilterValue::Type(label) => self.filter_types.contains(label),
            FilterValue::Size(category) => self.filter_sizes.contains(category),
            FilterValue::Date(group) => self.filter_dates.contains(group),
            FilterValue::Deleted(group) => self.filter_deleted.contains(group),
            FilterValue::Permissions(mode) => self.filter_permissions.contains(mode),
        }
    }

    /// Checks or unchecks `value`. Checked values replace a custom range of their column.
    pub(crate) fn set_value_checked(&mut self, value: FilterValue, checked: bool) {
        fn set<T: std::hash::Hash + Eq>(values: &mut std::collections::HashSet<T>, value: T, checked: bool) {
            if checked { values.insert(value); } else { values.remove(&value); }
        }
        match value {
            FilterValue::Type(label) => set(&mut self.filter_types, label, checked),
            FilterValue::Size(category) => {
                self.filter_size_range = None;
                set(&mut self.filter_sizes, category, checked);
            }
            FilterValue::Date(group) => {
                self.filter_date_range = None;
                set(&mut self.filter_dates, group, checked);
            }
            FilterValue::Deleted(group) => {
                self.filter_deleted_range = None;
                set(&mut self.filter_deleted, group, checked);
            }
            FilterValue::Permissions(mode) => set(&mut self.filter_permissions, mode, checked),
        }
    }

    /// A custom size range replaces the checked sizes; an open one removes the filter.
    pub(crate) fn set_size_range(&mut self, range: SizeRange) {
        self.filter_sizes.clear();
        self.filter_size_range = (!range.is_open()).then_some(range);
    }

    /// Custom days of `column`, Date or Deleted, replace its checked periods; an open range
    /// removes the filter.
    pub(crate) fn set_date_range(&mut self, column: FilterColumn, range: DateRange) {
        let range = (!range.is_open()).then_some(range);
        if column == FilterColumn::Deleted {
            self.filter_deleted.clear();
            self.filter_deleted_range = range;
        } else {
            self.filter_dates.clear();
            self.filter_date_range = range;
        }
    }

    pub(crate) fn clear_column_filters(&mut self) {
        for column in [FilterColumn::Name, FilterColumn::Location, FilterColumn::Type, FilterColumn::Size, FilterColumn::Permissions, FilterColumn::Date, FilterColumn::Deleted] {
            self.clear_column_filter(column);
        }
    }

    /// Visits every file once, telling whether the other filters keep it, to count the values
    /// of `column` as a spreadsheet does: a value hidden by another column counts zero.
    pub(crate) fn visit_all_faceted(&self, column: FilterColumn, today: chrono::NaiveDate, mut visit: impl FnMut(&FileEntry, bool)) {
        let filters = FileFilters::from_tab(self);
        let compiled = filters.compile(today);
        for file in self.files.iter() {
            visit(file, compiled.matches(file, Some(column)));
        }
    }

    /// Changes whenever the files or their filters do; keys caches of faceted counts.
    pub(crate) fn filter_revision(&self) -> (usize, u64) {
        (std::sync::Arc::as_ptr(&self.files) as usize, self.filter_cache_revision)
    }

    pub fn update_cache(&mut self) {
        self.update_cache_on(chrono::Local::now().date_naive());
    }

    fn update_cache_on(&mut self, today: chrono::NaiveDate) {
        self.presentation_cache.get_mut().take();
        let mut reference = CalendarReference::current();
        reference.today = today;
        self.cached_filtered_files =
            FileFilters::from_tab(self).snapshot(&self.files, today);
        self.calendar_reference = Some(reference);
        self.filter_cache_revision = self.filter_cache_revision.wrapping_add(1);
        self.recalculate_column_widths();
    }

    fn calendar_filter_request(
        &self,
        reference: &CalendarReference,
    ) -> Option<CalendarFilterRequest> {
        if (self.filter_dates.is_empty() && self.filter_deleted.is_empty()) || self.calendar_reference.as_ref() == Some(reference) {
            return None;
        }
        Some(CalendarFilterRequest {
            tab_id: self.id.clone(),
            revision: self.filter_cache_revision,
            reference: reference.clone(),
            files: self.files.clone(),
            filters: FileFilters::from_tab(self),
        })
    }

    fn apply_calendar_filter(&mut self, result: CalendarFilterResult) -> bool {
        if self.id != result.tab_id || self.filter_cache_revision != result.revision {
            return false;
        }
        self.cached_filtered_files = result.files;
        self.presentation_cache.get_mut().take();
        self.calendar_reference = Some(result.reference);
        self.filter_cache_revision = self.filter_cache_revision.wrapping_add(1);
        true
    }

}

impl LayoutNode {
    pub(crate) fn calendar_filter_requests(
        &self,
        reference: &CalendarReference,
    ) -> Vec<CalendarFilterRequest> {
        let mut requests = Vec::new();
        self.visit_panes(&mut |pane| {
            for tab in &pane.tabs {
                if let Some(request) = tab.calendar_filter_request(reference) {
                    requests.push(request);
                }
            }
        });
        requests
    }

    pub(crate) fn apply_calendar_filters(&mut self, results: Vec<CalendarFilterResult>) -> bool {
        let mut changed = false;
        for result in results {
            let mut result = Some(result);
            self.visit_panes_mut(&mut |pane| {
                for tab in &mut pane.tabs {
                    if result.as_ref().is_some_and(|r| r.tab_id == tab.id) {
                        changed |= tab.apply_calendar_filter(result.take().unwrap());
                        break;
                    }
                }
            });
        }
        changed
    }

    pub(crate) fn has_date_groups(&self) -> bool {
        let mut grouped = false;
        self.visit_panes(&mut |pane| grouped |= pane.tabs.iter().any(|tab| tab.grouping().is_calendar()));
        grouped
    }

}
