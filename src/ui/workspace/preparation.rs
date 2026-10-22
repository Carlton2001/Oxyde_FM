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
    FolderSize { path: String, size: oxyde_lib::models::FolderSizeResult },
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
    keep_folder_sizes: bool,
}
pub(crate) struct Prepared {
    request: Request,
    files: Arc<Vec<FileEntry>>,
    filtered: Arc<Vec<FileEntry>>,
    types: HashSet<String>,
    sizes: HashSet<(u64, bool, bool)>,
    changed_size: Option<String>,
}
impl Request {
    pub(crate) fn calculate(self) -> Prepared {
        let mut changed_size = None;
        let mut files = match &self.update {
            FileUpdate::Replace(files) | FileUpdate::Listing(files) => files.clone(),
            _ => self.source.clone(),
        };
        if let FileUpdate::Append(additions) = &self.update {
            Arc::make_mut(&mut files).extend(additions.iter().cloned());
        }
        if self.keep_folder_sizes && matches!(self.update, FileUpdate::Listing(_) | FileUpdate::Replace(_)) {
            let previous: std::collections::HashMap<_, _> = self.source.iter()
                .filter(|file| super::folder_sizes::measurable(file) && file.is_calculated)
                .map(|file| (file.path.as_str(), file)).collect();
            for file in Arc::make_mut(&mut files).iter_mut().filter(|file| super::folder_sizes::measurable(file)) {
                if let Some(old) = previous.get(file.path.as_str()) {
                    file.size = old.size;
                    file.files_count = old.files_count;
                    file.folders_count = old.folders_count;
                    file.is_calculated = true;
                }
            }
        }
        if let FileUpdate::FolderSize { path, size } = &self.update {
            if let Some(file) = Arc::make_mut(&mut files).iter_mut()
                .find(|file| file.path == *path && super::folder_sizes::measurable(file)) {
                // Initial results and unchanged totals stay quiet. Only a
                // published change to an existing total receives a flash.
                if file.is_calculated && file.size != size.size {
                    changed_size = Some(path.clone());
                }
                file.size = size.size;
                file.files_count = Some(size.files_count);
                file.folders_count = Some(size.folders_count);
                file.is_calculated = true;
            }
        }
        // A batch cache avoids repeated Shell calls, including for unknown extensions.
        // Recreated per preparation so association changes are picked up on refresh.
        let mut native_kinds = std::collections::HashMap::new();
        for file in Arc::make_mut(&mut files).iter_mut()
            .filter(|file| !file.is_dir && !matches!(self.update, FileUpdate::FolderSize { .. })) {
            let extension = std::path::Path::new(&file.name).extension()
                .and_then(|value| value.to_str()).filter(|value| !value.is_empty());
            file.native_kind = extension.and_then(|extension| {
                let extension = format!(".{}", extension.to_ascii_lowercase());
                *native_kinds.entry(extension).or_insert_with_key(|extension| {
                    oxyde_lib::platform::platform().file_kind_for_extension(extension)
                })
            });
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
            changed_size,
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
            keep_folder_sizes: self.show_size_histogram,
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
        if let Some(path) = result.changed_size {
            self.folder_sizes.flash_updated_size(path);
        }
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

#[cfg(all(test, target_os = "windows"))]
#[path = "preparation/tests/native_kinds.rs"]
mod native_kind_tests;
