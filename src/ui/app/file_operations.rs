use super::{Annuler, OxydeApp, PendingFileOperation, Refaire, ToastKind};
use crate::ui::i18n::{I18nContext, Message};
use gpui::*;

/// Notices waiting behind the one shown.
const MAX_QUEUED_NOTICES: usize = 8;

/// What a failed deletion left behind, tied to the notice it was reported with.
#[derive(Clone)]
pub(crate) struct DeleteRetry {
    pub(crate) detail: String,
    pub(crate) paths: Vec<String>,
    pub(crate) permanent: bool,
}

fn actionable_transfer_paths(paths: Vec<String>, resolutions: &std::collections::HashMap<String, oxyde_lib::models::ConflictAction>) -> Vec<String> {
    paths.into_iter().filter(|path| resolutions.get(path) != Some(&oxyde_lib::models::ConflictAction::Skip)).collect()
}

#[cfg(test)]
#[path = "file_operations/tests/skip.rs"]
mod skip_tests;

impl OxydeApp {
    pub fn execute_undo(&mut self, cx: &mut Context<Self>) {
        if self.history_operation_in_progress {
            return;
        }
        // The disabled button tells why; the shortcut says it the same way.
        if let Some(block) = self.history.summary().undo_blocked {
            let message = match block {
                oxyde_lib::models::transaction::UndoBlock::Overwrite => Message::UndoOverwriteUnavailable,
                oxyde_lib::models::transaction::UndoBlock::Archive => Message::UndoArchiveUnavailable,
            };
            self.toast(ToastKind::Info, message, String::new(), 0, cx);
            cx.notify();
            return;
        }
        let history = self.history.clone();
        let Some(handle) = self.handle.clone() else {
            return;
        };
        self.history_operation_in_progress = true;
        cx.notify();
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let runtime = tokio::runtime::Handle::current();
                let result = tokio::task::spawn_blocking(move || {
                    runtime.block_on(oxyde_lib::commands::ops::undo_last_action_report(&history))
                })
                .await;
                handle.update(&mut cx, |this, cx| {
                    this.history_operation_in_progress = false;
                    if let Ok(Err(error)) = &result {
                        log::warn!("Undo failed: {error:?}");
                        let (message, detail) = crate::ui::errors::undo_failure(error);
                        this.toast(ToastKind::Error, message, detail, 0, cx);
                    } else if let Err(error) = &result {
                        this.toast(ToastKind::Error, Message::UndoFailed, format!("{error}"), 0, cx);
                    } else if let Ok(Ok(report)) = &result {
                        if !report.retained.is_empty() {
                            let locations = report.retained.iter().map(|path| path.to_string_lossy()).collect::<Vec<_>>().join("
");
                            this.toast(ToastKind::Error, Message::UndoStagedOutputRetained, locations, 0, cx);
                        } else if !report.left_in_trash.is_empty() {
                            let locations = report.left_in_trash.iter().map(|path| path.to_string_lossy()).collect::<Vec<_>>().join("\n");
                            this.toast(ToastKind::Error, Message::UndoLeftInTrash, locations, 0, cx);
                        } else if report.transaction.is_some() {
                            this.toast(ToastKind::Success, Message::ToastUndo, String::new(), 0, cx);
                        }
                    }
                    let refresh_files = matches!(&result, Ok(Ok(report)) if report.transaction.is_some())
                        || matches!(&result, Ok(Err(oxyde_lib::models::CommandError::TrashError(_))) | Err(_));
                    if refresh_files { this.refresh_all_tabs(cx); }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn execute_redo(&mut self, cx: &mut Context<Self>) {
        if self.history_operation_in_progress {
            return;
        }
        let history = self.history.clone();
        let Some(handle) = self.handle.clone() else {
            return;
        };
        self.history_operation_in_progress = true;
        cx.notify();
        let operations = self.file_ops.clone();
        let turbo = self.config.default_turbo_mode;
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let runtime = tokio::runtime::Handle::current();
                // A copy made again shows in the operations panel, where it can be stopped.
                let (queued, shown) = futures::channel::oneshot::channel();
                let panel = oxyde_lib::systems::file_ops::RedoCopyPanel {
                    manager: operations.clone(),
                    runtime: runtime.clone(),
                    history: history.clone(),
                    turbo,
                    queued: std::sync::Mutex::new(Some(queued)),
                };
                let redo = tokio::task::spawn_blocking(move || {
                    runtime.block_on(oxyde_lib::commands::ops::redo_last_action_shown(&history, &panel))
                });
                let copy = shown.await.ok();
                if let Some(id) = &copy {
                    handle.update(&mut cx, |this, cx| this.watch_operation(id.clone(), true, cx));
                }
                let result = redo.await;
                // Stopped from the panel, which shows it: the entry stays to redo.
                let stopped = copy.as_ref().is_some_and(|id| {
                    operations.operation_progress(id).is_some_and(|progress| progress.status == oxyde_lib::systems::file_ops::OpStatus::Cancelled)
                });
                handle.update(&mut cx, |this, cx| {
                    this.history_operation_in_progress = false;
                    if stopped {
                        log::info!("Redo copy cancelled");
                    } else if let Ok(Err(error)) = &result {
                        log::warn!("Redo failed: {error:?}");
                        let (message, detail) = crate::ui::errors::redo_failure(error);
                        this.toast(ToastKind::Error, message, detail, 0, cx);
                    } else if let Err(error) = &result {
                        this.toast(ToastKind::Error, Message::RedoFailed, format!("{error}"), 0, cx);
                    } else if matches!(result, Ok(Ok(Some(_)))) {
                        this.toast(ToastKind::Success, Message::ToastRedo, String::new(), 0, cx);
                    }
                    let refresh_files = matches!(&result, Ok(Ok(Some(_))))
                        || matches!(&result, Ok(Err(oxyde_lib::models::CommandError::TrashError(_))) | Err(_));
                    if refresh_files { this.refresh_all_tabs(cx); }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn on_action_annuler(&mut self, _: &Annuler, _: &mut Window, cx: &mut Context<Self>) {
        self.execute_undo(cx);
    }

    pub fn on_action_refaire(&mut self, _: &Refaire, _: &mut Window, cx: &mut Context<Self>) {
        self.execute_redo(cx);
    }

    pub fn execute_file_move(
        &mut self,
        paths: Vec<String>,
        target_dir: String,
        cx: &mut Context<Self>,
    ) {
        self.prepare_file_operation(paths, target_dir, true, None, cx);
    }

    pub fn execute_file_copy(
        &mut self,
        paths: Vec<String>,
        target_dir: String,
        cx: &mut Context<Self>,
    ) {
        self.prepare_file_operation(paths, target_dir, false, None, cx);
    }

    /// Writes streamed files to a staging folder off the UI thread, then copies
    /// them like any other files, with the usual conflicts, progress and undo.
    pub(crate) fn import_streamed_files(
        &mut self,
        files: oxyde_lib::platform::StreamedFiles,
        target_dir: String,
        cx: &mut Context<Self>,
    ) {
        let Some(handle) = self.handle.clone() else {
            return;
        };
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let extracted = tokio::task::spawn_blocking(move || files.extract()).await;
                handle.update(&mut cx, |this, cx| match extracted {
                    Ok(Ok((paths, staging))) => {
                        this.prepare_file_operation(paths, target_dir, false, Some(std::rc::Rc::new(staging)), cx)
                    }
                    Ok(Err(error)) => {
                        log::warn!("Streamed drop failed: {error}");
                        this.report_notice((Message::PrepareFailed, error), None);
                        cx.notify();
                    }
                    Err(error) => {
                        this.report_notice((Message::PrepareFailed, format!("{error}")), None);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    fn prepare_file_operation(
        &mut self,
        paths: Vec<String>,
        target_dir: String,
        is_move: bool,
        staging: Option<std::rc::Rc<oxyde_lib::platform::DropStaging>>,
        cx: &mut Context<Self>,
    ) {
        // Menus and keys already refuse these; drops reach here directly.
        if paths.is_empty() || self.is_read_only_path(&target_dir) {
            return;
        }
        // Nothing leaves a read-only location: a move from there is a copy.
        let is_move = is_move && !paths.iter().any(|path| {
            std::path::Path::new(path).parent().is_some_and(|parent| self.is_read_only_path(&parent.to_string_lossy()))
        });
        let Some(handle) = self.handle.clone() else {
            return;
        };
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let mut check_paths = paths;
                let check_destination = target_dir.clone();
                let runtime = tokio::runtime::Handle::current();
                let runtime_for_check = runtime.clone();
                let checked = tokio::task::spawn_blocking(move || {
                    if is_move {
                        check_paths.retain(|source| !oxyde_lib::commands::ops::is_self_transfer(
                            std::path::Path::new(source), std::path::Path::new(&check_destination)));
                    }
                    let response = runtime_for_check.block_on(oxyde_lib::commands::ops::check_conflicts(
                        check_paths.clone(),
                        check_destination,
                    ))?;
                    Ok::<_, oxyde_lib::models::CommandError>((check_paths, response))
                })
                .await;
                handle.update(&mut cx, |this, cx| match checked {
                    Ok(Ok((paths, _))) if paths.is_empty() => {}
                    Ok(Ok((paths, response))) if !response.is_empty() => {
                        this.pending_file_operations
                            .push_back(PendingFileOperation {
                                paths,
                                target_dir,
                                is_move,
                                conflicts: response,
                                dialog: Default::default(),
                                runtime,
                                staging,
                            });
                        cx.notify();
                    }
                    Ok(Ok((paths, _))) => {
                        this.queue_file_operation(paths, target_dir, is_move, None, runtime, staging, cx)
                    }
                    Ok(Err(error)) => {
                        this.report_notice((Message::PrepareFailed, format!("{error}")), None);
                        cx.notify();
                    }
                    Err(error) => {
                        this.report_notice((Message::PrepareFailed, format!("{error}")), None);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    fn queue_file_operation(
        &mut self,
        paths: Vec<String>,
        target_dir: String,
        is_move: bool,
        resolutions: Option<std::collections::HashMap<String, oxyde_lib::models::ConflictAction>>,
        runtime: tokio::runtime::Handle,
        staging: Option<std::rc::Rc<oxyde_lib::platform::DropStaging>>,
        cx: &mut Context<Self>,
    ) {
        use oxyde_lib::systems::file_ops::{FileOpType, FileOperation};
        let mut op = FileOperation::new(
            if is_move {
                FileOpType::Move
            } else {
                FileOpType::Copy
            },
            paths.into_iter().map(std::path::PathBuf::from).collect(),
            Some(std::path::PathBuf::from(target_dir)),
            resolutions,
        );
        op.compression_quality = self.archive_quality_for_path(op.destination.as_ref().unwrap());
        op.archive_qualities = self.archive_qualities();
        op.copy_name_suffix = cx.i18n().text(Message::CopyNameSuffix).to_string();
        op.set_turbo(self.config.default_turbo_mode);
        let op_id = self
            .file_ops
            .queue_operation_on(&runtime, self.history.clone(), op);
        if let Some(staging) = staging {
            self.drop_stagings.insert(op_id.clone(), staging);
        }
        self.watch_file_operation(op_id, cx);
    }

    pub(super) fn watch_file_operation(&mut self, op_id: String, cx: &mut Context<Self>) {
        self.watch_operation(op_id, false, cx);
    }

    /// `quiet`: whoever queued the operation reports its end, as a Redo does its copy.
    fn watch_operation(&mut self, op_id: String, quiet: bool, cx: &mut Context<Self>) {
        use oxyde_lib::systems::file_ops::{FileOpType, OpStatus};
        let operations = self.file_ops.clone();
        let Some(handle) = self.handle.clone() else {
            return;
        };
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let mut previous = None;
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(200))
                        .await;
                    let Some(progress) = operations.operation_progress(&op_id) else {
                        handle.update(&mut cx, |app, cx| app.resume_viewers_after_deletion(&op_id, true, cx));
                        break;
                    };
                    let changed = previous.as_ref() != Some(&progress);
                    if changed {
                        cx.update(|cx| crate::ui::components::operations::sync_taskbar_progress(&operations, cx));
                    }
                    let op_type = progress.op_type.clone();
                    let status = progress.status.clone();
                    previous = Some(progress);
                    if quiet && matches!(status, OpStatus::Completed | OpStatus::Cancelled | OpStatus::Error(_)) {
                        handle.update(&mut cx, |_, cx| cx.notify());
                        break;
                    }
                    match status {
                        OpStatus::Queued
                        | OpStatus::Calculating
                        | OpStatus::Running
                        | OpStatus::Paused => {
                            handle.update(&mut cx, |app, cx| {
                                if changed { cx.notify(); }
                                else if let Some(panel) = &app.operations_panel_view {
                                    // Keep detailed progress/ETA polling in its own window;
                                    // an unchanged badge must not redraw directory panes.
                                    panel.update(cx, |_, cx| cx.notify());
                                }
                            });
                        }
                        OpStatus::Error(error) => {
                            // What a deletion left on disk is read off the UI thread:
                            // a share that stopped answering is what makes it fail.
                            let outcomes = operations.operation_item_outcomes(&op_id);
                            let retry = {
                                let (op_type, error) = (op_type.clone(), error.clone());
                                tokio::task::spawn_blocking(move || delete_retry(outcomes?, &op_type, &error))
                                    .await
                                    .ok()
                                    .flatten()
                            };
                            handle.update(&mut cx, |this, cx| {
                                this.resume_viewers_after_deletion(&op_id, true, cx);
                                log::warn!("File operation failed: {error}");
                                if this.retry_operation_with_password(&op_id, &error, cx) {
                                    return;
                                }
                                // The alert below reports the kept files; a toast would repeat it.
                                let recovery = this.refresh_after_archive_recovery(&error, cx).is_some();
                                this.report_notice((if recovery { Message::ArchiveRecoveryNotice } else { Message::TransferFailed }, error), retry);
                                cx.notify();
                            });
                            break;
                        }
                        OpStatus::Completed => {
                            handle.update(&mut cx, |this, cx| {
                                this.resume_viewers_after_deletion(&op_id, false, cx);
                                this.refresh_all_tabs(cx);
                                this.state.refresh_sidebar();
                                this.reconcile_watchers(cx);
                                let message = match op_type {
                                    FileOpType::Copy => Message::ToastCopyCompleted,
                                    FileOpType::Move => Message::ToastMoveCompleted,
                                    FileOpType::Trash => Message::ToastTrashCompleted,
                                    FileOpType::Delete => Message::ToastDeleteCompleted,
                                    FileOpType::Compress => Message::ToastCompressCompleted,
                                    FileOpType::Extract => Message::ToastExtractCompleted,
                                };
                                this.toast(ToastKind::Success, message, String::new(), 0, cx);
                                cx.notify();
                            });
                            break;
                        }
                        OpStatus::Cancelled => {
                            handle.update(&mut cx, |app, cx| {
                                app.resume_viewers_after_deletion(&op_id, true, cx);
                                cx.notify();
                            });
                            break;
                        }
                    }
                }
                // Copied, failed or cancelled: the staged streamed files go.
                handle.update(&mut cx, |this, _| this.drop_stagings.remove(&op_id));
                cx.update(|cx| crate::ui::components::operations::sync_taskbar_progress(&operations, cx));
            }
        })
        .detach();
    }

    pub(crate) fn cancel_pending_file_operation(&mut self, cx: &mut Context<Self>) {
        self.pending_file_operations.pop_front();
        cx.notify();
    }

    pub(crate) fn apply_conflict_decisions(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = self.pending_file_operations.pop_front() else {
            return;
        };
        let resolutions = pending.dialog.completed_resolutions(&pending.conflicts);
        let paths = actionable_transfer_paths(pending.paths, &resolutions);
        if paths.is_empty() {
            self.toast(ToastKind::Info, Message::ToastTransferSkipped, String::new(), 0, cx);
            cx.notify();
            return;
        }
        self.queue_file_operation(
            paths,
            pending.target_dir,
            pending.is_move,
            Some(resolutions),
            pending.runtime,
            pending.staging,
            cx,
        );
        cx.notify();
    }
}

/// Selected items a Trash/Delete operation did not finish, still on disk. Blocking.
fn delete_retry(
    outcomes: std::collections::HashMap<std::path::PathBuf, oxyde_lib::systems::file_ops::ItemOutcome>,
    op_type: &oxyde_lib::systems::file_ops::FileOpType,
    error: &str,
) -> Option<DeleteRetry> {
    use oxyde_lib::systems::file_ops::{FileOpType, ItemOutcome};
    // Closing programs would not help: the mount stays until unmounted.
    if oxyde_lib::models::error::MountNotEntered::from_message(error).is_some() {
        return None;
    }
    let permanent = match op_type {
        FileOpType::Trash => false,
        FileOpType::Delete => true,
        _ => return None,
    };
    let mut paths: Vec<String> = outcomes
        .into_iter()
        // A broken link is still there to delete.
        .filter(|(path, outcome)| *outcome != ItemOutcome::Completed && std::fs::symlink_metadata(path).is_ok())
        .map(|(path, _)| path.to_string_lossy().into_owned())
        .collect();
    paths.sort();
    (!paths.is_empty()).then(|| DeleteRetry { detail: error.to_string(), paths, permanent })
}

impl OxydeApp {
    /// Shows a failure's notice, or queues it behind the one shown so that neither it
    /// nor the shown one's retry is lost.
    pub(crate) fn report_notice(&mut self, notice: (Message, String), retry: Option<DeleteRetry>) {
        if self.file_operation_notice.is_none() {
            self.file_operation_notice = Some(notice);
            self.delete_retry = retry;
        } else if self.queued_notices.len() < MAX_QUEUED_NOTICES {
            self.queued_notices.push_back((notice, retry));
        } else {
            log::warn!("Operation notice dropped, {MAX_QUEUED_NOTICES} already wait: {}", notice.1);
        }
    }

    pub(crate) fn retry_failed_delete(&mut self, cx: &mut Context<Self>) {
        let retry = self.delete_retry.take();
        self.dismiss_notice();
        if let Some(retry) = retry {
            self.queue_trash_delete(retry.paths, retry.permanent, cx);
        }
        cx.notify();
    }
}
