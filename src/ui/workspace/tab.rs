use super::{SortColumn, SortDirection, TabLoadStatus, TabState, ViewMode};
use gpui::{ScrollHandle, UniformListScrollHandle};
use oxyde_lib::models::FileEntry;
mod scroll;

#[cfg(test)]
mod prepared_sort_tests;

#[cfg(test)]
#[path = "tab/tests/archive_watchers.rs"]
mod archive_watcher_tests;

impl TabState {
    /// The archive's containing directory reports replacement/deletion events.
    /// This consumes the location resolved by the listing worker, without I/O.
    pub(crate) fn watched_path(&self) -> String {
        self.archive_root.as_ref().and_then(|root| root.parent())
            .map(|parent| parent.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.clone())
    }

    pub(crate) fn reveal_pending_item(&mut self, width: f32) {
        let Some(path) = self.pending_reveal.take() else { return; };
        let Some(index) = self.filtered_files().iter().position(|file| file.path == path) else { return; };
        let modified = self.filtered_files()[index].modified;
        self.collapsed_date_groups.remove(&crate::ui::date_groups::DateGroup::for_timestamp(modified, chrono::Local::now().date_naive()));
        let grid = self.view_mode == ViewMode::Grid;
        let columns = if grid { crate::ui::style::GridGeometry::for_viewport(width, crate::ui::components::scrollbar::zed_track_width()).columns } else { 1 };
        let rows = crate::ui::date_groups::display_rows(self.filtered_files(), self.group_by_date, &self.collapsed_date_groups, columns);
        if let Some(row) = rows.iter().position(|row| matches!(row, crate::ui::date_groups::DisplayRow::Files(indices) if indices.contains(&index))) {
            if grid {
                let y = crate::ui::date_groups::content_height(&rows[..row], true);
                self.list_scroll.0.borrow().base_handle.set_offset(gpui::point(gpui::px(0.0), gpui::px(-y)));
            } else { self.list_scroll.scroll_to_item(row, gpui::ScrollStrategy::Top); }
        }
    }


    pub fn new(id: String, path: String) -> Self {
        Self {
            id,
            quick_search: None,
            search_input: None,
            search_slide: Default::default(),
            search_text: String::new(),
            search_revision: 0,
            pending_reveal: None,
            path: path.clone(),
            archive_root: None,
            files: Vec::new(),
            show_size_histogram: false,
            size_calculation: None,
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            list_scroll: UniformListScrollHandle::new(),
            list_scroll_h: ScrollHandle::new(),
            breadcrumb_scroll: ScrollHandle::new(),
            breadcrumb_scroll_path: std::cell::RefCell::new(String::new()),
            breadcrumb_viewport_width: std::rc::Rc::new(std::cell::Cell::new(0.0)),
            sort_column: SortColumn::Name,
            sort_direction: SortDirection::Ascending,
            filter_types: std::collections::HashSet::new(),
            filter_name: String::new(),
            filter_original_location: String::new(),
            filter_sizes: std::collections::HashSet::new(),
            filter_dates: std::collections::HashSet::new(),
            selected_paths: std::collections::HashSet::new(),
            selection_anchor: None,
            keyboard_cursor: None,
            view_mode: ViewMode::Grid,
            group_by_date: false,
            collapsed_date_groups: Default::default(),
            cached_filtered_files: Vec::new(),
            filter_cache_revision: 0,
            calendar_reference: None,
            type_col_width: 100.0,
            size_col_width: 100.0,
            date_col_width: 180.0,
            deleted_col_width: 180.0,
            column_type_labels: Default::default(),
            column_size_values: Default::default(),
            column_measurement_key: None,
            load_status: TabLoadStatus::Loading,
        }
    }

    pub fn recalculate_column_widths(&mut self) {
        self.column_type_labels.clear();
        self.column_size_values.clear();
        self.column_measurement_key = None;

        // We use the full files list instead of cached_filtered_files
        // to keep columns stable while the user interacts with filters.
        for f in &self.files {
            self.column_type_labels.insert(crate::ui::components::file_list::file_type_label(f));

            self.column_size_values.insert((f.size, f.is_dir, f.is_calculated));
        }
    }

    pub fn filtered_files(&self) -> &[FileEntry] {
        &self.cached_filtered_files
    }

    pub fn sort_files(&mut self) {
        // Build each normalized key once; keep folders first in both directions.
        #[derive(PartialEq, Eq, PartialOrd, Ord)]
        enum Value {
            Text(String),
            Number(u64),
            Deleted(Option<i64>),
            Location(Option<std::path::PathBuf>),
        }
        let col = self.sort_column;
        let key = |file: &FileEntry| {
            let name = file.name.to_lowercase();
            let value = match col {
                SortColumn::Name => Value::Text(String::new()),
                SortColumn::Type => Value::Text(
                    std::path::Path::new(&file.name)
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .unwrap_or("")
                        .to_lowercase(),
                ),
                SortColumn::Size => Value::Number(file.size),
                SortColumn::Modified => Value::Number(file.modified),
                SortColumn::DeletedAt => Value::Deleted(file.deleted_time),
                SortColumn::OriginalLocation => Value::Text(
                    file.original_path
                        .as_deref()
                        .and_then(|path| std::path::Path::new(path).parent())
                        .map(|path| path.to_string_lossy().to_lowercase())
                        .unwrap_or_default(),
                ),
                SortColumn::Location => Value::Location(
                    std::path::Path::new(&file.path)
                        .parent()
                        .map(std::path::Path::to_path_buf),
                ),
            };
            (value, name)
        };
        match self.sort_direction {
            SortDirection::Ascending => self
                .files
                .sort_by_cached_key(|file| (!file.is_dir, key(file))),
            SortDirection::Descending => self
                .files
                .sort_by_cached_key(|file| (!file.is_dir, std::cmp::Reverse(key(file)))),
        }
    }
}
