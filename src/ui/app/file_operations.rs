use super::{Annuler, OxydeApp, PendingFileOperation, Refaire, ToastKind};
use crate::ui::i18n::{I18nContext, Message};
use gpui::*;

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
                    runtime.block_on(oxyde_lib::commands::ops::undo_last_action(&history))
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
                    } else if matches!(result, Ok(Ok(Some(_)))) {
                        this.toast(ToastKind::Success, Message::ToastUndo, String::new(), 0, cx);
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
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let runtime = tokio::runtime::Handle::current();
                let result = tokio::task::spawn_blocking(move || {
                    runtime.block_on(oxyde_lib::commands::ops::redo_last_action(&history))
                })
                .await;
                handle.update(&mut cx, |this, cx| {
                    this.history_operation_in_progress = false;
                    if let Ok(Err(error)) = &result {
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
        self.prepare_file_operation(paths, target_dir, true, cx);
    }

    pub fn execute_file_copy(
        &mut self,
        paths: Vec<String>,
        target_dir: String,
        cx: &mut Context<Self>,
    ) {
        self.prepare_file_operation(paths, target_dir, false, cx);
    }

    fn prepare_file_operation(
        &mut self,
        paths: Vec<String>,
        target_dir: String,
        is_move: bool,
        cx: &mut Context<Self>,
    ) {
        if paths.is_empty() {
            return;
        }
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
                            });
                        cx.notify();
                    }
                    Ok(Ok((paths, _))) => {
                        this.queue_file_operation(paths, target_dir, is_move, None, runtime, cx)
                    }
                    Ok(Err(error)) => {
                        this.file_operation_notice =
                            Some((Message::PrepareFailed, format!("{error:?}")));
                        cx.notify();
                    }
                    Err(error) => {
                        this.file_operation_notice =
                            Some((Message::PrepareFailed, format!("{error}")));
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
        self.watch_file_operation(op_id, cx);
    }

    pub(super) fn watch_file_operation(&mut self, op_id: String, cx: &mut Context<Self>) {
        use oxyde_lib::systems::file_ops::{FileOpType, OpStatus};
        let operations = self.file_ops.clone();
        let Some(handle) = self.handle.clone() else {
            return;
        };
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(200))
                        .await;
                    let Some((op_type, status)) = operations.operation_state(&op_id) else {
                        break;
                    };
                    match status {
                        OpStatus::Queued
                        | OpStatus::Calculating
                        | OpStatus::Running
                        | OpStatus::Paused => {
                            handle.update(&mut cx, |_, cx| cx.notify());
                        }
                        OpStatus::Error(error) => {
                            handle.update(&mut cx, |this, cx| {
                                log::warn!("File operation failed: {error}");
                                let recovery = this.archive_recovery_notice(&error, cx);
                                this.file_operation_notice = Some((if recovery { Message::ArchiveRecoveryNotice } else { Message::TransferFailed }, error));
                                cx.notify();
                            });
                            break;
                        }
                        OpStatus::Completed => {
                            handle.update(&mut cx, |this, cx| {
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
                            handle.update(&mut cx, |_, cx| cx.notify());
                            break;
                        }
                    }
                }
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
            cx,
        );
        cx.notify();
    }
}
