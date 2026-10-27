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
        let text = tab.filtered_files().iter()
            .filter(|file| tab.selected_paths.contains(&file.path))
            .map(|file| if names_only { file.name.clone() } else { file.path.clone() })
            .collect::<Vec<_>>().join("\n");
        if !text.is_empty() {
            let count = text.lines().count();
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.toast(ToastKind::Info, if names_only { Message::ToastNamesCopied } else { Message::ToastPathsCopied }, String::new(), count, cx);
        }
    }

    pub fn execute_copier(&mut self, cx: &mut Context<Self>) { self.copy_files_to_clipboard(false, cx); }
    pub fn on_action_couper(&mut self, _: &Couper, _: &mut Window, cx: &mut Context<Self>) { self.execute_couper(cx); }
    pub fn execute_couper(&mut self, cx: &mut Context<Self>) { self.copy_files_to_clipboard(true, cx); }
    fn copy_files_to_clipboard(&mut self, cut: bool, cx: &mut Context<Self>) {
        let Some(pane) = self.state.find_pane(&self.state.active_pane_id) else { return; };
        let paths: Vec<_> = pane.active_tab().selected_paths.iter().cloned().collect();
        if paths.is_empty() { return; }
        let text = paths.join("\n");
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || oxyde_lib::platform::platform().file_clipboard().set_files(paths, cut)).await;
            let _ = this.update(cx, |this, cx| {
                // Applications the platform cannot offer files to receive their paths.
                if matches!(result, Ok(Ok(true))) {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
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
        if target_dir.starts_with("search://") { return; }
        #[cfg(target_os = "windows")]
        let text = None;
        #[cfg(not(target_os = "windows"))]
        let text = {
            let system = cx.read_from_clipboard();
            // Files another application offers through GPUI come first, as in Zed; Linux
            // GPUI offers none yet.
            if let Some(paths) = system.as_ref().and_then(external_paths) {
                self.execute_file_copy(paths, target_dir, cx);
                return;
            }
            system.and_then(|item| item.text())
        };
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(|| oxyde_lib::platform::platform().file_clipboard().get_files(text)).await;
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

#[cfg(not(target_os = "windows"))]
fn external_paths(item: &ClipboardItem) -> Option<Vec<String>> {
    item.entries().iter().find_map(|entry| match entry {
        ClipboardEntry::ExternalPaths(paths) if !paths.paths().is_empty() => {
            Some(paths.paths().iter().map(|path| path.to_string_lossy().into_owned()).collect())
        }
        _ => None,
    })
}
