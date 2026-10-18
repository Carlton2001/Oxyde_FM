//! Immutable data captured by virtualized row closures, shared across redraws.
use super::TabState;
use crate::ui::date_groups::{CalendarReference, GroupKey, DisplayRow};
use oxyde_lib::models::FileEntry;
use std::{collections::HashSet, sync::Arc};

pub(crate) struct Presentation {
    pub files: Arc<Vec<FileEntry>>,
    pub rows: Arc<Vec<DisplayRow>>,
    pub groups: Arc<std::collections::BTreeMap<GroupKey, Vec<usize>>>,
    pub heights: Vec<f32>,
    pub folders: usize,
    pub histogram_total: u64,
    grid: bool,
    columns: usize,
    grouped: crate::ui::date_groups::GroupMode,
    collapsed: HashSet<GroupKey>,
    calendar: Option<CalendarReference>,
    row_height: f32,
    grid_height: f32,
}

impl TabState {
    pub(crate) fn presentation(&self, grid: bool, columns: usize) -> Arc<Presentation> {
        let columns = columns.max(1);
        let calendar = self.grouping().is_calendar().then(CalendarReference::current);
        let row_height = crate::ui::style::row_height();
        let grid_height = crate::ui::style::grid_card_row_height();
        let mut cache = self.presentation_cache.borrow_mut();
        if let Some(previous) = cache.as_ref() {
            if Arc::ptr_eq(&previous.files, &self.cached_filtered_files)
                && previous.grid == grid
                && previous.columns == columns
                && previous.grouped == self.grouping()
                && previous.collapsed == self.collapsed_groups
                && previous.calendar == calendar
                && previous.row_height == row_height
                && previous.grid_height == grid_height
            {
                return previous.clone();
            }
        }
        let files = self.cached_filtered_files.clone();
        let (rows, groups) = crate::ui::date_groups::display_data(&files, self.grouping(), &self.collapsed_groups, columns);
        let rows = Arc::new(rows);
        let groups = Arc::new(groups);
        let mut heights = Vec::with_capacity(rows.len() + 1);
        heights.push(0.0);
        for row in rows.iter() {
            heights.push(heights.last().unwrap() + crate::ui::date_groups::row_height(row, grid));
        }
        let folders = files.iter().filter(|file| file.is_dir).count();
        let histogram_total = if files.iter().all(|file| !file.is_dir || file.is_calculated) {
            files
                .iter()
                .fold(0u64, |total, file| total.saturating_add(file.size))
        } else {
            0
        };
        let presentation = Arc::new(Presentation {
            files,
            rows,
            groups,
            heights,
            folders,
            histogram_total,
            grid,
            columns,
            grouped: self.grouping(),
            collapsed: self.collapsed_groups.clone(),
            calendar,
            row_height,
            grid_height,
        });
        *cache = Some(presentation.clone());
        presentation
    }
}

#[cfg(test)]
#[path = "presentation/tests/reuse.rs"]
mod tests;
