//! OK on the Permissions tab: the changes go out of the UI thread, item after item.
//! A single item reports as before; a selection names its first failure and keeps
//! what the other items received.
use crate::ui::app::properties::PropertiesWindow;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use gpui::*;
use oxyde_lib::models::permissions::{PermissionChange, PermissionError};

/// Text of a failure, named after its item in a selection.
fn detail(path: &str, error: &PermissionError, cx: &App) -> String {
    let text = match error {
        PermissionError::Changed => cx.i18n().text(Message::PermissionsChanged).to_string(),
        PermissionError::NotKept => cx.i18n().text(Message::PermissionsNotKept).to_string(),
        PermissionError::Failed(detail) | PermissionError::Incomplete { first: detail, .. } => detail.clone(),
    };
    let name = std::path::Path::new(path).file_name().map(|name| oxyde_lib::utils::path_text::display(&name.to_string_lossy()));
    match (error, name) {
        (PermissionError::Incomplete { .. }, _) | (_, None) => text,
        (_, Some(name)) => format!("{name}: {text}"),
    }
}

impl PropertiesWindow {
    /// Reads again the permissions of `paths`, off the UI thread, as the items' loaded state.
    fn reload_permissions(&mut self, paths: Vec<String>, revision: u64, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let fresh = cx
                .background_spawn(async move {
                    paths
                        .into_iter()
                        .filter_map(|path| {
                            let loaded = oxyde_lib::commands::io::get_file_properties(path.clone()).ok()?.permissions?;
                            Some((path, loaded))
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let Some(edit) = this
                    .properties_dialog
                    .as_mut()
                    .filter(|dialog| dialog.revision == revision)
                    .and_then(|dialog| dialog.permissions.as_mut())
                else {
                    return;
                };
                for (path, loaded) in fresh {
                    if let Some(item) = edit.items.iter_mut().find(|item| item.path == path) {
                        item.loaded = loaded;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(in crate::ui::app::properties) fn save_permissions(
        &mut self,
        changes: Vec<(String, PermissionChange)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.properties_dialog.as_mut() else {
            return;
        };
        let revision = dialog.revision;
        let selection = dialog.paths.len() > 1;
        dialog.saving = true;
        dialog.error = None;
        dialog.validation_error = None;
        if let Some(edit) = &mut dialog.permissions {
            edit.menu = None;
        }
        window.focus(&dialog.ok_focus, cx);
        cx.notify();
        let window_handle = window.window_handle();
        let cancel = self.cancellation.clone();
        cx.spawn(async move |this, cx| {
            let results = oxyde_lib::commands::io::set_selection_permissions(changes.clone(), cancel).await;
            let _ = window_handle.update(cx, |_, window, cx| {
                this.update(cx, |this, cx| {
                    let Some(dialog) = this.properties_dialog.as_mut().filter(|dialog| dialog.revision == revision) else {
                        return;
                    };
                    dialog.saving = false;
                    if results.iter().all(|(_, result)| result.is_ok()) {
                        this.close_file_properties(window, cx);
                        if let Some(owner) = this.owner.upgrade() {
                            owner.update(cx, |owner, cx| owner.refresh_all(cx));
                        }
                        return;
                    }
                    // What was applied is now what the items have.
                    let applied = |result: &Result<(), PermissionError>| {
                        matches!(result, Ok(()) | Err(PermissionError::Incomplete { .. }))
                    };
                    if let Some(edit) = &mut dialog.permissions {
                        for ((path, result), (_, change)) in results.iter().zip(&changes) {
                            let Some(item) = edit.items.iter_mut().find(|item| &item.path == path).filter(|_| applied(result)) else {
                                continue;
                            };
                            item.loaded.mode = change.mode;
                            item.loaded.owner.id = change.owner;
                            item.loaded.group.id = change.group;
                        }
                        if results.iter().all(|(_, result)| applied(result)) {
                            edit.enclosed_apply = false;
                        }
                    }
                    let failed: u64 = results
                        .iter()
                        .map(|(_, result)| match result {
                            Ok(()) => 0,
                            Err(PermissionError::Incomplete { failed, .. }) => *failed,
                            Err(_) => 1,
                        })
                        .sum();
                    let (path, error) = results
                        .iter()
                        .find_map(|(path, result)| result.as_ref().err().map(|error| (path, error)))
                        .expect("a failure");
                    match error {
                        _ if selection => {
                            let mut args = FluentArgs::new();
                            args.set("count", crate::ui::format::format_number(failed, cx.i18n().language()));
                            args.set("detail", detail(path, error, cx));
                            dialog.error =
                                Some(cx.i18n().text_with_args(Message::PermissionsSelectionIncomplete, &args).to_string());
                        }
                        PermissionError::Changed => dialog.validation_error = Some(Message::PermissionsChanged),
                        PermissionError::NotKept => dialog.validation_error = Some(Message::PermissionsNotKept),
                        PermissionError::Failed(detail) => dialog.error = Some(detail.clone()),
                        PermissionError::Incomplete { failed, first } => {
                            let mut args = FluentArgs::new();
                            args.set("count", crate::ui::format::format_number(*failed, cx.i18n().language()));
                            args.set("detail", first.clone());
                            dialog.error = Some(cx.i18n().text_with_args(Message::PermissionsIncomplete, &args).to_string());
                        }
                    }
                    // Changed since the window opened: what they have now is read again, so
                    // that OK applies the choices over it instead of failing each time.
                    let changed: Vec<String> = results
                        .iter()
                        .filter(|(_, result)| matches!(result, Err(PermissionError::Changed)))
                        .map(|(path, _)| path.clone())
                        .collect();
                    if !changed.is_empty() {
                        this.reload_permissions(changed, revision, cx);
                    }
                    if results.iter().any(|(_, result)| applied(result)) {
                        if let Some(owner) = this.owner.upgrade() {
                            owner.update(cx, |owner, cx| owner.refresh_all(cx));
                        }
                    }
                    cx.notify();
                })
            });
        })
        .detach();
    }
}
