use super::*;

impl OxydeApp {
    pub(crate) fn execute_trash_action(&mut self, restore: bool, all: bool, cx: &mut Context<Self>) {
        if self.trash_busy { return; }
        let Some(pane) = self.state.find_pane(&self.state.active_pane_id) else { return; };
        let tab = pane.active_tab();
        if tab.path != "trash://" { return; }
        let paths: Vec<_> = tab.files.iter()
            .filter(|file| all || tab.selected_paths.contains(&file.path))
            .map(|file| file.path.clone()).collect();
        if paths.is_empty() { return; }
        let count = paths.len();
        self.trash_busy = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let runtime = tokio::runtime::Handle::current();
            let result = tokio::task::spawn_blocking(move || runtime.block_on(async move {
                if restore { oxyde_lib::commands::ops::restore_items(paths).await.map(|_| ()) }
                else { oxyde_lib::commands::ops::purge_recycle_bin(paths).await }
            })).await;
            let _ = this.update(cx, |this, cx| {
                this.trash_busy = false;
                let error = match result {
                    Ok(Ok(())) => None,
                    Ok(Err(error)) => Some(error.to_string()),
                    Err(error) => Some(error.to_string()),
                };
                if let Some(error) = error { this.toast(ToastKind::Error, Message::ToastFailed, error, 0, cx); }
                else { this.toast(ToastKind::Success, if restore { Message::ToastRestored } else { Message::ToastPurged }, String::new(), count, cx); }
                this.refresh_all_tabs(cx);
                cx.notify();
            });
        }).detach();
    }

    pub fn on_action_supprimer(&mut self, _: &Supprimer, window: &mut Window, cx: &mut Context<Self>) {
        self.shift_delete = window.modifiers().shift;
        self.execute_supprimer(cx);
    }

    pub fn execute_supprimer(&mut self, cx: &mut Context<Self>) {
        if self.state.find_pane(&self.state.active_pane_id).is_some_and(|pane| pane.active_tab().path == "trash://") {
            self.execute_trash_action(false, false, cx);
            return;
        }
        let pane_id = self.state.active_pane_id.clone();
        if let Some(pane) = self.state.layout.find_pane(&pane_id) {
            let paths: Vec<String> = pane.active_tab().selected_paths.iter().cloned().collect();
            if paths.is_empty() {
                return;
            }
            if self.config.confirm_delete {
                self.pending_trash_delete = Some(trash_settings::PendingDelete { paths, permanent: self.shift_delete, focus: cx.focus_handle(), confirm_focus: cx.focus_handle() }); cx.notify(); return;
            }
            self.queue_trash_delete(paths, self.shift_delete, cx);

        }
    }

    pub(crate) fn queue_trash_delete(&mut self, paths: Vec<String>, permanent: bool, cx: &mut Context<Self>) {
            let Some(handle) = self.handle.clone() else {
                return;
            };
            cx.spawn(move |_, cx: &mut AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    let runtime = tokio::runtime::Handle::current();
                    handle.update(&mut cx, |this, cx| {
                        let mut op = oxyde_lib::systems::file_ops::FileOperation::new(
                            if permanent { oxyde_lib::systems::file_ops::FileOpType::Delete } else { oxyde_lib::systems::file_ops::FileOpType::Trash },
                            paths.into_iter().map(std::path::PathBuf::from).collect(),
                            None,
                            None,
                        );
                        op.archive_qualities = this.archive_qualities();
                        let id =
                            this.file_ops
                                .queue_operation_on(&runtime, this.history.clone(), op);
                        this.watch_file_operation(id, cx);
                    });
                }
            })
            .detach();
    }

}
