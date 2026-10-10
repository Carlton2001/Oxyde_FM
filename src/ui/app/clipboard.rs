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
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || oxyde_lib::platform::platform().set_clipboard_files(paths, cut)).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(())) => this.toast(ToastKind::Info, if cut { Message::ToastCut } else { Message::ToastCopied }, String::new(), 0, cx),
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
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(|| oxyde_lib::platform::platform().get_clipboard_files()).await;
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
