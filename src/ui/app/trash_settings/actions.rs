use super::*;
use oxyde_lib::models::FileEntry;
use super::confirmation::SHOWN_NAMES;
use std::collections::HashSet;

/// Longest wait for the volumes to tell whether a deletion would be permanent.
const POLICY_CHECK_DELAY: std::time::Duration = std::time::Duration::from_secs(5);

/// Paths and shown names of the trash items an action applies to.
fn trash_targets(files: &[FileEntry], selected: &HashSet<String>, all: bool) -> (Vec<String>, Vec<String>) {
    files
        .iter()
        .filter(|file| all || selected.contains(&file.path))
        .map(|file| (file.path.clone(), oxyde_lib::utils::path_text::display(&file.name)))
        .unzip()
}

/// Emptying the whole trash always asks, as the system does; purging a selection
/// follows the preference, like a permanent delete elsewhere.
fn purge_needs_confirmation(all: bool, confirm_delete: bool) -> bool {
    all || confirm_delete
}

fn display_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|name| oxyde_lib::utils::path_text::display(&name.to_string_lossy()))
        .unwrap_or_else(|| path.to_string())
}

impl OxydeApp {
    fn prompt_delete(
        &mut self,
        target: DeleteTarget,
        paths: Vec<String>,
        names: Vec<String>,
        count: u64,
        permanent_prompt: bool,
        cx: &mut Context<Self>,
    ) {
        self.pending_trash_delete = Some(PendingDelete {
            target, paths, names, count, permanent_prompt,
            focus: cx.focus_handle(), confirm_focus: cx.focus_handle(),
        });
        cx.notify();
    }

    /// Ask before emptying every volume's trash from its Properties.
    pub(super) fn prompt_empty_trash(&mut self, count: u64, cx: &mut Context<Self>) {
        if self.pending_trash_delete.is_none() {
            self.prompt_delete(DeleteTarget::WholeTrash, Vec::new(), Vec::new(), count, true, cx);
        }
    }

    pub(crate) fn execute_trash_action(&mut self, restore: bool, all: bool, cx: &mut Context<Self>) {
        if self.trash_busy || self.pending_trash_delete.is_some() { return; }
        let Some(pane) = self.state.find_pane(&self.state.active_pane_id) else { return; };
        let tab = pane.active_tab();
        if tab.path != "trash://" { return; }
        // « All » names the whole trash; a selection is only what the filters show.
        let listed = if all { tab.files.as_slice() } else { tab.filtered_files() };
        let (paths, names) = trash_targets(listed, &tab.selected_paths, all);
        if paths.is_empty() { return; }
        if !restore && purge_needs_confirmation(all, self.config.confirm_delete) {
            let count = paths.len() as u64;
            self.prompt_delete(DeleteTarget::TrashItems { all }, paths, names, count, true, cx);
            return;
        }
        self.run_trash_action(restore, paths, cx);
    }

    /// Restore or purge trash items; a purge has been confirmed when the preference asks.
    pub(super) fn run_trash_action(&mut self, restore: bool, paths: Vec<String>, cx: &mut Context<Self>) {
        if self.trash_busy || paths.is_empty() { return; }
        let count = paths.len();
        self.trash_busy = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let runtime = tokio::runtime::Handle::current();
            #[cfg_attr(target_os = "windows", allow(unused_mut))]
            let mut result = tokio::task::spawn_blocking(move || runtime.block_on(async move {
                if restore { oxyde_lib::commands::ops::restore_items_report(paths).await.map(Some) }
                else { oxyde_lib::commands::ops::purge_recycle_bin(paths).await.map(|_| None) }
            })).await;
            #[cfg(not(target_os = "windows"))]
            let on_other_volumes = match &mut result {
                Ok(Ok(Some(report))) => std::mem::take(&mut report.on_other_volumes),
                _ => Vec::new(),
            };
            #[cfg(target_os = "windows")]
            let on_other_volumes: Vec<(String, String)> = Vec::new();
            let _ = this.update(cx, |this, cx| {
                this.trash_busy = false;
                let count = count - on_other_volumes.len();
                let error = match result {
                    Ok(Ok(Some(report))) if !report.failures.is_empty() => {
                        let mut args = crate::ui::i18n::FluentArgs::new();
                        args.set("restored", report.restored.len().to_string());
                        args.set("failed", report.failures.len().to_string());
                        let detail = format!("{}\n{}", cx.i18n().text_with_args(Message::TrashRestoreSummary, &args), report.failures[0].0);
                        Some(detail)
                    }
                    Ok(Ok(_)) => None,
                    Ok(Err(error)) => Some(error.to_string()),
                    Err(error) => Some(error.to_string()),
                };
                if let Some(error) = error { this.toast(ToastKind::Error, Message::ToastFailed, error, 0, cx); }
                else if count > 0 { this.toast(ToastKind::Success, if restore { Message::ToastRestored } else { Message::ToastPurged }, String::new(), count, cx); }
                this.refresh_all_tabs(cx);
                // A rename cannot bring these back: a move does, with its progress and conflicts.
                let mut by_folder = std::collections::BTreeMap::<String, Vec<String>>::new();
                for (content, folder) in on_other_volumes {
                    by_folder.entry(folder).or_default().push(content);
                }
                for (folder, contents) in by_folder {
                    this.execute_file_move(contents, folder, cx);
                }
                cx.notify();
            });
        }).detach();
    }

    pub fn on_action_supprimer(&mut self, _: &Supprimer, window: &mut Window, cx: &mut Context<Self>) {
        self.shift_delete = window.modifiers().shift;
        self.execute_supprimer(cx);
    }

    pub fn execute_supprimer(&mut self, cx: &mut Context<Self>) {
        if self.trash_delete_check_pending || self.pending_trash_delete.is_some() {
            return;
        }
        if self.state.find_pane(&self.state.active_pane_id).is_some_and(|pane| pane.active_tab().path == "trash://") {
            self.execute_trash_action(false, false, cx);
            return;
        }
        let pane_id = self.state.active_pane_id.clone();
        if let Some(pane) = self.state.layout.find_pane(&pane_id) {
            let paths = pane.active_tab().actionable_selection();
            if paths.is_empty() {
                return;
            }
            let shift = self.shift_delete;
            let confirm = self.config.confirm_delete;
            self.trash_delete_check_pending = true;
            cx.spawn(async move |this, cx| {
                let snapshot = paths.clone();
                // Where nothing can be recycled, ask for the permanent deletion at
                // once rather than failing to recycle and asking again.
                // A volume that does not answer must not hold every later deletion: past the
                // delay the Recycle Bin is tried, and the Shell warns if it deletes for good.
                let permanent = shift || {
                    use futures::FutureExt;
                    let check = tokio::task::spawn_blocking(move || {
                        super::deletion_policy::selection_deletes_permanently(&snapshot)
                    });
                    let delay = cx.background_executor().timer(POLICY_CHECK_DELAY);
                    futures::select_biased! {
                        permanent = check.fuse() => permanent.unwrap_or(false),
                        _ = delay.fuse() => {
                            log::warn!("Recycle Bin policy check timed out; trying the Recycle Bin");
                            false
                        }
                    }
                };
                let _ = this.update(cx, |this, cx| {
                    this.trash_delete_check_pending = false;
                    if this.pending_trash_delete.is_some() {
                        return;
                    }
                    // Without confirmation, only Shift may destroy unasked.
                    if !confirm && (shift || !permanent) {
                        this.queue_trash_delete(paths, shift, cx);
                        return;
                    }
                    let names = paths.iter().take(SHOWN_NAMES).map(|path| display_name(path)).collect();
                    let count = paths.len() as u64;
                    this.prompt_delete(DeleteTarget::Files { permanent }, paths, names, count, permanent, cx);
                });
            }).detach();
        }
    }

    pub(crate) fn queue_trash_delete(&mut self, paths: Vec<String>, permanent: bool, cx: &mut Context<Self>) {
            let Some(handle) = self.handle.clone() else {
                return;
            };
            let mut op = oxyde_lib::systems::file_ops::FileOperation::new(
                if permanent { oxyde_lib::systems::file_ops::FileOpType::Delete } else { oxyde_lib::systems::file_ops::FileOpType::Trash },
                paths.into_iter().map(std::path::PathBuf::from).collect(),
                None,
                None,
            );
            let released = self.suspend_viewers_for_deletion(&op.id, &op.sources, cx);
            self.trash_delete_check_pending = true;
            cx.spawn(move |_, cx: &mut AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    let runtime = tokio::runtime::Handle::current();
                    released.await;
                    handle.update(&mut cx, |this, cx| {
                        this.trash_delete_check_pending = false;
                        op.archive_qualities = this.archive_qualities();
                        op.set_turbo(this.config.default_turbo_mode);
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

#[cfg(test)]
#[path = "actions/tests/purge.rs"]
mod tests;
