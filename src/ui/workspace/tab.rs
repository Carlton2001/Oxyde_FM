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
    pub(crate) fn quick_search_query(&self) -> Option<&str> {
        self.quick_search.as_ref().map(|search| search.query.as_str()).or_else(|| {
            let query = self.search_text.trim();
            (self.path == "trash://" && !query.is_empty()).then_some(query)
        })
    }

    pub(crate) fn grouping(&self) -> crate::ui::date_groups::GroupMode {
        if self.supports_grouping(self.group_mode) { self.group_mode }
        else { crate::ui::date_groups::GroupMode::None }
    }
    pub(crate) fn is_grouped(&self) -> bool { self.grouping() != crate::ui::date_groups::GroupMode::None }
    pub(crate) fn supports_grouping(&self, mode: crate::ui::date_groups::GroupMode) -> bool {
        use crate::ui::date_groups::GroupMode;
        if self.path == "__network_vincinity__" { return mode == GroupMode::None; }
        match mode {
            GroupMode::Deleted => self.path == "trash://",
            GroupMode::Origin => self.path == "trash://" || self.quick_search.is_some(),
            GroupMode::Created => self.path != "trash://" && self.archive_root.is_none(),
            _ => true,
        }
    }
    pub(crate) fn available_groupings(&self) -> Vec<crate::ui::date_groups::GroupMode> {
        crate::ui::date_groups::GroupMode::ALL.into_iter().filter(|mode| self.supports_grouping(*mode)).collect()
    }

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
        let group = crate::ui::date_groups::group_for_file(&self.filtered_files()[index], self.grouping(), chrono::Local::now().date_naive());
        self.collapsed_groups.remove(&group);
        let grid = self.view_mode == ViewMode::Grid;
        let columns = if grid { crate::ui::style::GridGeometry::for_viewport(width, crate::ui::components::scrollbar::zed_track_width()).columns } else { 1 };
        let rows = crate::ui::date_groups::display_rows(self.filtered_files(), self.grouping(), &self.collapsed_groups, columns);
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
            restored: false,
            path: path.clone(),
            archive_root: None,
            files: Default::default(),
            show_size_histogram: false,
            size_calculation: None,
            folder_sizes: Default::default(),
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            list_scroll: UniformListScrollHandle::new(),
            list_scroll_h: ScrollHandle::new(),
            breadcrumb_scroll: ScrollHandle::new(),
            breadcrumb_scroll_path: std::cell::RefCell::new(String::new()),
            breadcrumb_viewport_width: std::rc::Rc::new(std::cell::Cell::new(0.0)),
            breadcrumb_glide: Default::default(),
            scroll_top_anchor: Default::default(),
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
            group_mode: crate::ui::date_groups::GroupMode::None,
            collapsed_groups: Default::default(),
            cached_filtered_files: Default::default(),
            presentation_cache: Default::default(),
            filter_cache_revision: 0,
            calendar_reference: None,
            type_col_width: 100.0,
            size_col_width: 100.0,
            date_col_width: 180.0,
            deleted_col_width: 180.0,
            column_type_labels: Default::default(),
            column_size_values: Default::default(),
            column_measurement_key: None,
            load_status: if path == super::viewer::VIEWER_PATH { TabLoadStatus::Ready } else { TabLoadStatus::Loading },
        }
    }

    pub fn recalculate_column_widths(&mut self) {
        self.column_type_labels.clear();
        self.column_size_values.clear();
        self.column_measurement_key = None;

        // We use the full files list instead of cached_filtered_files
        // to keep columns stable while the user interacts with filters.
        for f in self.files.iter() {
            self.column_type_labels.insert(crate::ui::components::file_list::file_type_label(f));

            self.column_size_values.insert((f.size, f.is_dir, f.is_calculated));
        }
    }

    pub fn filtered_files(&self) -> &[FileEntry] {
        &self.cached_filtered_files
    }

    #[cfg(test)]
    pub fn sort_files(&mut self) {
        super::sorting::sort_files(std::sync::Arc::make_mut(&mut self.files).as_mut_slice(), self.sort_column, self.sort_direction);
    }
}

#[cfg(test)]
#[path = "tab/tests/grouping.rs"]
mod grouping_tests;
