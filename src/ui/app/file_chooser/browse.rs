//! Browsing in the file chooser: the history, listings read and arranged in the
//! background, the selection, and what opening an entry does, entering a folder or
//! choosing a file.
use super::arrange::{self, SortKey};
use super::{FileChooserWindow, Listing};
use gpui::*;
use std::path::Path;
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

    pub(super) fn parent(&self) -> Option<String> {
        Path::new(&self.folder).parent().map(|parent| parent.to_string_lossy().into_owned())
    }

    pub(super) fn go_up(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = self.parent() {
            self.show_folder(parent, cx);
        }
    }

    fn go(&mut self, folder: String, step: Step, cx: &mut Context<Self>) {
        let left = std::mem::replace(&mut self.folder, folder);
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
        // A new folder starts unfiltered, its path no longer typed.
        self.location = None;
        self.location_missing = false;
        self.location_task = None;
        if !self.filter_text.is_empty() {
            self.filter_text.clear();
            self.filter.update(cx, |input, cx| input.set_content(String::new(), cx));
        }
        self.entries = Arc::new(Vec::new());
        self.shown = Arc::new(Vec::new());
        self.selected = None;
        self.scroll.set_offset(point(px(0.0), px(0.0)));
        // Clamped once laid out: the last crumbs show.
        self.crumbs.set_offset(point(px(-1.0e6), px(0.0)));
        // Back in a parent, the folder just left stays selected.
        self.list(Some(left), cx);
    }

    /// Lists the folder again, the selection kept.
    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
        let keep = self.selected_path();
        self.list(keep, cx);
    }

    /// Lists the folder off the UI thread; a later navigation drops this listing.
    fn list(&mut self, keep: Option<String>, cx: &mut Context<Self>) {
        self.listing = Listing::Loading;
        self.arrange_task = None;
        let folder = self.folder.clone();
        let (show_hidden, show_system) = (self.show_hidden, self.show_system);
        self.listing_task = Some(cx.spawn(async move |this, cx| {
            let listed = oxyde_lib::commands::io::list_dir(folder.clone(), Some(show_hidden), Some(show_system)).await;
            let _ = this.update(cx, |this, cx| {
                match listed {
                    Ok(entries) => {
                        this.entries = Arc::new(entries);
                        this.arrange(keep, cx);
                    }
                    Err(error) => {
                        log::warn!("The file chooser cannot list {folder}: {error:?}");
                        this.entries = Arc::new(Vec::new());
                        this.shown = Arc::new(Vec::new());
                        this.selected = None;
                        this.listing = Listing::Failed;
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Filters and sorts the entries off the UI thread, then selects `keep` again.
    pub(super) fn arrange(&mut self, keep: Option<String>, cx: &mut Context<Self>) {
        let entries = self.entries.clone();
        let filter = self.filter_text.clone();
        let (key, descending) = (self.options.sort, self.options.descending);
        self.arrange_task = Some(cx.spawn(async move |this, cx| {
            let shown = cx.background_spawn(async move { arrange::arrange(&entries, &filter, key, descending) }).await;
            let _ = this.update(cx, |this, cx| {
                let filtered = !this.filter_text.trim().is_empty();
                this.selected = keep.and_then(|path| shown.iter().position(|&index| this.entries[index].path == path))
                    // Filtering, the first match is ready for Enter.
                    .or_else(|| (filtered && !shown.is_empty()).then_some(0));
                this.shown = Arc::new(shown);
                this.listing = Listing::Ready;
                if let Some(position) = this.selected {
                    this.reveal(position);
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn selected_path(&self) -> Option<String> {
        self.shown_entry(self.selected?).map(|entry| entry.path.clone())
    }

    pub(super) fn set_filter(&mut self, value: &str, cx: &mut Context<Self>) {
        if self.filter_text != value {
            self.filter_text = value.to_string();
            let keep = self.selected_path();
            self.arrange(keep, cx);
        }
    }

    /// Escape in the filter: empties it, then hands the keyboard back to the list.
    pub(super) fn clear_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.filter_text.is_empty() {
            self.filter.update(cx, |input, cx| input.set_content(String::new(), cx));
            self.set_filter("", cx);
        }
        window.focus(&self.focus, cx);
    }

    /// A header's click: its column sorts, or sorts the other way.
    pub(super) fn sort_by(&mut self, key: SortKey, cx: &mut Context<Self>) {
        if self.options.sort == key {
            self.options.descending = !self.options.descending;
        } else {
            self.options.sort = key;
            self.options.descending = false;
        }
        let keep = self.selected_path();
        self.arrange(keep, cx);
    }

    pub(super) fn toggle_hidden(&mut self, cx: &mut Context<Self>) {
        self.show_hidden = !self.show_hidden;
        self.refresh(cx);
    }

    pub(super) fn toggle_grid(&mut self, cx: &mut Context<Self>) {
        self.options.grid = !self.options.grid;
        self.scroll.set_offset(point(px(0.0), px(0.0)));
        if let Some(position) = self.selected {
            self.reveal(position);
        }
        cx.notify();
    }

    /// The rows of the list: entries, or grid lines of cards.
    pub(super) fn rows(&self) -> usize {
        if self.options.grid { self.shown.len().div_ceil(self.columns.max(1)) } else { self.shown.len() }
    }

    fn reveal(&self, position: usize) {
        let row = if self.options.grid { position / self.columns.max(1) } else { position };
        self.list.scroll_to_item(row, ScrollStrategy::Nearest);
    }

    pub(super) fn select(&mut self, position: usize, cx: &mut Context<Self>) {
        if position < self.shown.len() {
            self.selected = Some(position);
            self.reveal(position);
            cx.notify();
        }
    }

    /// Moves the selection by `step` entries, from the first or last without one.
    pub(super) fn move_selection(&mut self, step: isize, cx: &mut Context<Self>) {
        let Some(last) = self.shown.len().checked_sub(1) else { return };
        let position = match self.selected {
            Some(position) => position.saturating_add_signed(step).min(last),
            None if step < 0 => last,
            None => 0,
        };
        self.select(position, cx);
    }

    /// Enters a folder, or chooses a file; an archive is a file here.
    pub(super) fn open_entry(&mut self, position: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.shown_entry(position) else { return };
        if entry.is_dir {
            let path = entry.path.clone();
            self.show_folder(path, cx);
        } else {
            let path = oxyde_lib::utils::path_text::decode(&entry.path);
            self.finish(Some(vec![path]), window, cx);
        }
    }

    pub(super) fn accept(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(position) = self.selected {
            self.open_entry(position, window, cx);
        }
    }
}
