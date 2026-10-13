use super::{CalendarFilterRequest, CalendarFilterResult, FileFilters, LayoutNode, SizeCategory, TabState};
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

impl SizeCategory {
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
    fn from_tab(tab: &TabState) -> Self {
        Self {
            filter_types: tab.filter_types.clone(),
            filter_name: tab.filter_name.clone(),
            filter_original_location: tab.filter_original_location.clone(),
            filter_sizes: tab.filter_sizes.clone(),
            filter_dates: tab.filter_dates.clone(),
        }
    }
    fn filtered_files_on(&self, files: &[FileEntry], today: chrono::NaiveDate) -> Vec<FileEntry> {
        if !self.filter_types.is_empty()
            || !self.filter_name.is_empty()
            || !self.filter_original_location.is_empty()
            || !self.filter_sizes.is_empty()
            || !self.filter_dates.is_empty()
        {
            let name_filter = self.filter_name.to_lowercase();
            let location_filter = self.filter_original_location.to_lowercase();
            files
                .iter()
                .filter(|f| {
                    let matches_type = if self.filter_types.is_empty() {
                        true
                    } else {
                        if f.is_dir {
                            self.filter_types.contains("Dossier")
                        } else {
                            std::path::Path::new(&f.name)
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|s| self.filter_types.contains(&s.to_uppercase()))
                                .unwrap_or(false)
                        }
                    };
                    let matches_name = if self.filter_name.is_empty() {
                        true
                    } else {
                        f.name
                            .to_lowercase()
                            .contains(&name_filter)
                    };
                    let matches_size = if self.filter_sizes.is_empty() {
                        true
                    } else {
                        self.filter_sizes.iter().any(|cat| cat.matches(f.size))
                    };
                    let matches_date = if self.filter_dates.is_empty() {
                        true
                    } else {
                        self.filter_dates
                            .contains(&DateGroup::for_timestamp(f.modified, today))
                    };
                    let matches_location = self.filter_original_location.is_empty()
                        || f.original_path
                            .as_deref()
                            .and_then(|path| std::path::Path::new(path).parent())
                            .is_some_and(|path| {
                                path.to_string_lossy()
                                    .to_lowercase()
                                    .contains(&location_filter)
                            });
                    matches_type && matches_name && matches_size && matches_date && matches_location
                })
                .cloned()
                .collect()
        } else {
            files.to_vec()
        }
    }
}

impl CalendarFilterRequest {
    pub fn calculate(self) -> CalendarFilterResult {
        let files = self
            .filters
            .filtered_files_on(&self.files, self.reference.today);
        CalendarFilterResult {
            tab_id: self.tab_id,
            revision: self.revision,
            reference: self.reference,
            files,
        }
    }
}

impl TabState {
    /// Match the header indicators: selecting every present value does not filter.
    pub(crate) fn metadata_filter_flags(&self) -> (bool, bool, bool) {
        let type_filtered = !self.filter_types.is_empty() && self.files.iter().any(|file| {
            !self.filter_types.contains(&crate::ui::components::file_list::file_type_label(file))
        });
        let categories = [SizeCategory::Minuscule, SizeCategory::Petit, SizeCategory::Moyen,
            SizeCategory::Grand, SizeCategory::Huge];
        let size_filtered = !self.filter_sizes.is_empty() && self.files.iter().any(|file| {
            categories.iter().find(|category| category.matches(file.size))
                .is_some_and(|category| !self.filter_sizes.contains(category))
        });
        let date_filtered = !self.filter_dates.is_empty() &&
            crate::ui::date_groups::counts_by_date(&self.files, chrono::Local::now().date_naive())
                .keys().any(|group| !self.filter_dates.contains(group));
        (type_filtered, size_filtered, date_filtered)
    }

    pub fn update_cache(&mut self) {
        self.update_cache_on(chrono::Local::now().date_naive());
    }

    fn update_cache_on(&mut self, today: chrono::NaiveDate) {
        let mut reference = CalendarReference::current();
        reference.today = today;
        self.cached_filtered_files =
            FileFilters::from_tab(self).filtered_files_on(&self.files, today);
        self.calendar_reference = Some(reference);
        self.filter_cache_revision = self.filter_cache_revision.wrapping_add(1);
        self.recalculate_column_widths();
    }

    fn calendar_filter_request(
        &self,
        reference: &CalendarReference,
    ) -> Option<CalendarFilterRequest> {
        if self.filter_dates.is_empty() || self.calendar_reference.as_ref() == Some(reference) {
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
        self.visit_panes(&mut |pane| grouped |= pane.tabs.iter().any(|tab| tab.group_by_date));
        grouped
    }

}
