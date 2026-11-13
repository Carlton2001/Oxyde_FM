//! The file chooser's keyboard, the main window's file keys where they apply to
//! choosing (`handle_file_key`, `type_select`), Shift with the arrows and Ctrl+A when
//! several entries may be chosen, Ctrl+Shift+A to select nothing, Ctrl+Shift+N for a
//! new folder, plus Ctrl+L and Ctrl+H, as other file choosers have them.
use super::FileChooserWindow;
use crate::ui::app::type_select::{find_match, RESET_DELAY};
use gpui::*;
use std::time::Instant;

impl FileChooserWindow {
    pub(super) fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.shortcut(&event.keystroke, window, cx) || self.footer_key(&event.keystroke, window, cx) {
            cx.stop_propagation();
        } else if self.focus.is_focused(window) && self.list_key(&event.keystroke, window, cx) {
            self.note_interaction(cx);
            cx.stop_propagation();
        }
    }

    /// Keys that work wherever the keyboard is in the window.
    fn shortcut(&mut self, key: &Keystroke, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let m = key.modifiers;
        if m.platform || m.function { return false; }
        match (key.key.as_str(), m.control, m.alt, m.shift) {
            ("f5", false, false, false) | ("r", true, false, false) => self.refresh(cx),
            ("f", true, false, false) => self.open_filter(window, cx),
            ("l", true, false, false) => self.begin_location(None, window, cx),
            ("h", true, false, false) => self.toggle_hidden(cx),
            ("n", true, false, true) => self.create_folder(window, cx),
            // As the navigation bar's tooltips give them.
            ("left", false, true, false) => self.go_back(cx),
            ("right", false, true, false) => self.go_forward(cx),
            ("up", false, true, false) => self.go_up(cx),
            _ => return false,
        }
        true
    }

    /// Keys of the list itself, as the main window's file list takes them.
    fn list_key(&mut self, key: &Keystroke, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let m = key.modifiers;
        if m.platform || m.function { return false; }
        match (key.key.as_str(), m.control, m.alt, m.shift) {
            ("up" | "down" | "left" | "right", false, false, shift) => self.move_selection(key.key.as_str(), shift, cx),
            ("a", true, false, false) if self.multiple => self.select_all(cx),
            // Ctrl+Shift+A selects nothing, as in the main window.
            ("a", true, false, true) => {
                if self.selection.clear() { cx.notify(); }
            }
            ("enter", false, false, false) => self.activate(window, cx),
            ("backspace", false, false, false) => {
                if !self.back.is_empty() { self.go_back(cx); } else { self.go_up(cx); }
            }
            ("escape", false, false, false) => self.finish(None, window, cx),
            _ => {
                if m.control || m.alt { return false; }
                let Some(text) = key.key_char.as_deref() else { return false };
                return self.type_select(text, window, cx);
            }
        }
        true
    }

    /// The arrows, which leave left and right to the grid; Shift extends the selection.
    pub(super) fn move_selection(&mut self, key: &str, shift: bool, cx: &mut Context<Self>) {
        if !self.options.grid && matches!(key, "left" | "right") { return; }
        let rows = self.position_rows();
        let rows: Vec<&[usize]> = rows.iter().map(Vec::as_slice).collect();
        if let Some(position) = crate::ui::app::selection::arrow_target(&rows, self.selection.cursor(), key) {
            if shift { self.extend_to(position, cx) } else { self.select(position, cx) }
        }
    }

    /// Typed letters move the selection to the next matching name; `/` or `~` first
    /// starts typing a path, as other file choosers do.
    fn type_select(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if text.is_empty() || text.chars().any(char::is_control) { return false; }
        let fresh = self.typed.last.is_none_or(|last| last.elapsed() > RESET_DELAY);
        // A leading space is not a name search.
        if (self.typed.prefix.is_empty() || fresh) && text == " " { return false; }
        if (self.typed.prefix.is_empty() || fresh) && (text == "/" || text == "~") {
            self.begin_location(Some(text.to_string()), window, cx);
            return true;
        }
        let prefix = self.typed.push(text, Instant::now()).to_owned();
        let names: Vec<&str> = self.shown.iter().map(|entry| entry.name.as_str()).collect();
        if let Some(position) = find_match(&names, self.selection.cursor(), &prefix) {
            self.select(position, cx);
        }
        true
    }
}
