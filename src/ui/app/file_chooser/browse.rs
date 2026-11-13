//! Browsing in the file chooser: the history, listings read and arranged in the
//! background, the selection, and what opening an entry does, entering a folder or
//! choosing a file, and what the button that accepts chooses, or where it saves.
use super::selection::{Kept, Selection};
use super::{FileChooserWindow, Listing};
use crate::ui::workspace::{SortColumn, SortDirection};
use gpui::*;
use oxyde_lib::models::FileEntry;
use oxyde_lib::platform::FileChooserPurpose;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// How a navigation moves through the history.
#[derive(Clone, Copy)]
enum Step {
    New,
    Back,
    Forward,
}

impl FileChooserWindow {
    /// Opens `folder` as a new step of the history.
    pub(super) fn show_folder(&mut self, folder: String, cx: &mut Context<Self>) {
        self.go(folder, Step::New, cx);
    }

    pub(super) fn go_back(&mut self, cx: &mut Context<Self>) {
        if let Some(folder) = self.back.pop() {
            self.go(folder, Step::Back, cx);
        }
    }

    pub(super) fn go_forward(&mut self, cx: &mut Context<Self>) {
        if let Some(folder) = self.forward.pop() {
            self.go(folder, Step::Forward, cx);
        }
    }

    /// The parent folder, as the main window's Up button finds it.
    pub(super) fn parent(&self) -> Option<String> {
        Path::new(&self.folder).parent().filter(|parent| !parent.as_os_str().is_empty())
            .map(|parent| parent.to_string_lossy().into_owned())
    }

    pub(super) fn go_up(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = self.parent() {
            self.show_folder(parent, cx);
        }
    }

    fn go(&mut self, folder: String, step: Step, cx: &mut Context<Self>) {
        let left = std::mem::replace(&mut self.folder, folder);
        if !left.is_empty() {
            self.note_interaction(cx);
        }
        if !left.is_empty() && left != self.folder {
            match step {
                Step::New => {
                    self.back.push(left.clone());
                    self.forward.clear();
                }
                Step::Back => self.forward.push(left.clone()),
                Step::Forward => self.back.push(left.clone()),
            }
        }
        // A new folder starts unfiltered, its path no longer edited.
        self.location.close();
        self.rename = None;
        self.crumbs.menu = None;
        self.crumbs.context = None;
        self.filter.clear();
        self.entries = Arc::new(Vec::new());
        self.shown = Arc::new(Vec::new());
        self.folders = 0;
        self.subfolders.clear();
        self.selection.clear();
        self.list.0.borrow().base_handle.set_offset(point(px(0.0), px(0.0)));
        self.grid_scroll.set_offset(point(px(0.0), px(0.0)));
        // Back in a parent, the folder just left stays selected.
        self.list_folder(Kept::path(left), cx);
        self.follow_tree(cx);
    }

    /// Lists the folder again, the selection kept.
    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
        let keep = self.kept();
        self.list_folder(keep, cx);
    }

    /// Lists the folder off the UI thread; a later navigation drops this listing. When
    /// a type filter names MIME types, the files' types are read there too, once for
    /// all filters. Its folders are then checked for subfolders, as the main window
    /// checks them.
    pub(super) fn list_folder(&mut self, keep: Kept, cx: &mut Context<Self>) {
        self.listing = Listing::Loading;
        self.arrange_task = None;
        let folder = self.folder.clone();
        let (show_hidden, show_system) = (self.show_hidden, self.show_system);
        let typed = !self.directory && self.filters.iter().any(super::arrange::names_mime_types);
        self.listing_task = Some(cx.spawn(async move |this, cx| {
            let listed = oxyde_lib::commands::io::list_dir(folder.clone(), Some(show_hidden), Some(show_system)).await;
            let content_types = match &listed {
                Ok(entries) if typed => {
                    let files: Vec<String> = entries.iter().filter(|entry| !entry.is_dir).map(|entry| entry.path.clone()).collect();
                    cx.background_spawn(async move { content_types(files) }).await
                }
                _ => HashMap::new(),
            };
            let _ = this.update(cx, |this, cx| {
                match listed {
                    Ok(entries) => {
                        this.entries = Arc::new(entries);
                        this.admission.content_types = Arc::new(content_types);
                        this.arrange(keep, cx);
                        this.check_subfolders(cx);
                    }
                    Err(error) => {
                        log::warn!("The file chooser cannot list {folder}: {error:?}");
                        this.entries = Arc::new(Vec::new());
                        this.shown = Arc::new(Vec::new());
                        this.folders = 0;
                        this.selection.clear();
                        this.listing = Listing::Failed(crate::ui::errors::path_load_message(&error));
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn check_subfolders(&mut self, cx: &mut Context<Self>) {
        let folder = self.folder.clone();
        self.subfolder_task = Some(cx.spawn(async move |this, cx| {
            let Ok(nodes) = oxyde_lib::commands::sidebar::get_sidebar_nodes(folder.clone()).await else { return };
            let _ = this.update(cx, |this, cx| {
                if this.folder == folder {
                    this.update_tree(&folder, &nodes);
                    this.subfolders = nodes.into_iter().map(|node| (node.path, node.has_subdirs)).collect();
                    cx.notify();
                }
            });
        }));
    }

    /// Filters and sorts the entries off the UI thread, then selects `keep` again.
    pub(super) fn arrange(&mut self, keep: Kept, cx: &mut Context<Self>) {
        let entries = self.entries.clone();
        let filter = self.filter.text.clone();
        let admission = self.admission.clone();
        let (sort, direction) = (self.options.sort, self.options.direction);
        self.arrange_task = Some(cx.spawn(async move |this, cx| {
            let arranged = cx.background_spawn(async move {
                let platform = oxyde_lib::platform::platform();
                super::arrange::arrange(&entries, &filter, &admission, &|mime, pattern| platform.content_type_matches(mime, pattern), sort, direction)
            }).await;
            let _ = this.update(cx, |this, cx| {
                let filtered = !this.filter.text.is_empty();
                this.selection = Selection::restore(&keep, &arranged.shown);
                // Filtering, the first match is ready for Enter.
                if this.selection.is_empty() && filtered && !arranged.shown.is_empty() {
                    this.selection.only(0);
                }
                this.folders = arranged.folders;
                this.shown = Arc::new(arranged.shown);
                if this.column_values.0 != arranged.types || this.column_values.1 != arranged.sizes {
                    this.column_values = Arc::new((arranged.types, arranged.sizes));
                    this.column_revision += 1;
                }
                this.listing = Listing::Ready;
                if let Some(position) = this.selection.cursor() {
                    this.reveal(position);
                }
                cx.notify();
            });
        }));
    }

    /// The selection by path, to find again once the entries are arranged.
    pub(super) fn kept(&self) -> Kept {
        self.selection.keep(&self.shown)
    }

    pub(super) fn set_filter(&mut self, value: &str, cx: &mut Context<Self>) {
        if self.filter.text != value {
            self.filter.text = value.to_string();
            let keep = self.kept();
            self.arrange(keep, cx);
        }
    }

    /// A header's press: its column sorts, or sorts the other way, as `toggle_sort`.
    pub(super) fn toggle_sort(&mut self, column: SortColumn, cx: &mut Context<Self>) {
        if self.options.sort == column {
            self.options.direction = match self.options.direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            };
        } else {
            self.options.sort = column;
            self.options.direction = SortDirection::Ascending;
        }
        let keep = self.kept();
        self.arrange(keep, cx);
        cx.notify();
    }

    pub(super) fn toggle_hidden(&mut self, cx: &mut Context<Self>) {
        self.show_hidden = !self.show_hidden;
        self.refresh(cx);
    }

    pub(super) fn set_grid(&mut self, grid: bool, cx: &mut Context<Self>) {
        if self.options.grid != grid {
            self.options.grid = grid;
            if let Some(position) = self.selection.cursor() {
                self.reveal(position);
            }
            cx.notify();
        }
    }

    /// Cards per grid row, from the pane's width as the main grid computes it.
    pub(super) fn grid_geometry(&self) -> crate::ui::style::GridGeometry {
        crate::ui::style::GridGeometry::for_viewport(self.pane_width.get().max(1.0), crate::ui::components::scrollbar::track_width())
    }

    /// Rows of positions as the arrows move through them: one entry per details row,
    /// a line of cards in the grid.
    pub(super) fn position_rows(&self) -> Vec<Vec<usize>> {
        let columns = if self.options.grid { self.grid_geometry().columns.max(1) } else { 1 };
        (0..self.shown.len()).collect::<Vec<_>>().chunks(columns).map(<[usize]>::to_vec).collect()
    }

    /// Scrolls the entry into view, as `scroll_row_into_view` does.
    fn reveal(&self, position: usize) {
        if !self.options.grid {
            self.list.scroll_to_item(position, ScrollStrategy::Nearest);
            return;
        }
        let row = position / self.grid_geometry().columns.max(1);
        let height = crate::ui::style::grid_card_row_height();
        // The grid's list starts below a 0.25 rem top padding.
        let top = row as f32 * height + crate::ui::scale::rem_pixels(rems(0.25)).as_f32();
        let handle = &self.grid_scroll;
        let viewport = handle.bounds().size.height.as_f32().max(1.0);
        let visible_top = -handle.offset().y.as_f32();
        let target = if top < visible_top { top }
            else if top + height > visible_top + viewport { (top + height - viewport).max(0.0) }
            else { visible_top };
        handle.set_offset(point(px(0.0), px(-target)));
    }

    /// The entry alone, as a plain press or arrow selects it.
    pub(super) fn select(&mut self, position: usize, cx: &mut Context<Self>) {
        self.change_selection(position, cx, Selection::only);
    }

    /// A press with its modifiers, which choose several entries when the application
    /// allows it: Ctrl toggles one, Shift extends from the anchor, both add the range.
    pub(super) fn press_entry(&mut self, position: usize, modifiers: Modifiers, cx: &mut Context<Self>) {
        match (self.multiple, modifiers.control, modifiers.shift) {
            (true, true, false) => self.change_selection(position, cx, Selection::toggle),
            (true, add, true) => self.change_selection(position, cx, |selection, position| selection.extend(position, add)),
            _ => self.select(position, cx),
        }
    }

    /// Shift and an arrow: the range from the anchor, when several may be chosen.
    pub(super) fn extend_to(&mut self, position: usize, cx: &mut Context<Self>) {
        if self.multiple {
            self.change_selection(position, cx, |selection, position| selection.extend(position, false));
        } else {
            self.select(position, cx);
        }
    }

    pub(super) fn select_all(&mut self, cx: &mut Context<Self>) {
        if self.multiple {
            self.selection.all(self.shown.len());
            cx.notify();
        }
    }

    fn change_selection(&mut self, position: usize, cx: &mut Context<Self>, change: impl FnOnce(&mut Selection, usize)) {
        if position < self.shown.len() {
            change(&mut self.selection, position);
            self.take_name_of(position, cx);
            self.reveal(position);
            cx.notify();
        }
    }

    /// Enters a folder, or chooses a file, or saves over it under its name; an archive
    /// is a file here.
    pub(super) fn open_entry(&mut self, position: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.shown_entry(position) else { return };
        if entry.is_dir {
            let path = entry.path.clone();
            self.show_folder(path, cx);
        } else if self.saving() {
            self.take_name_of(position, cx);
            self.save(window, cx);
        } else {
            let path = oxyde_lib::utils::path_text::decode(&entry.path);
            self.finish(Some(vec![path]), window, cx);
        }
    }

    /// Enter: a single entry opens, as on a double press; several are chosen.
    pub(super) fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.selection.single() {
            Some(position) => self.open_entry(position, window, cx),
            None => self.accept(window, cx),
        }
    }

    /// Whether the button that accepts has something to choose, or a name to save under.
    pub(super) fn can_accept(&self, cx: &App) -> bool {
        if self.awaits_interaction() {
            return false;
        }
        match self.purpose {
            FileChooserPurpose::Save { .. } => !self.typed_name(cx).is_empty() || self.selected_folder().is_some(),
            _ => self.directory || !self.selection.is_empty(),
        }
    }

    /// The button that accepts. Folders: those selected, else the one open; saving
    /// several files, the one folder that holds them. Files: the files selected; a
    /// single folder selected opens instead, as in GTK. Saving one file: the name typed.
    pub(super) fn accept(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.purpose, FileChooserPurpose::Save { .. }) {
            self.save(window, cx);
            return;
        }
        let selected: Vec<&FileEntry> = self.selection.positions().filter_map(|position| self.shown_entry(position)).collect();
        let decode = |entry: &&FileEntry| oxyde_lib::utils::path_text::decode(&entry.path);
        if self.directory {
            let chosen: Vec<PathBuf> = if selected.is_empty() {
                vec![oxyde_lib::utils::path_text::decode(&self.folder)]
            } else {
                selected.iter().map(decode).collect()
            };
            match (&self.purpose, &chosen[..]) {
                (FileChooserPurpose::SaveFiles { .. }, [folder]) => self.save_files(folder.clone(), window, cx),
                (FileChooserPurpose::SaveFiles { .. }, _) => {}
                _ => self.finish(Some(chosen), window, cx),
            }
            return;
        }
        let files: Vec<PathBuf> = selected.iter().filter(|entry| !entry.is_dir).map(decode).collect();
        if !files.is_empty() {
            self.finish(Some(files), window, cx);
        } else if let [folder] = selected[..] {
            let path = folder.path.clone();
            self.show_folder(path, cx);
        }
    }
}

/// The MIME types of files, by their listed paths.
fn content_types(files: Vec<String>) -> HashMap<String, &'static str> {
    let platform = oxyde_lib::platform::platform();
    files.into_iter().filter_map(|path| {
        let mime = platform.content_type(&oxyde_lib::utils::path_text::decode(&path), false)?;
        Some((path, mime))
    }).collect()
}
