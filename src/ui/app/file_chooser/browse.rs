//! Browsing in the file chooser: listings read in the background, the selection, and
//! what opening an entry does, entering a folder or choosing a file.
use super::{FileChooserWindow, Listing};
use gpui::*;
use std::path::Path;
use std::sync::Arc;

impl FileChooserWindow {
    /// Lists `folder` off the UI thread; a later navigation drops this listing.
    pub(super) fn navigate(&mut self, folder: String, cx: &mut Context<Self>) {
        let previous = std::mem::replace(&mut self.folder, folder.clone());
        self.entries = Arc::new(Vec::new());
        self.listing = Listing::Loading;
        self.scroll.set_offset(point(px(0.0), px(0.0)));
        let (show_hidden, show_system) = (self.show_hidden, self.show_system);
        self.listing_task = Some(cx.spawn(async move |this, cx| {
            let listed = oxyde_lib::commands::io::list_dir(folder.clone(), Some(show_hidden), Some(show_system)).await;
            let _ = this.update(cx, |this, cx| {
                match listed {
                    Ok(entries) => {
                        // Back to the parent, the folder just left stays selected.
                        this.selected = entries.iter().position(|entry| entry.path == previous);
                        if let Some(index) = this.selected {
                            this.list.scroll_to_item(index, ScrollStrategy::Center);
                        }
                        this.entries = Arc::new(entries);
                        this.listing = Listing::Ready;
                    }
                    Err(error) => {
                        log::warn!("The file chooser cannot list {folder}: {error:?}");
                        this.selected = None;
                        this.listing = Listing::Failed;
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn parent(&self) -> Option<String> {
        Path::new(&self.folder).parent().map(|parent| parent.to_string_lossy().into_owned())
    }

    pub(super) fn go_up(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = self.parent() {
            self.navigate(parent, cx);
        }
    }

    pub(super) fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.entries.len() {
            self.selected = Some(index);
            self.list.scroll_to_item(index, ScrollStrategy::Nearest);
            cx.notify();
        }
    }

    /// Moves the selection by `step` rows, from the first or last row without one.
    pub(super) fn move_selection(&mut self, step: isize, cx: &mut Context<Self>) {
        let Some(last) = self.entries.len().checked_sub(1) else { return };
        let index = match self.selected {
            Some(index) => index.saturating_add_signed(step).min(last),
            None if step < 0 => last,
            None => 0,
        };
        self.select(index, cx);
    }

    /// Enters a folder, or chooses a file; an archive is a file here.
    pub(super) fn open_entry(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.entries.get(index) else { return };
        if entry.is_dir {
            let path = entry.path.clone();
            self.navigate(path, cx);
        } else {
            let path = oxyde_lib::utils::path_text::decode(&entry.path);
            self.finish(Some(vec![path]), window, cx);
        }
    }

    pub(super) fn accept(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.selected {
            self.open_entry(index, window, cx);
        }
    }

    pub(super) fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let page = (self.scroll.bounds().size.height.as_f32() / crate::ui::style::row_height()).floor().max(1.0) as isize;
        match keystroke.key.as_str() {
            "down" => self.move_selection(1, cx),
            "up" if keystroke.modifiers.alt => self.go_up(cx),
            "up" => self.move_selection(-1, cx),
            "pagedown" => self.move_selection(page, cx),
            "pageup" => self.move_selection(-page, cx),
            "home" => self.select(0, cx),
            "end" => self.move_selection(isize::MAX, cx),
            "backspace" => self.go_up(cx),
            "enter" => self.accept(window, cx),
            "escape" => self.finish(None, window, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    pub(super) fn move_scrollbar(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some((track, viewport, content)) = self.scroll_drag.clone() else { return };
        if !event.dragging() {
            self.scroll_drag = None;
            cx.notify();
            return;
        }
        let Some(origin) = self.scrollbar_origins.get(&track) else { return };
        let thumb = (viewport * (viewport / content.max(1.0)).min(1.0))
            .max(crate::ui::scale::rem_pixels(rems(2.5)).as_f32());
        let travel = (viewport - thumb).max(1.0);
        let position = (event.position.y.as_f32() - origin.y - self.grab_offset).clamp(0.0, travel);
        self.scroll.set_offset(point(px(0.0), px(-position / travel * (content - viewport).max(0.0))));
        cx.notify();
    }
}
