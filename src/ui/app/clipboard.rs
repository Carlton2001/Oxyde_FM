use super::{Coller, Copier, Couper, OxydeApp, ToastKind};
use crate::ui::i18n::Message;
use gpui::*;

impl OxydeApp {
    pub fn on_action_copier(&mut self, _: &Copier, _: &mut Window, cx: &mut Context<Self>) {
        self.execute_copier(cx);
    }

    pub(crate) fn execute_copy_text(&mut self, names_only: bool, cx: &mut Context<Self>) {
        let Some(pane) = self.state.find_pane(&self.state.active_pane_id) else { return; };
        let tab = pane.active_tab();
        let lines = tab.filtered_files().iter()
            .filter(|file| tab.selected_paths.contains(&file.path))
            .map(|file| if names_only { file.name.clone() } else { file.path.clone() })
            .collect::<Vec<_>>();
        // Counted by item: a name may hold a line break.
        let count = lines.len();
        let text = lines.join("\n");
        if !text.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.toast(ToastKind::Info, if names_only { Message::ToastNamesCopied } else { Message::ToastPathsCopied }, String::new(), count, cx);
        }
    }

    pub fn execute_copier(&mut self, cx: &mut Context<Self>) { self.copy_files_to_clipboard(false, cx); }
    pub fn on_action_couper(&mut self, _: &Couper, _: &mut Window, cx: &mut Context<Self>) { self.execute_couper(cx); }
    pub fn execute_couper(&mut self, cx: &mut Context<Self>) { self.copy_files_to_clipboard(true, cx); }
    fn copy_files_to_clipboard(&mut self, cut: bool, cx: &mut Context<Self>) {
        let Some(pane) = self.state.find_pane(&self.state.active_pane_id) else { return; };
        let paths = pane.active_tab().actionable_selection();
        if paths.is_empty() { return; }
        // Cutting from a read-only location would move nothing: menus hide it, keys do nothing.
        if cut && paths.iter().any(|path| self.is_read_only_location(&self.state.active_pane_id, path)) { return; }
        let text = paths.join("\n");
        let offers_files = oxyde_lib::platform::platform().file_clipboard().interface_offers_files(&paths);
        let item = if offers_files { files_item(&paths, text, cut) } else { ClipboardItem::new_string(text) };
        let gate = self.clipboard_gate.clone();
        cx.spawn(async move |this, cx| {
            let _turn = gate.lock().await;
            let result = tokio::task::spawn_blocking(move || oxyde_lib::platform::platform().file_clipboard().set_files(paths, cut)).await;
            let _ = this.update(cx, |this, cx| {
                // Applications the platform cannot offer files to receive their paths.
                if matches!(result, Ok(Ok(true))) {
                    cx.write_to_clipboard(item);
                }
                match result {
                    Ok(Ok(_)) => this.toast(ToastKind::Info, if cut { Message::ToastCut } else { Message::ToastCopied }, String::new(), 0, cx),
                    Ok(Err(error)) => this.toast(ToastKind::Error, Message::ToastFailed, error, 0, cx),
                    Err(error) => this.toast(ToastKind::Error, Message::ToastFailed, error.to_string(), 0, cx),
                }
            });
        }).detach();
    }

    pub fn on_action_coller(&mut self, _: &Coller, _: &mut Window, cx: &mut Context<Self>) {
        self.execute_coller(cx);
    }

    pub fn execute_coller(&mut self, cx: &mut Context<Self>) {
        let Some(pane) = self.state.find_pane(&self.state.active_pane_id) else { return; };
        let target_dir = pane.active_tab().path.clone();
        if target_dir.starts_with("search://") || target_dir == "trash://"
            || self.is_read_only_location(&self.state.active_pane_id, &target_dir) { return; }
        #[cfg(target_os = "windows")]
        let text = None;
        #[cfg(not(target_os = "windows"))]
        let text = {
            // Files another application announces are read on the worker below: reading
            // here would hold the interface while their owner does not answer.
            let announced = oxyde_lib::platform::platform().file_clipboard().external_files_types()
                .iter().any(|mime_type| cx.clipboard_offers(mime_type));
            let system = if announced { None } else { cx.read_from_clipboard() };
            // Files another application offers through GPUI come first, as in Zed. Oxyde's
            // own files go through its file clipboard, which knows whether they are cut.
            if let Some(paths) = system.as_ref().filter(|item| !own_files(item)).and_then(external_paths) {
                self.execute_file_copy(paths, target_dir, cx);
                return;
            }
            system.and_then(|item| item.text())
        };
        let gate = self.clipboard_gate.clone();
        cx.spawn(async move |this, cx| {
            // A copy or cut asked before this paste finishes writing first.
            let _turn = gate.lock().await;
            let target = target_dir.clone();
            let result = tokio::task::spawn_blocking(move || {
                let clipboard = oxyde_lib::platform::platform().file_clipboard();
                let files = clipboard.get_files(text)?;
                // Files Oxyde cannot read go through the system's own Paste.
                if files.0.is_empty() { clipboard.paste_with_system(&target)?; }
                Ok::<_, String>(files)
            }).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(Ok((paths, is_cut))) if !paths.is_empty() => {
                        if is_cut { this.execute_file_move(paths, target_dir, cx); }
                        else { this.execute_file_copy(paths, target_dir, cx); }
                    }
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => this.toast(ToastKind::Error, Message::ToastFailed, error, 0, cx),
                    Err(error) => this.toast(ToastKind::Error, Message::ToastFailed, error.to_string(), 0, cx),
                }
            });
        }).detach();
    }
}

/// The text entry's metadata of files Oxyde offers; GPUI's Wayland clipboard reads "cut".
const COPIED: &str = "copy";
const CUT: &str = "cut";

/// The paths as text, and as files for the platforms whose GPUI clipboard offers them.
fn files_item(paths: &[String], text: String, cut: bool) -> ClipboardItem {
    let metadata = if cut { CUT } else { COPIED };
    ClipboardItem {
        entries: vec![
            ClipboardEntry::String(ClipboardString { text, metadata: Some(metadata.into()) }),
            ClipboardEntry::ExternalPaths(ExternalPaths(paths.iter().map(Into::into).collect())),
        ],
    }
}

#[cfg(not(target_os = "windows"))]
fn own_files(item: &ClipboardItem) -> bool {
    item.entries().iter().any(|entry| {
        matches!(entry, ClipboardEntry::String(text) if matches!(text.metadata.as_deref(), Some(COPIED | CUT)))
    })
}

#[cfg(not(target_os = "windows"))]
fn external_paths(item: &ClipboardItem) -> Option<Vec<String>> {
    item.entries().iter().find_map(|entry| match entry {
        ClipboardEntry::ExternalPaths(paths) if !paths.paths().is_empty() => {
            Some(paths.paths().iter().map(|path| path.to_string_lossy().into_owned()).collect())
        }
        _ => None,
    })
}
