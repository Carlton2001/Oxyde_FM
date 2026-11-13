//! The items of the selected operation, as the main window's details list: loaded
//! with their metadata in the background, filtered by path and sorted by column off
//! the UI thread, a double click showing an item in its folder.
mod icons;
pub(super) mod outcome;
mod table;

use super::OperationsPanelView;
use crate::ui::components::icons::IconType;
use crate::ui::components::input::{InputAppearance, TextInput, TextInputOptions};
use crate::ui::workspace::SortDirection;
use gpui::*;
use oxyde_lib::models::FileEntry;
use oxyde_lib::systems::file_ops::{FileOpType, FileOperation, ItemOutcome};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(test)]
#[path = "items/tests/arrange.rs"]
mod arrange_tests;

pub(super) struct Item {
    path: PathBuf,
    name: String,
    parent: String,
    search: String,
    /// The item on disk when it was listed; None once gone.
    entry: Option<FileEntry>,
    icon: (String, IconType),
    extension: String,
}

impl Item {
    /// `shared_folder`: every item comes from one folder, which the filter then
    /// leaves out, as no row shows it.
    fn new(path: &Path, entry: Option<FileEntry>, shared_folder: bool) -> Self {
        let full = path.to_string_lossy();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| full.to_string());
        Self {
            icon: icons::icon(entry.as_ref(), path),
            extension: Path::new(&name)
                .extension()
                .map(|extension| extension.to_string_lossy().to_lowercase())
                .unwrap_or_default(),
            path: path.to_path_buf(),
            parent: path
                .parent()
                .map(|parent| parent.to_string_lossy().into_owned())
                .unwrap_or_default(),
            search: if shared_folder { name.to_lowercase() } else { full.to_lowercase() },
            name,
            entry,
        }
    }

    fn is_dir(&self) -> bool {
        self.entry.as_ref().is_some_and(|entry| entry.is_dir)
    }

    fn size(&self) -> u64 {
        self.entry.as_ref().map_or(0, |entry| entry.size)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Column {
    Name,
    Location,
    Type,
    Size,
    State,
}

/// The rows to show, in order: those whose path (or name, when they share a folder)
/// holds `query`, sorted by `column`.
pub(super) fn arrange(
    rows: &[Item],
    query: &str,
    column: Column,
    direction: SortDirection,
    outcomes: &HashMap<PathBuf, ItemOutcome>,
) -> Vec<usize> {
    let names = |a: &Item, b: &Item| oxyde_lib::systems::file_ops::compare_names(&a.path, &b.path);
    let mut shown: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, item)| query.is_empty() || item.search.contains(query))
        .map(|(index, _)| index)
        .collect();
    shown.sort_by(|&left, &right| {
        let (a, b) = (&rows[left], &rows[right]);
        let order = match column {
            Column::Name => Ordering::Equal,
            Column::Location => oxyde_lib::utils::compare_natural(&a.parent.to_lowercase(), &b.parent.to_lowercase()),
            // Folders before files, as the type column of the file list.
            Column::Type => b.is_dir().cmp(&a.is_dir()).then_with(|| a.extension.cmp(&b.extension)),
            Column::Size => a.size().cmp(&b.size()),
            Column::State => outcome::rank(outcomes.get(&a.path).copied())
                .cmp(&outcome::rank(outcomes.get(&b.path).copied())),
        }
        .then_with(|| names(a, b));
        match direction {
            SortDirection::Ascending => order,
            SortDirection::Descending => order.reverse(),
        }
    });
    shown
}

/// Metadata column widths, in the file list's units.
#[derive(Clone, Copy, Default)]
struct Widths {
    file_type: f32,
    size: f32,
    state: f32,
}

pub(super) struct Items {
    /// The operation these rows belong to.
    pub(super) operation: Option<String>,
    kind: Option<FileOpType>,
    destination: Option<PathBuf>,
    /// The folder every item comes from; without one, a Location column names each.
    origin: Option<PathBuf>,
    rows: Arc<Vec<Item>>,
    shown: Arc<Vec<usize>>,
    query: String,
    sort: Column,
    direction: SortDirection,
    pub(super) list: UniformListScrollHandle,
    loading: bool,
    load_task: Option<Task<()>>,
    arrange_task: Option<Task<()>>,
    /// The selected row, as an index into `rows`.
    selected: Option<usize>,
    pub(super) input: Option<Entity<TextInput>>,
    /// Bumped when rows load, so the columns are measured again.
    revision: u64,
    measured: Option<(u32, crate::ui::i18n::Language, u64)>,
    widths: Widths,
    /// The list's width at the last layout, which the columns share.
    pane_width: std::rc::Rc<std::cell::Cell<f32>>,
}

impl Default for Items {
    fn default() -> Self {
        Self {
            operation: None,
            kind: None,
            destination: None,
            origin: None,
            rows: Arc::default(),
            shown: Arc::default(),
            query: String::new(),
            sort: Column::Name,
            direction: SortDirection::Ascending,
            list: UniformListScrollHandle::new(),
            loading: false,
            load_task: None,
            arrange_task: None,
            selected: None,
            input: None,
            revision: 0,
            measured: None,
            widths: Widths::default(),
            pane_width: Default::default(),
        }
    }
}

impl OperationsPanelView {
    /// A filter field for this window; the view outlives the windows it shows in.
    pub(super) fn attach_items_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.entity().downgrade();
        let input = cx.new(|cx| {
            TextInput::new(
                cx,
                "",
                String::new(),
                move |value, _, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.items.query = value.to_lowercase();
                        this.arrange_items(cx);
                    });
                },
                |_, _| {},
                |window, cx| {
                    crate::ui::components::tool_window::close_transients(window, cx);
                    window.remove_window();
                },
                |_, _, _, _, _| {},
                TextInputOptions {
                    appearance: InputAppearance::Embedded,
                    clearable: true,
                    ..Default::default()
                },
                window,
            )
        });
        self.items.query.clear();
        self.items.input = Some(input);
    }

    /// Lists `operation`'s items again when another operation is selected.
    pub(super) fn follow_items(&mut self, operation: &FileOperation, cx: &mut Context<Self>) {
        if self.items.operation.as_deref() == Some(operation.id.as_str()) {
            return;
        }
        let items = &mut self.items;
        items.operation = Some(operation.id.clone());
        items.kind = Some(operation.op_type.clone());
        items.destination = operation.destination.clone();
        items.origin = operation.source_parent.clone();
        items.rows = Arc::default();
        items.shown = Arc::default();
        items.selected = None;
        items.loading = true;
        items.arrange_task = None;
        items.list = UniformListScrollHandle::new();
        let sources = operation.sources.clone();
        // Moved and copied items are found at their destination once their source is gone.
        let destination = matches!(operation.op_type, FileOpType::Copy | FileOpType::Move)
            .then(|| operation.destination.clone())
            .flatten();
        let outcomes = operation.item_outcomes.clone();
        let shared_folder = operation.source_parent.is_some();
        let (query, sort, direction) = (items.query.clone(), items.sort, items.direction);
        items.load_task = Some(cx.spawn(async move |this, cx| {
            let (rows, shown) = cx
                .background_executor()
                .spawn(async move {
                    let rows: Vec<Item> = sources
                        .iter()
                        .map(|source| {
                            let moved = destination.as_ref().zip(source.file_name()).map(|(folder, name)| folder.join(name));
                            Item::new(source, icons::resolve_entry(source, moved.as_deref()).ok(), shared_folder)
                        })
                        .collect();
                    let shown = arrange(&rows, &query, sort, direction, &outcomes);
                    (Arc::new(rows), Arc::new(shown))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.items.rows = rows;
                this.items.shown = shown;
                this.items.loading = false;
                this.items.revision += 1;
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Filters and sorts the loaded rows again, off the UI thread.
    fn arrange_items(&mut self, cx: &mut Context<Self>) {
        let rows = self.items.rows.clone();
        let (query, sort, direction) = (self.items.query.clone(), self.items.sort, self.items.direction);
        let outcomes = self.selected_outcomes(cx);
        self.items.arrange_task = Some(cx.spawn(async move |this, cx| {
            let shown = cx
                .background_executor()
                .spawn(async move { Arc::new(arrange(&rows, &query, sort, direction, &outcomes)) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.items.shown = shown;
                this.items.list.scroll_to_item(0, ScrollStrategy::Top);
                cx.notify();
            });
        }));
    }

    fn selected_outcomes(&self, cx: &App) -> HashMap<PathBuf, ItemOutcome> {
        let id = self.items.operation.clone();
        self.app
            .upgrade()
            .zip(id)
            .and_then(|(app, id)| app.read(cx).file_ops.operation_item_outcomes(&id))
            .unwrap_or_default()
    }

    /// A header press: sorts by its column, or reverses the order when it already does.
    fn sort_items(&mut self, column: Column, cx: &mut Context<Self>) {
        if self.items.sort == column {
            self.items.direction = match self.items.direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            };
        } else {
            self.items.sort = column;
            self.items.direction = SortDirection::Ascending;
        }
        self.arrange_items(cx);
        cx.notify();
    }

    /// A double click: the item where it is now, selected in its folder in the main
    /// window. A moved or copied item is looked for at its destination first, a
    /// compressed one in its archive; a deleted one leaves its former folder to open.
    fn reveal_item(&mut self, row: usize, cx: &mut Context<Self>) {
        let Some(item) = self.items.rows.get(row) else { return };
        let source = item.path.clone();
        let mut candidates = Vec::new();
        match (&self.items.kind, &self.items.destination) {
            (Some(FileOpType::Copy | FileOpType::Move), Some(folder)) => {
                if let Some(name) = source.file_name() {
                    candidates.push(folder.join(name));
                }
            }
            (Some(FileOpType::Compress), Some(archive)) => candidates.push(archive.clone()),
            _ => {}
        }
        candidates.push(source.clone());
        let extracted = matches!(self.items.kind, Some(FileOpType::Extract))
            .then(|| self.items.destination.clone())
            .flatten();
        cx.spawn(async move |this, cx| {
            let target = cx
                .background_executor()
                .spawn(async move {
                    if let Some(folder) = extracted.filter(|folder| folder.is_dir()) {
                        return Some((folder, None));
                    }
                    candidates
                        .into_iter()
                        .find(|path| path.symlink_metadata().is_ok())
                        .and_then(|path| Some((path.parent()?.to_path_buf(), Some(path))))
                        .or_else(|| source.parent().filter(|folder| folder.is_dir()).map(|folder| (folder.to_path_buf(), None)))
                })
                .await;
            if let Some((folder, item)) = target {
                let _ = this.update(cx, |this, cx| this.show_in_main_window(folder, item, cx));
            }
        })
        .detach();
    }
}
