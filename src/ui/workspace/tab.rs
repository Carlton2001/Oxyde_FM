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
    /// The name of `mode` here: grouping by origin reads Location in search results.
    pub(crate) fn grouping_message(&self, mode: crate::ui::date_groups::GroupMode) -> crate::ui::i18n::Message {
        if mode == crate::ui::date_groups::GroupMode::Origin && self.path != "trash://" {
            crate::ui::i18n::Message::ColumnLocation
        } else {
            mode.message()
        }
    }
    pub(crate) fn available_groupings(&self) -> Vec<crate::ui::date_groups::GroupMode> {
        crate::ui::date_groups::GroupMode::ALL.into_iter().filter(|mode| self.supports_grouping(*mode)).collect()
    }

    /// A renamed item stays selected under its new name, as in Nautilus.
    /// Returns whether it was selected.
    pub(crate) fn follow_renamed_item(&mut self, old_path: &str, new_path: &str) -> bool {
        for path in [&mut self.selection_anchor, &mut self.keyboard_cursor].into_iter().flatten() {
            if path == old_path {
                *path = new_path.to_owned();
            }
        }
        let selected = self.selected_paths.remove(old_path);
        if selected {
            self.selected_paths.insert(new_path.to_owned());
        }
        selected
    }

    /// The archive's containing directory reports replacement/deletion events.
    /// This consumes the location resolved by the listing worker, without I/O.
    pub(crate) fn watched_path(&self) -> String {
        self.archive_root.as_ref().and_then(|root| root.parent())
            .map(|parent| parent.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.clone())
    }

    /// Entries per display row in a pane `width` wide: a line of cards, or one row.
    pub(crate) fn card_columns(&self, width: f32) -> usize {
        self.view_mode.cards().map_or(1, |layout| crate::ui::style::GridGeometry::for_viewport(layout, width, crate::ui::components::scrollbar::track_width()).columns)
    }

    pub(crate) fn reveal_pending_item(&mut self, width: f32) {
        let Some(path) = self.pending_reveal.take() else { return; };
        let Some(index) = self.filtered_files().iter().position(|file| file.path == path) else { return; };
        let group = crate::ui::date_groups::group_for_file(&self.filtered_files()[index], self.grouping(), chrono::Local::now().date_naive());
        self.collapsed_groups.remove(&group);
        let cards = self.view_mode.cards();
        let columns = self.card_columns(width);
        let rows = crate::ui::date_groups::display_rows(self.filtered_files(), self.grouping(), &self.collapsed_groups, columns);
        if let Some(row) = rows.iter().position(|row| matches!(row, crate::ui::date_groups::DisplayRow::Files(indices) if indices.contains(&index))) {
            if cards.is_some() {
                let y = crate::ui::date_groups::content_height(&rows[..row], cards);
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
            pending_rename_reveal: None,
            restored: false,
            opened_at: None,
            path: path.clone(),
            archive_root: None,
            denied_folder: None,
            denied_subfolders: Default::default(),
            undeletable_entries: Default::default(),
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
            filter_name_regex: false,
            filter_original_location: String::new(),
            filter_sizes: std::collections::HashSet::new(),
            filter_size_range: None,
            filter_permissions: std::collections::HashSet::new(),
            filter_dates: std::collections::HashSet::new(),
            filter_date_range: None,
            filter_deleted: std::collections::HashSet::new(),
            filter_deleted_range: None,
            keep_filters: false,
            selected_paths: Default::default(),
            selection_anchor: None,
            keyboard_cursor: None,
            view_mode: ViewMode::Grid,
            group_mode: crate::ui::date_groups::GroupMode::None,
            collapsed_groups: Default::default(),
            cached_filtered_files: Default::default(),
            presentation_cache: Default::default(),
            selection_cache: Default::default(),
            filter_flags_cache: Default::default(),
            filter_cache_revision: 0,
            files_sorted: None,
            calendar_reference: None,
            type_col_width: 100.0,
            size_col_width: 100.0,
            date_col_width: 180.0,
            deleted_col_width: 180.0,
            permissions_col_width: 64.0,
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

    /// Baseline width of the Permissions column, zero where it is not shown.
    pub fn permissions_column_width(&self, enabled: bool) -> f32 {
        if enabled && self.path != "__network_vincinity__" { self.permissions_col_width } else { 0.0 }
    }

    /// Baseline width of the columns kept outside the shared layout: Permissions and Deleted.
    pub fn fixed_columns_width(&self, permissions: bool) -> f32 {
        self.permissions_column_width(permissions) + if self.path == "trash://" { self.deleted_col_width } else { 0.0 }
    }

    pub fn filtered_files(&self) -> &[FileEntry] {
        &self.cached_filtered_files
    }

    /// The selection as the list draws it, kept while neither the selection nor the
    /// shown files change: a large selection is not copied and summed at each frame.
    pub(crate) fn selection_snapshot(&self) -> std::rc::Rc<SelectionSnapshot> {
        let mut cache = self.selection_cache.borrow_mut();
        if let Some(cached) = cache.as_ref() {
            if std::sync::Arc::ptr_eq(&cached.files, &self.cached_filtered_files) && cached.stamp == self.selected_paths.stamp() {
                return cached.clone();
            }
        }
        let shown: Vec<&FileEntry> = if self.selected_paths.is_empty() {
            Vec::new()
        } else {
            self.filtered_files().iter().filter(|file| self.selected_paths.contains(&file.path)).collect()
        };
        let folders = shown.iter().filter(|file| file.is_dir).count();
        let snapshot = std::rc::Rc::new(SelectionSnapshot {
            files: self.cached_filtered_files.clone(),
            paths: std::rc::Rc::new((*self.selected_paths).clone()),
            stamp: self.selected_paths.stamp(),
            folders,
            file_count: shown.len() - folders,
            bytes: shown.iter().filter(|file| !file.is_dir || file.is_calculated).map(|file| file.size).sum(),
            shown: std::rc::Rc::new(shown.iter().map(|file| file.path.clone()).collect()),
        });
        *cache = Some(snapshot.clone());
        snapshot
    }

    /// The selected entries an action applies to, in display order: what the filters
    /// show, members of collapsed groups included. A selected path hidden by a filter or
    /// gone from the listing is never copied, moved, compressed or deleted.
    pub(crate) fn actionable_selection(&self) -> Vec<String> {
        if self.selected_paths.is_empty() {
            return Vec::new();
        }
        self.filtered_files()
            .iter()
            .filter(|file| self.selected_paths.contains(&file.path))
            .map(|file| file.path.clone())
            .collect()
    }

    #[cfg(test)]
    pub fn sort_files(&mut self) {
        super::sorting::sort_files(std::sync::Arc::make_mut(&mut self.files).as_mut_slice(), self.sort_column, self.sort_direction);
    }
}

#[cfg(test)]
#[path = "tab/tests/grouping.rs"]
mod grouping_tests;

#[cfg(test)]
#[path = "tab/tests/renamed_selection.rs"]
mod renamed_selection_tests;

#[cfg(test)]
#[path = "tab/tests/actionable_selection.rs"]
mod actionable_selection_tests;

#[cfg(test)]
#[path = "tab/tests/selection_snapshot.rs"]
mod selection_snapshot_tests;

/// What [`TabState::selection_snapshot`] keeps.
pub(crate) struct SelectionSnapshot {
    /// The shown files it was taken from.
    files: std::sync::Arc<Vec<FileEntry>>,
    pub(crate) paths: std::rc::Rc<std::collections::HashSet<String>>,
    /// The [`SelectedPaths`](super::SelectedPaths) stamp `paths` was copied at.
    stamp: u64,
    /// The shown selected paths, in display order: what a drag carries.
    pub(crate) shown: std::rc::Rc<Vec<String>>,
    pub(crate) folders: usize,
    pub(crate) file_count: usize,
    pub(crate) bytes: u64,
}
