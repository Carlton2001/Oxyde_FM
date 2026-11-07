//! The file chooser's keyboard: shortcuts wherever the keyboard is, then the list's
//! own keys, where typing a name selects it and typing a path edits the location.
use super::FileChooserWindow;
use gpui::*;
use std::time::{Duration, Instant};

/// Letters typed within this delay of each other make one name.
const TYPING_PAUSE: Duration = Duration::from_secs(1);

impl FileChooserWindow {
    pub(super) fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.shortcut(&event.keystroke, window, cx) || (self.focus.is_focused(window) && self.list_key(&event.keystroke, window, cx)) {
            cx.stop_propagation();
        }
    }

    fn shortcut(&mut self, keystroke: &Keystroke, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let modifiers = keystroke.modifiers;
        let only = |control: bool, alt: bool| modifiers.control == control && modifiers.alt == alt && !modifiers.shift && !modifiers.platform;
        match keystroke.key.as_str() {
            "l" if only(true, false) => self.begin_location(None, window, cx),
            "f" if only(true, false) => {
                let focus = self.filter.read(cx).focus_handle.clone();
                window.focus(&focus, cx);
            }
            "h" if only(true, false) => self.toggle_hidden(cx),
            "r" if only(true, false) => self.refresh(cx),
            "f5" if only(false, false) => self.refresh(cx),
            "left" if only(false, true) => self.go_back(cx),
            "right" if only(false, true) => self.go_forward(cx),
            "up" if only(false, true) => self.go_up(cx),
            _ => return false,
        }
        true
    }

    /// Cards per row in the grid, one entry per row in the details.
    fn line(&self) -> isize {
        if self.options.grid { self.columns.max(1) as isize } else { 1 }
    }

    fn page(&self) -> isize {
        let row = if self.options.grid { crate::ui::style::grid_card_row_height() } else { crate::ui::style::row_height() };
        let rows = (self.scroll.bounds().size.height.as_f32() / row).floor().max(1.0) as isize;
        rows * self.line()
    }

    /// Moves the selection a line up or down, as the filter's arrows also do.
    pub(super) fn move_line(&mut self, down: bool, cx: &mut Context<Self>) {
        let line = self.line();
        self.move_selection(if down { line } else { -line }, cx);
    }

    fn list_key(&mut self, keystroke: &Keystroke, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let modifiers = keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform {
            return false;
        }
        match keystroke.key.as_str() {
            "down" => self.move_line(true, cx),
            "up" => self.move_line(false, cx),
            "right" if self.options.grid => self.move_selection(1, cx),
            "left" if self.options.grid => self.move_selection(-1, cx),
            "pagedown" => self.move_selection(self.page(), cx),
            "pageup" => self.move_selection(-self.page(), cx),
            "home" => self.select(0, cx),
            "end" => self.move_selection(isize::MAX, cx),
            "backspace" => self.go_up(cx),
            "enter" => self.accept(window, cx),
            "escape" if !self.filter_text.is_empty() => self.clear_filter(window, cx),
            "escape" => self.finish(None, window, cx),
            _ => {
                let Some(text) = keystroke.key_char.as_deref().filter(|text| !text.chars().any(char::is_control)) else { return false };
                self.type_name(text, window, cx);
            }
        }
        true
    }

    /// Selects the first name starting with the letters typed; a path starts the location.
    fn type_name(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let now = Instant::now();
        if now.duration_since(self.typed.1) > TYPING_PAUSE {
            self.typed.0.clear();
        }
        self.typed.1 = now;
        if self.typed.0.is_empty() && (text == "/" || text == "~") {
            self.begin_location(Some(text.to_string()), window, cx);
            return;
        }
        if self.typed.0.is_empty() && text == " " {
            return;
        }
        self.typed.0.push_str(text);
        if let Some(position) = super::arrange::find_prefix(&self.entries, &self.shown, &self.typed.0) {
            self.select(position, cx);
        }
    }
}
