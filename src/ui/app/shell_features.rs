use super::*;
use oxyde_lib::utils::change_signal::ChangeSignal;
use std::sync::Arc;
use std::time::{Duration, Instant};

impl OxydeApp {
    pub fn on_action_ouvrir(&mut self, _: &Ouvrir, _: &mut Window, cx: &mut Context<Self>) {
        self.execute_ouvrir(cx);
    }

    /// Shell handlers may pump Windows messages. Never invoke them while GPUI
    /// holds the App/Entity borrow for a mouse or keyboard callback.
    pub(crate) fn open_external_item(&mut self, path: String, cx: &mut Context<Self>) {
        self.pause_viewer_media_for_external_open(&path, cx);
        let source = path.clone();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let path = oxyde_lib::commands::archive::prepare_archive_file_for_open(std::path::Path::new(&path))?;
                oxyde_lib::platform::platform().open_item(&path.to_string_lossy())
                    .map_err(oxyde_lib::models::CommandError::SystemError)
            }).await;
            let error = match result {
                Ok(Ok(())) => return,
                Ok(Err(oxyde_lib::models::CommandError::ArchiveCache(issue))) => {
                    let _ = this.update(cx, |this, cx| this.archive_cache_failed(issue, source, cx));
                    return;
                }
                Ok(Err(error)) => error.to_string(),
                Err(error) => error.to_string(),
            };
            let _ = this.update(cx, |this, cx| {
                let archive = super::archive_password::archive_of(&source);
                let retry = source.clone();
                if this.ask_archive_password(&archive, &error, move |this, cx| this.open_external_item(retry, cx), cx) {
                    return;
                }
                this.toast(ToastKind::Error, Message::OpenItemFailed, error, 0, cx);
                cx.notify();
            });
        }).detach();
    }

    pub(crate) fn open_terminal(&mut self, path: String, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || oxyde_lib::platform::platform().open_terminal(&path)).await;
            let error = match result {
                Ok(Ok(())) => return,
                Ok(Err(error)) => error,
                Err(error) => error.to_string(),
            };
            let _ = this.update(cx, |this, cx| {
                this.toast(ToastKind::Error, Message::OpenItemFailed, error, 0, cx);
                cx.notify();
            });
        }).detach();
    }

    /// The folder selected alone, or else the folder on display.
    pub(super) fn open_terminal_for_selection(&mut self, cx: &mut Context<Self>) {
        let Some(pane) = self.state.find_pane(&self.state.active_pane_id) else { return };
        let tab = pane.active_tab();
        let selected = (tab.selected_paths.len() == 1 && tab.path != "trash://").then(|| tab.files.iter()
            .find(|file| file.is_dir && tab.selected_paths.contains(&file.path))).flatten();
        let path = selected.map_or(&tab.path, |file| &file.path).clone();
        if oxyde_lib::platform::platform().offers_terminal() && crate::ui::components::is_terminal_folder(&path) {
            self.open_terminal(path, cx);
        }
    }

    pub(crate) fn open_file_item(
        &mut self,
        pane_id: &str,
        path: String,
        is_dir: bool,
        cx: &mut Context<Self>,
    ) {
        let entry = self.file_for_action(&path, is_dir);
        let navigable_archive = entry.is_navigable_archive();
        if entry.needs_mount_for_browsing() {
            self.open_disk_image(pane_id.to_string(), path, false, cx);
        } else if is_dir || navigable_archive {
            self.open_directory_item(pane_id, path, false, cx);
        } else if entry.archive_path.is_none()
            && oxyde_lib::utils::archive_volumes::old_naming_first_volume(&oxyde_lib::utils::path_text::decode(&path)).is_some()
        {
            self.open_old_naming_volume(pane_id, path, cx);
        } else {
            self.open_external_item(path, cx);
        }
    }

    /// `film.r03` opens its set from `film.rar`, as `film.part3.rar` does. Without that
    /// first volume, or past a missing one, the file goes to its application.
    fn open_old_naming_volume(&mut self, pane_id: &str, path: String, cx: &mut Context<Self>) {
        let pane = pane_id.to_string();
        let origin = self.state.find_pane(pane_id).map(|pane| (pane.active_tab().id.clone(), pane.active_tab().path.clone()));
        cx.spawn(async move |this, cx| {
            let volume = oxyde_lib::utils::path_text::decode(&path);
            let first = tokio::task::spawn_blocking(move || {
                oxyde_lib::utils::archive_volumes::first_volume_of_old_naming(&volume)
            })
            .await
            .ok()
            .flatten();
            let _ = this.update(cx, |this, cx| match first {
                Some(first) => {
                    // The tab moved on meanwhile: the set does not replace what it shows now.
                    if this.state.find_pane(&pane).map(|pane| (pane.active_tab().id.clone(), pane.active_tab().path.clone())) == origin {
                        let first = oxyde_lib::utils::path_text::encode(&first);
                        this.open_directory_item(&pane, first, false, cx);
                    }
                }
                None => this.open_external_item(path, cx),
            });
        })
        .detach();
    }

    pub fn execute_ouvrir(&mut self, cx: &mut Context<Self>) {
        let pane_id = self.state.active_pane_id.clone();
        if let Some(pane) = self.state.layout.find_pane(&pane_id) {
            let tab = pane.active_tab();
            let paths: Vec<(String, bool)> = tab.files.iter()
                .filter(|entry| tab.selected_paths.contains(&entry.path))
                .map(|entry| (entry.path.clone(), entry.is_dir)).collect();
            if paths.len() > super::open_many::CONFIRM_ABOVE {
                self.ask_open_many(pane_id, paths, cx);
                return;
            }
            for (path, is_dir) in paths {
                self.open_file_item(&pane_id, path, is_dir, cx);
            }
        }
    }

    pub(crate) fn refresh_quick_access(&mut self, cx: &mut Context<Self>) {
        self.quick_access_request = self.quick_access_request.wrapping_add(1);
        let request = self.quick_access_request;
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(|| {
                let platform = oxyde_lib::platform::platform();
                (
                    oxyde_lib::commands::system::get_quick_access_items(),
                    platform.cloud_sync_roots(),
                    platform.wsl_distributions(),
                )
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                if this.quick_access_request != request {
                    return;
                }
                let result = result.map(|(items, roots, distributions)| {
                    let wsl_root = oxyde_lib::platform::WSL_ROOT;
                    let known = this.state.sidebar_nodes.get(wsl_root).map(Vec::as_slice).unwrap_or_default();
                    if !known.iter().map(|node| &node.path).eq(distributions.iter().map(|node| &node.path)) {
                        this.state.sidebar_nodes.insert(wsl_root.to_string(), distributions);
                        this.state.refresh_sidebar();
                        cx.notify();
                    }
                    let has_cloud_roots = !roots.is_empty();
                    if this.has_cloud_roots != has_cloud_roots {
                        this.has_cloud_roots = has_cloud_roots;
                        cx.notify();
                    }
                    let roots = if this.config.show_cloud_roots { roots } else { Vec::new() };
                    if !this.state.cloud_root_items.iter().map(|i| (&i.path, &i.name)).eq(roots.iter().map(|i| (&i.path, &i.name))) {
                        this.state.cloud_root_items = roots;
                        this.state.refresh_sidebar();
                        cx.notify();
                    }
                    items
                });
                match result {
                    Ok(Ok(items)) => {
                        let items = super::pinned_order::apply_order(items, &this.config.pinned_order);
                        if !this
                            .state
                            .pinned_items
                            .iter()
                            .map(|i| (&i.path, &i.name))
                            .eq(items.iter().map(|i| (&i.path, &i.name)))
                        {
                            this.state.pinned_items = items;
                            // A folder unpinned during a press or a reorder ends it, or the layer would stay.
                            if let Some((path, _)) = &this.pinned_press {
                                if !this.state.pinned_items.iter().any(|item| item.path == *path) {
                                    this.pinned_press = None;
                                }
                            }
                            if this.pinned_reorder.as_ref().is_some_and(|reorder| !this.state.pinned_items.iter().any(|item| item.path == reorder.path)) {
                                this.pinned_reorder = None;
                            }
                            this.state.refresh_sidebar();
                            this.refresh_system_menus(cx);
                            cx.notify();
                        }
                    }
                    Ok(Err(error)) => log::warn!("Cannot refresh quick access: {error}"),
                    Err(error) => log::warn!("Quick access worker: {error}"),
                }
            });
        })
        .detach();
    }

    pub(super) fn start_shell_features_task(&mut self, cx: &mut Context<Self>) {
        self.shell_features_task = Some(cx.spawn(async move |this, cx| {
            let revision = Arc::new(ChangeSignal::new());
            let callback_revision = revision.clone();
            let registration = tokio::task::spawn_blocking(move || {
                oxyde_lib::platform::platform().quick_access().watch(callback_revision)
            })
            .await;
            let _guard = match registration {
                Ok(Ok(guard)) => guard,
                other => {
                    log::warn!(
                        "Quick access notifications unavailable: {}",
                        match other {
                            Ok(Err(e)) => e,
                            Err(e) => e.to_string(),
                            _ => unreachable!(),
                        }
                    );
                    None
                }
            };
            let mut observed = 0;
            let mut pending = None;
            // Due at once; the clock may not reach 10 s back in the first seconds after boot.
            let mut refresh = Instant::now().checked_sub(Duration::from_secs(10)).unwrap_or_else(Instant::now);
            loop {
                let now = Instant::now();
                let current = revision.value();
                if current != observed {
                    observed = current;
                    pending = Some(now);
                }
                let due = pending.is_some_and(|changed: Instant| {
                    now.duration_since(changed) >= Duration::from_millis(350)
                });
                if due || now.duration_since(refresh) >= Duration::from_secs(5) {
                    pending = None;
                    refresh = now;
                    if this
                        .update(cx, |this, cx| this.refresh_quick_access(cx))
                        .is_err()
                    {
                        break;
                    }
                    let status = tokio::task::spawn_blocking(|| {
                        oxyde_lib::platform::platform().peek_status()
                    })
                    .await;
                    if let Ok(Ok(status)) = status {
                        if this
                            .update(cx, |this, cx| {
                                if this.peek_status != status {
                                    this.peek_status = status;
                                    cx.notify();
                                }
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                }
                // Woken by a change, or when the debounce or the periodic refresh is due.
                let now = Instant::now();
                let mut wait = Duration::from_secs(5).saturating_sub(now.duration_since(refresh));
                if let Some(changed) = pending {
                    wait = wait.min(Duration::from_millis(350).saturating_sub(now.duration_since(changed)));
                }
                let timer = cx.background_executor().timer(wait);
                futures::future::select(std::pin::pin!(timer), std::pin::pin!(revision.changed())).await;
            }
        }));
    }

    pub(crate) fn peek_file(&mut self, path: String, cx: &mut Context<Self>) {
        if self.network_dialog.is_some()
            || self.remote_login.is_some()
            || self.editing_view.is_some()
            || self.settings_confirmation.is_some()
            || self.network_force_confirmation.is_some()
            || self.pending_trash_delete.is_some()
            || self.file_operation_notice.is_some()
            || self.quit_confirmation.is_some()
            || self.open_many.is_some()
        {
            return;
        }
        cx.spawn(async move |this, cx| {
            let result = oxyde_lib::commands::system::open_peek(path).await;
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    this.toast(ToastKind::Error, Message::PeekFailed, error, 0, cx)
                });
            }
        })
        .detach();
    }

    pub(super) fn peek_selected(&mut self, cx: &mut Context<Self>) {
        let path = self
            .state
            .find_pane(&self.state.active_pane_id)
            .and_then(|pane| {
                let tab = pane.active_tab();
                if tab.selected_paths.len() != 1 || tab.path == "trash://" {
                    return None;
                }
                tab.files
                    .iter()
                    .find(|file| tab.selected_paths.contains(&file.path) && !file.is_dir)
                    .map(|file| file.path.clone())
            });
        if let Some(path) = path {
            self.peek_file(path, cx);
        }
    }

    pub(super) fn peek_shortcut_matches(&self, key: &Keystroke) -> bool {
        if self.settings_open || !self.context_menus.is_empty() {
            return false;
        }
        let status = &self.peek_status;
        if !status.installed || !status.enabled {
            return false;
        }
        if status.space_enabled && key.key == "space" && !key.modifiers.modified() {
            return true;
        }
        status
            .activation_shortcut
            .as_deref()
            .is_some_and(|shortcut| shortcut_matches(shortcut, key))
    }
}

fn shortcut_matches(shortcut: &str, key: &Keystroke) -> bool {
    let parts: Vec<_> = shortcut.split('+').collect();
    let has = |name: &str| parts.iter().any(|part| part.eq_ignore_ascii_case(name));
    key.modifiers.control == has("Ctrl")
        && key.modifiers.alt == has("Alt")
        && key.modifiers.shift == has("Shift")
        && key.modifiers.platform == has("Win")
        && !key.modifiers.function
        && parts
            .last()
            .is_some_and(|part| part.eq_ignore_ascii_case(&key.key))
}

#[cfg(test)]
mod tests {
    use super::shortcut_matches;
    use gpui::Keystroke;
    #[test]
    fn peek_shortcuts_require_exact_modifiers() {
        assert!(shortcut_matches(
            "Ctrl+Space",
            &Keystroke::parse("ctrl-space").unwrap()
        ));
        assert!(!shortcut_matches(
            "Ctrl+Space",
            &Keystroke::parse("ctrl-shift-space").unwrap()
        ));
        assert!(shortcut_matches(
            "Ctrl+Shift+P",
            &Keystroke::parse("ctrl-shift-p").unwrap()
        ));
    }
}
