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
    FolderSizes(Vec<(String, oxyde_lib::models::FolderSizeResult)>),
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
    /// `source` is already in this request's order, its kinds resolved.
    sorted: bool,
}
pub(crate) struct Prepared {
    request: Request,
    files: Arc<Vec<FileEntry>>,
    filtered: Arc<Vec<FileEntry>>,
    /// The column values, `None` when the files are those of the tab.
    columns: Option<(HashSet<String>, HashSet<(u64, bool, bool)>)>,
    /// `columns` holds only the appended files' values, added to the tab's.
    columns_appended: bool,
    changed_sizes: Vec<String>,
}
impl Request {
    pub(crate) fn calculate(self) -> Prepared {
        // Batches added to files already in order: only the batch is resolved, sorted
        // and summarized, then merged in.
        if let (FileUpdate::Append(additions), true) = (&self.update, self.sorted) {
            let mut additions = additions.to_vec();
            resolve_native_kinds(&mut additions);
            let columns = column_values(&additions);
            let files = Arc::new(super::sorting::merge_sorted(&self.source, additions, self.column, self.direction));
            let filtered = self.filters.snapshot(&files, self.calendar.today);
            return Prepared { request: self, files, filtered, columns: Some(columns), columns_appended: true, changed_sizes: Vec::new() };
        }
        let mut changed_sizes = Vec::new();
        let mut files = match &self.update {
            FileUpdate::Replace(files) | FileUpdate::Listing(files) => files.clone(),
            _ => self.source.clone(),
        };
        if let FileUpdate::Append(additions) = &self.update {
            Arc::make_mut(&mut files).extend(additions.iter().cloned());
        }
        if self.keep_folder_sizes
            && matches!(self.update, FileUpdate::Listing(_) | FileUpdate::Replace(_))
        {
            let previous: std::collections::HashMap<_, _> = self
                .source
                .iter()
                .filter(|file| super::folder_sizes::measurable(file) && file.is_calculated)
                .map(|file| (file.path.as_str(), file))
                .collect();
            for file in Arc::make_mut(&mut files)
                .iter_mut()
                .filter(|file| super::folder_sizes::measurable(file))
            {
                if let Some(old) = previous.get(file.path.as_str()) {
                    file.size = old.size;
                    file.files_count = old.files_count;
                    file.folders_count = old.folders_count;
                    file.is_calculated = true;
                }
            }
        }
        if let FileUpdate::FolderSizes(updates) = &self.update {
            let updates: std::collections::HashMap<_, _> = updates
                .iter()
                .map(|(path, size)| (path.as_str(), size))
                .collect();
            for file in Arc::make_mut(&mut files)
                .iter_mut()
                .filter(|file| super::folder_sizes::measurable(file))
            {
                if let Some(size) = updates.get(file.path.as_str()) {
                    if file.is_calculated && file.size != size.size {
                        changed_sizes.push(file.path.clone());
                    }
                    file.size = size.size;
                    file.files_count = Some(size.files_count);
                    file.folders_count = Some(size.folders_count);
                    file.is_calculated = true;
                }
            }
        }
        // A filter or a group changed: the files, already in order, are neither
        // copied nor sorted again.
        let unchanged = matches!(self.update, FileUpdate::Current) && self.sorted;
        if !unchanged {
            if !matches!(self.update, FileUpdate::FolderSizes(_)) {
                resolve_native_kinds(Arc::make_mut(&mut files).as_mut_slice());
            }
            let sort = if matches!(self.update, FileUpdate::Listing(_)) {
                super::sorting::sort_listed_files
            } else {
                super::sorting::sort_files
            };
            // New folder sizes move nothing unless the files are ordered by size.
            let in_order = self.sorted && matches!(self.update, FileUpdate::FolderSizes(_)) && self.column != SortColumn::Size;
            if !in_order {
                sort(
                    Arc::make_mut(&mut files).as_mut_slice(),
                    self.column,
                    self.direction,
                );
            }
        }
        let filtered = self.filters.snapshot(&files, self.calendar.today);
        let columns = (!unchanged).then(|| column_values(&files));
        Prepared {
            request: self,
            files,
            filtered,
            columns,
            columns_appended: false,
            changed_sizes,
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
            sorted: self.files_sorted.as_ref().is_some_and(|(column, direction, files)| {
                *column == self.sort_column
                    && *direction == self.sort_direction
                    && std::ptr::eq(files.as_ptr(), Arc::as_ptr(&self.files))
            }),
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
        self.files_sorted = Some((result.request.column, result.request.direction, Arc::downgrade(&self.files)));
        for path in result.changed_sizes {
            self.folder_sizes.flash_updated_size(path);
        }
        self.cached_filtered_files = result.filtered;
        self.presentation_cache.get_mut().take();
        if let Some((types, sizes)) = result.columns {
            if result.columns_appended {
                self.column_type_labels.extend(types);
                self.column_size_values.extend(sizes);
            } else {
                self.column_type_labels = types;
                self.column_size_values = sizes;
            }
            self.column_measurement_key = None;
        }
        self.calendar_reference = Some(result.request.calendar);
        self.invalidate_preparation();
        true
    }
}

#[cfg(test)]
#[path = "preparation/tests/revisions.rs"]
mod tests;

#[cfg(test)]
#[path = "preparation/tests/unchanged.rs"]
mod unchanged_tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "preparation/tests/native_kinds.rs"]
mod native_kind_tests;

fn column_values(files: &[FileEntry]) -> (HashSet<String>, HashSet<(u64, bool, bool)>) {
    let mut types = HashSet::new();
    let mut sizes = HashSet::new();
    for file in files {
        types.insert(crate::ui::components::file_list::file_type_label(file));
        sizes.insert((file.size, file.is_dir, file.is_calculated));
    }
    (types, sizes)
}

/// Native categories of files by extension, which icons and their colors follow.
/// A batch cache avoids repeated Shell calls, including for unknown extensions; it is
/// recreated per call so association changes are picked up on refresh. Blocking.
pub(crate) fn resolve_native_kinds(files: &mut [FileEntry]) {
    let mut native_kinds = std::collections::HashMap::new();
    for file in files.iter_mut().filter(|file| !file.is_dir) {
        let extension = std::path::Path::new(&file.name)
            .extension()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty());
        file.native_kind = extension.and_then(|extension| {
            let extension = format!(".{}", extension.to_ascii_lowercase());
            *native_kinds
                .entry(extension)
                .or_insert_with_key(|extension| oxyde_lib::platform::platform().file_kind_for_extension(extension))
        });
    }
}
