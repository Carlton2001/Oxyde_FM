use super::*;

impl OxydeApp {
    pub(super) fn file_keyboard_enabled(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
            && !self.settings_open
            && self.network_dialog.is_none()
            && self.trash_settings.is_none() && self.editing_path.is_none()
            && self.settings_confirmation.is_none() && self.network_force_confirmation.is_none()
            && self.pending_trash_delete.is_none() && self.pending_file_operations.is_empty()
            && self.file_operation_notice.is_none()
    }

    pub(super) fn refresh_active_view(&mut self, window: &Window, cx: &mut Context<Self>) {
        if !self.file_keyboard_enabled(window) { return; }
        let id = self.state.active_pane_id.clone();
        if let Some(pane) = self.state.find_pane(&id) {
            let index = pane.active_tab_index;
            let path = pane.active_tab().path.clone();
            self.refresh_tab_files(&id, index, path, cx);
        }
    }

    pub(super) fn handle_file_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.file_keyboard_enabled(window) { return false; }
        let key = &event.keystroke;
        let m = key.modifiers;
        if m.platform || m.function { return false; }
        // Menus retain the selected files and active pane. Dismiss them when a
        // file shortcut is used, while leaving unrelated keys to the menu.
        let recognized = matches!((key.key.as_str(), m.control, m.alt, m.shift),
            ("up" | "down" | "left" | "right" | "delete" | "tab", false, false, _)
            | ("enter" | "f2" | "backspace" | "f5", false, false, false)
            | ("enter" | "up", false, true, false)
            | ("c" | "z" | "n" | "tab" | "pageup" | "pagedown" | "+" | "=" | "add", true, false, _)
            | ("a" | "x" | "v" | "y" | "r" | "f" | "-" | "subtract" | "0", true, false, false)
            | ("c", true, true, false));
        if !recognized { return false; }
        if !self.context_menus.is_empty() {
            self.context_menus.clear();
            cx.notify();
        }
        let pane_id = self.state.active_pane_id.clone();
        let Some(pane) = self.state.find_pane(&pane_id) else { return false; };
        let path = pane.active_tab().path.clone();
        let parent = std::path::Path::new(&path).parent()
            .filter(|p| !p.as_os_str().is_empty()).map(|p| p.to_string_lossy().into_owned());
        match (key.key.as_str(), m.control, m.alt, m.shift) {
            ("up" | "down" | "left" | "right", false, false, shift) => self.move_file_selection(key.key.as_str(), shift, cx),
            ("a", true, false, false) => self.execute_tout_selectionner(cx),
            ("c", true, false, false) => self.execute_copier(cx),
            ("c", true, false, true) => self.execute_copy_text(false, cx),
            ("c", true, true, false) => self.execute_copy_text(true, cx),
            ("x", true, false, false) => self.execute_couper(cx),
            ("v", true, false, false) => self.execute_coller(cx),
            ("z", true, false, false) => self.execute_undo(cx),
            ("y", true, false, false) | ("z", true, false, true) => self.execute_redo(cx),
            ("delete", false, false, shift) => {
                self.shift_delete = shift;
                self.on_action_supprimer(&Supprimer, window, cx);
            }
            ("f5", false, false, false) | ("r", true, false, false) => self.refresh_active_view(window, cx),
            ("enter", false, false, false) => self.execute_ouvrir(cx),
            ("f2", false, false, false) => self.execute_renommer(window, cx),
            ("enter", false, true, false) => {
                if let Some(path) = pane.active_tab().selected_paths.iter().next().cloned() {
                    self.open_file_properties(path, window, cx);
                }
            }
            ("backspace", false, false, false) => {
                if !pane.active_tab().back_stack.is_empty() { self.go_back(&pane_id, cx); }
                else if let Some(parent) = parent { self.navigate_to(&pane_id, parent, cx); }
            }
            ("up", false, true, false) => {
                if let Some(parent) = parent { self.navigate_to(&pane_id, parent, cx); }
            }
            ("f", true, false, false) => self.open_quick_search(&pane_id, window, cx),
            ("n", true, false, false) => self.add_panel_right(cx),
            ("n", true, false, true) => self.create_new_folder(&pane_id, cx),
            ("tab", true, false, previous) | ("pageup", true, false, previous) | ("pagedown", true, false, previous) => {
                let previous = previous || key.key == "pageup";
                let count = pane.tabs.len();
                let index = (pane.active_tab_index + if previous { count - 1 } else { 1 }) % count;
                self.switch_tab(&pane_id, index, cx);
            }
            ("tab", false, false, previous) => {
                let mut ids = Vec::new();
                self.state.layout.visit_panes(&mut |pane| ids.push(pane.id.clone()));
                if let Some(index) = ids.iter().position(|id| id == &pane_id) {
                    let next = (index + if previous { ids.len() - 1 } else { 1 }) % ids.len();
                    self.activate_pane(&ids[next], cx);
                    cx.notify();
                }
            }
            ("+" | "=" | "add", true, false, _) => self.set_ui_rem_size(self.ui_rem_size() + 1, window, cx),
            ("-" | "subtract", true, false, false) => self.set_ui_rem_size(self.ui_rem_size().saturating_sub(1), window, cx),
            ("0", true, false, false) => self.set_ui_rem_size(16, window, cx),
            _ => return false,
        }
        true
    }
}
