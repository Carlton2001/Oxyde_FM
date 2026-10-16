//! Send-only snapshots: sorting, filtering and column summaries stay off the UI.
use super::{CalendarReference, FileFilters, SortColumn, SortDirection, TabState};
use oxyde_lib::models::FileEntry;
use std::{collections::HashSet, sync::Arc};

#[derive(Clone)]
pub(crate) enum FileUpdate {
    Current,
    Listing(Arc<Vec<FileEntry>>),
    Replace(Arc<Vec<FileEntry>>),
    Append(Arc<Vec<FileEntry>>),
}
pub(crate) struct Request {
    id: String,
    path: String,
    revision: u64,
    source: Arc<Vec<FileEntry>>,
    update: FileUpdate,
    column: SortColumn,
    direction: SortDirection,
    filters: FileFilters,
    calendar: CalendarReference,
}
pub(crate) struct Prepared {
    request: Request,
    files: Arc<Vec<FileEntry>>,
    filtered: Arc<Vec<FileEntry>>,
    types: HashSet<String>,
    sizes: HashSet<(u64, bool, bool)>,
}
impl Request {
    pub(crate) fn calculate(self) -> Prepared {
        let mut files = match &self.update {
            FileUpdate::Replace(files) | FileUpdate::Listing(files) => files.clone(),
            _ => self.source.clone(),
        };
        if let FileUpdate::Append(additions) = &self.update {
            Arc::make_mut(&mut files).extend(additions.iter().cloned());
        }
        let sort = if matches!(self.update, FileUpdate::Listing(_)) {
            super::sorting::sort_listed_files
        } else {
            super::sorting::sort_files
        };
        sort(
            Arc::make_mut(&mut files).as_mut_slice(),
            self.column,
            self.direction,
        );
        let filtered = self.filters.snapshot(&files, self.calendar.today);
        let mut types = HashSet::new();
        let mut sizes = HashSet::new();
        for file in files.iter() {
            types.insert(crate::ui::components::file_list::file_type_label(file));
            sizes.insert((file.size, file.is_dir, file.is_calculated));
        }
        Prepared {
            request: self,
            files,
            filtered,
            types,
            sizes,
        }
    }
}
impl TabState {
    pub(crate) fn invalidate_preparation(&mut self) {
        self.filter_cache_revision = self.filter_cache_revision.wrapping_add(1);
    }
    pub(crate) fn clear_files(&mut self) {
        self.files = Default::default();
        self.cached_filtered_files = Default::default();
        self.presentation_cache.get_mut().take();
        self.invalidate_preparation();
    }
    pub(crate) fn preparation(&self, update: FileUpdate) -> Request {
        Request {
            id: self.id.clone(),
            path: self.path.clone(),
            revision: self.filter_cache_revision,
            source: self.files.clone(),
            update,
            column: self.sort_column,
            direction: self.sort_direction,
            filters: FileFilters::from_tab(self),
            calendar: CalendarReference::current(),
        }
    }
    pub(crate) fn apply_preparation(&mut self, result: Prepared) -> bool {
        if self.id != result.request.id
            || self.path != result.request.path
            || self.filter_cache_revision != result.request.revision
            || !Arc::ptr_eq(&self.files, &result.request.source)
        {
            return false;
        }
        self.files = result.files;
        self.cached_filtered_files = result.filtered;
        self.presentation_cache.get_mut().take();
        self.column_type_labels = result.types;
        self.column_size_values = result.sizes;
        self.column_measurement_key = None;
        self.calendar_reference = Some(result.request.calendar);
        self.invalidate_preparation();
        true
    }
}

#[cfg(test)]
#[path = "preparation/tests/revisions.rs"]
mod tests;
