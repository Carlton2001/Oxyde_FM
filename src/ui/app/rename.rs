use super::{OxydeApp, Renommer, ToastKind};
use crate::ui::components::input::{InputAppearance, TextInput, TextInputOptions};
use crate::ui::i18n::{FluentArgs, I18n, I18nContext, Message};
use gpui::*;

impl OxydeApp {
    pub(crate) fn rename_context_item(
        &mut self,
        path: String,
        is_dir: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.begin_rename(path, is_dir, false, window, cx);
    }

    pub fn on_action_renommer(
        &mut self,
        _: &Renommer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_renommer(window, cx);
    }

    pub fn execute_renommer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pane_id = self.state.active_pane_id.clone();
        let selected = self.state.layout.find_pane(&pane_id).and_then(|pane| {
            let tab = pane.active_tab();
            tab.selected_paths.iter().next().cloned().map(|path| {
                let is_dir = tab
                    .files
                    .iter()
                    .any(|file| file.path == path && file.is_dir);
                (path, is_dir)
            })
        });
        if let Some((path, is_dir)) = selected {
            self.begin_rename(path, is_dir, false, window, cx);
        }
    }

    pub(crate) fn handle_slow_rename_click(
        &mut self,
        path: String,
        is_dir: bool,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ClickEvent::Mouse(event) = event else {
            return;
        };
        if self.suppress_item_selection_click
            || event.down.click_count != 1
            || event.down.modifiers != Modifiers::default()
            || event.up.modifiers != Modifiers::default()
        {
            self.rename_click_candidate = None;
            return;
        }
        let now = std::time::Instant::now();
        let previous = self.rename_click_candidate.take();
        let is_same_selected_item = self
            .state
            .layout
            .find_pane(&self.state.active_pane_id)
            .map(|pane| {
                let selected = &pane.active_tab().selected_paths;
                selected.len() == 1 && selected.contains(&path)
            })
            .unwrap_or(false);
        let is_slow_second_click = previous
            .as_ref()
            .map(|(previous_path, previous_at)| {
                previous_path == &path
                    && now.duration_since(*previous_at) >= std::time::Duration::from_millis(250)
                    && now.duration_since(*previous_at) <= std::time::Duration::from_secs(2)
            })
            .unwrap_or(false);

        if self.inline_rename || self.editing_path.is_some() {
            return;
        }

        if is_same_selected_item && is_slow_second_click {
            self.begin_rename(path, is_dir, true, window, cx);
        } else if is_same_selected_item {
            self.rename_click_candidate = Some((path, now));
        }
    }

    pub(super) fn begin_rename(
        &mut self,
        path: String,
        is_dir: bool,
        inline: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.begin_rename_with_extension(path, is_dir, inline, None, window, cx);
    }

    pub(super) fn begin_rename_with_extension(
        &mut self,
        path: String,
        is_dir: bool,
        inline: bool,
        extension: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Rename and breadcrumb editing share the same input entity.
        if self.breadcrumb_editing {
            self.cancel_breadcrumb_edit(cx);
        }
        let initial_name = std::path::Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();
        let selected_len = initial_selection_len(&initial_name, is_dir, extension);
        let initial_selection = Some(0..selected_len);

        let Some(handle) = self.handle.clone() else {
            return;
        };

        self.archive_creation = None;
        self.rename_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.rename_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.rename_generation = self.rename_generation.wrapping_add(1);
        self.rename_pending = false;
        self.rename_click_candidate = None;
        self.editing_path = Some(path);
        self.editing_text = initial_name.clone();
        self.rename_error = None;
        self.inline_rename = inline;

        let view = cx.new(|cx| {
            TextInput::new(
                cx,
                cx.i18n().text(Message::RenameNamePlaceholder),
                initial_name,
                {
                    let handle = handle.clone();
                    move |text, _window, cx| {
                        handle.update(cx, |this, cx| {
                            this.editing_text = text.to_string();
                            this.rename_error = None;
                            cx.notify();
                        });
                    }
                },
                {
                    let handle = handle.clone();
                    move |_window, cx| {
                        handle.update(cx, |this, cx| this.submit_rename(cx));
                    }
                },
                {
                    let handle = handle.clone();
                    move |_window, cx| {
                        handle.update(cx, |this, cx| this.cancel_rename(cx));
                    }
                },
                {
                    let handle = handle.clone();
                    move |input, has_selection, position, window, cx| {
                        handle.update(cx, |this, cx| {
                            this.open_text_context_menu(input, has_selection, position, window, cx)
                        });
                    }
                },
                TextInputOptions {
                    appearance: if inline {
                        InputAppearance::Inline
                    } else {
                        InputAppearance::Standard
                    },
                    submit_on_outside: inline,
                    initial_selection: initial_selection,
                    ..Default::default()
                },
                window,
            )
        });

        let focus_handle = view.read(cx).focus_handle.clone();
        self.editing_view = Some(view);
        window.focus(&focus_handle, cx);
        cx.notify();
    }

    pub(crate) fn submit_rename(&mut self, cx: &mut Context<Self>) {
        if self.inline_rename && !self.context_menus.is_empty() {
            return;
        }
        if self.rename_pending {
            return;
        }
        let Some(path) = self.editing_path.clone() else {
            return;
        };
        if let Some((sources, extension)) = self.archive_creation.clone() {
            let name = self.editing_text.trim();
            if name.is_empty() {
                self.rename_error = Some(Message::RenameEmptyName);
                cx.notify();
                return;
            }
            if name == "."
                || name == ".."
                || name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
                || name.chars().any(char::is_control)
                || name.ends_with(['.', ' '])
            {
                self.rename_error = Some(Message::RenameInvalidName);
                cx.notify();
                return;
            }
            let suffix = format!(".{extension}");
            let name = if name.to_lowercase().ends_with(&suffix) {
                name.to_string()
            } else {
                format!("{name}{suffix}")
            };
            let destination = std::path::Path::new(&path)
                .parent()
                .unwrap_or(std::path::Path::new(""))
                .join(name);
            self.cancel_rename(cx);
            self.queue_archive_operation(
                oxyde_lib::systems::file_ops::FileOpType::Compress,
                sources,
                destination,
                cx,
            );
            return;
        }
        let current_name = std::path::Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if self.editing_text == current_name {
            self.cancel_rename(cx);
            return;
        }
        if self.editing_text.trim().is_empty() {
            self.rename_error = Some(Message::RenameEmptyName);
            self.rename_shake_id = self.rename_shake_id.wrapping_add(1);
            cx.notify();
            return;
        }
        self.rename_pending = true;
        self.execute_file_rename(path, self.editing_text.clone(), cx);
    }

    pub(crate) fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.rename_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.archive_creation = None;
        self.rename_generation = self.rename_generation.wrapping_add(1);
        self.editing_path = None;
        self.editing_view = None;
        self.inline_rename = false;
        self.rename_pending = false;
        self.rename_error = None;
        cx.notify();
    }

    pub fn execute_file_rename(
        &mut self,
        old_path: String,
        new_name: String,
        cx: &mut Context<Self>,
    ) {
        let parent = std::path::Path::new(&old_path)
            .parent()
            .unwrap_or(std::path::Path::new(""));
        let new_path = parent.join(new_name).to_string_lossy().to_string();

        let Some(handle) = self.handle.clone() else {
            self.rename_pending = false;
            return;
        };
        let quality = self.archive_quality_for_path(std::path::Path::new(&old_path));
        let cancel = self.rename_cancel.clone();
        let rename_history = self.history.clone();
        let generation = self.rename_generation;
        let path_for_check = old_path.clone();
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let runtime = tokio::runtime::Handle::current();
                let result = tokio::task::spawn_blocking(move || {
                    let result = runtime.block_on(oxyde_lib::commands::io::rename_item_controlled_with_identity(
                        old_path.clone(), new_path.clone(), quality, cancel,
                    ));
                    if let Ok(Some(identity)) = &result {
                        oxyde_lib::commands::io::record_captured_folder_rename(&rename_history, &old_path, &new_path, *identity);
                    }
                    result.map(|_| ())
                })
                .await;
                handle.update(&mut cx, |this, cx| {
                    let current_attempt = this.rename_generation == generation
                        && this.editing_path.as_deref() == Some(path_for_check.as_str());
                    let recovery = match &result {
                        Ok(Err(error)) => this.archive_recovery_notice(&error.to_string(), cx),
                        _ => false,
                    };
                    if let Ok(Ok(())) = &result {
                        this.reconcile_watchers(cx);
                    }
                    if !current_attempt {
                        return;
                    }
                    this.rename_pending = false;
                    match result {
                        Ok(Ok(())) => {
                            this.editing_path = None;
                            this.editing_view = None;
                            this.inline_rename = false;
                            this.rename_error = None;
                            this.refresh_all_tabs(cx);
                            cx.notify();
                        }
                        Ok(Err(error)) => {
                            log::warn!("Rename failed: {error:?}");
                            if recovery {
                                this.editing_path = None;
                                this.editing_view = None;
                                this.inline_rename = false;
                                this.rename_error = None;
                            } else {
                                this.rename_error = Some(crate::ui::errors::rename_message(&error));
                                this.rename_shake_id = this.rename_shake_id.wrapping_add(1);
                            }
                            cx.notify();
                        }
                        Err(error) => {
                            log::warn!("Rename task failed: {error}");
                            this.rename_error = Some(Message::RenameTaskFailed);
                            this.rename_shake_id = this.rename_shake_id.wrapping_add(1);
                            cx.notify();
                        }
                    }
                });
            }
        })
        .detach();
    }

    pub fn create_new_folder(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        use oxyde_lib::models::transaction::{Transaction, TransactionDetails, TransactionType};

        if let Some(pane) = self.state.layout.find_pane(pane_id) {
            let current_path = pane.active_tab().path.clone();
            if current_path.starts_with("search://") { return; }
            let language = cx.i18n().language();
            let quality = self.archive_quality_for_path(std::path::Path::new(&current_path));
            let tab_id = pane.active_tab().id.clone();
            let Some(handle) = self.handle.clone() else {
                return;
            };
            cx.spawn(move |_, cx: &mut AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    let created = tokio::task::spawn_blocking({
                        let current_path = current_path.clone();
                        move || {
                            let i18n = I18n::for_language(language);
                            for counter in 0.. {
                                let name = if counter == 0 {
                                    i18n.text(Message::NewFolderName).to_string()
                                } else {
                                    let mut args = FluentArgs::new();
                                    args.set("number", counter);
                                    i18n.text_with_args(Message::NewFolderNumbered, &args)
                                        .replace(['\u{2068}', '\u{2069}'], "")
                                };
                                let path = std::path::Path::new(&current_path).join(name);
                                let created = if oxyde_lib::utils::archive::split_virtual_path(&current_path).is_some() {
                                    oxyde_lib::commands::archive::create_archive_directory_with_quality(&path, &quality).map(|()| None)
                                } else {
                                    oxyde_lib::commands::io::create_local_folder(&path)
                                };
                                match created {
                                    Ok(identity) => return Ok((path, identity)),
                                    Err(error)
                                        if error.kind() == std::io::ErrorKind::AlreadyExists =>
                                    {
                                        continue;
                                    }
                                    Err(error) => return Err(error),
                                }
                            }
                            unreachable!()
                        }
                    })
                    .await;
                    handle.update(&mut cx, |this, cx| match created {
                        Ok(Ok((folder_path, identity))) => {
                            this.queue_created_item_rename(
                                tab_id,
                                folder_path.to_string_lossy().into_owned(),
                                true,
                                cx,
                            );
                            if oxyde_lib::utils::archive::split_virtual_path(&current_path).is_none() {
                                this.history.push(Transaction::new(
                                    TransactionType::NewFolder,
                                    TransactionDetails {
                                        deleted_items: None,
                                        renamed_item_identity: None,
                                        moved_directory_identities: None,
                                        new_folder_identity: identity,
                                        moved_file_identities: None,
                                        copied_directory_identities: None,
                                        copied_file_identities: None,
                                        paths: vec![folder_path.to_string_lossy().into_owned()],
                                        target_dir: Some(current_path),
                                        old_path: None,
                                        new_path: None,
                                        created_files: None,
                                        created_dirs: None,
                                        overwritten_files: None,
                                        moved_files: None,
                                        trash_metadata_backups: None,
                                        moved_dirs: None,
                                        copied_files: None,
                                        copied_file_hashes: None,
                                    },
                                ));
                            }
                            this.refresh_all_tabs(cx);
                            this.state.refresh_sidebar();
                            this.reconcile_watchers(cx);
                            cx.notify();
                        }
                        Ok(Err(error)) => {
                            let detail = error.to_string();
                            if !this.archive_recovery_notice(&detail, cx) {
                                this.toast(ToastKind::Error, Message::ToastFailed, detail, 0, cx);
                            }
                        }
                        Err(error) => this.toast(ToastKind::Error, Message::ToastFailed, error.to_string(), 0, cx),
                    });
                }
            })
            .detach();
        }
    }

    pub(super) fn queue_created_item_rename(
        &mut self,
        tab_id: String,
        path: String,
        is_dir: bool,
        cx: &mut Context<Self>,
    ) {
        let parent = std::path::Path::new(&path)
            .parent()
            .map(|path| path.to_string_lossy().into_owned());
        let mut target = None;
        self.state.layout.visit_panes(&mut |pane| {
            if let Some((index, tab)) = pane
                .tabs
                .iter()
                .enumerate()
                .find(|(_, tab)| tab.id == tab_id)
            {
                if Some(&tab.path) == parent.as_ref() && pane.active_tab().id == tab_id {
                    target = Some((pane.id.clone(), index, tab.path.clone()));
                }
            }
        });
        let Some((pane_id, index, directory)) = target else {
            return;
        };
        self.state.active_pane_id = pane_id.clone();
        self.created_item_rename = Some((tab_id, path, is_dir));
        self.request_tab_files(&pane_id, index, directory, false, cx);
    }

    pub(super) fn prepare_created_item_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((tab_id, path, is_dir)) = self.created_item_rename.clone() {
            let pane_id = self.state.active_pane_id.clone();
            let width = self
                .pane_sizes
                .get(&pane_id)
                .map(|size| size.width)
                .unwrap_or(self.list_width);
            if let Some(pane) = self.state.find_pane_mut(&pane_id) {
                let tab = pane.active_tab_mut();
                if tab.id != tab_id
                    || std::path::Path::new(&path).parent() != Some(std::path::Path::new(&tab.path))
                {
                    self.created_item_rename = None;
                } else if tab.files.iter().any(|file| file.path == path) {
                    tab.filter_name.clear();
                    tab.filter_original_location.clear();
                    tab.filter_types.clear();
                    tab.filter_sizes.clear();
                    tab.filter_dates.clear();
                    tab.update_cache();
                    tab.selected_paths.clear();
                    tab.selected_paths.insert(path.clone());
                    tab.selection_anchor = Some(path.clone());
                    if let Some(index) = tab
                        .filtered_files()
                        .iter()
                        .position(|file| file.path == path)
                    {
                        let grid = matches!(tab.view_mode, crate::ui::workspace::ViewMode::Grid);
                        let columns = if grid { crate::ui::style::GridGeometry::for_viewport(width, crate::ui::components::scrollbar::zed_track_width()).columns } else { 1 };
                        let file = &tab.filtered_files()[index];
                        tab.collapsed_date_groups.remove(&crate::ui::date_groups::DateGroup::for_timestamp(file.modified, chrono::Local::now().date_naive()));
                        let rows = crate::ui::date_groups::display_rows(tab.filtered_files(), tab.group_by_date, &tab.collapsed_date_groups, columns);
                        if let Some(row) = rows.iter().position(|row| matches!(row, crate::ui::date_groups::DisplayRow::Files(indices) if indices.contains(&index))) {
                            if grid {
                                let y = crate::ui::date_groups::content_height(&rows[..row], true);
                                tab.list_scroll.0.borrow().base_handle.set_offset(point(px(0.0), px(-y)));
                            } else {
                                tab.list_scroll.scroll_to_item(row, gpui::ScrollStrategy::Top);
                            }
                        }
                    }
                    self.created_item_rename = None;
                    self.begin_rename(path, is_dir, true, window, cx);
                }
            } else {
                self.created_item_rename = None;
            }
        }
    }
}

fn initial_selection_len(name: &str, is_dir: bool, extension: Option<&str>) -> usize {
    if is_dir {
        return name.len();
    }
    if let Some(extension) = extension {
        if let Some(stem) = name.strip_suffix(&format!(".{extension}")) {
            return stem.len();
        }
    }
    std::path::Path::new(name).file_stem().and_then(|stem| stem.to_str())
        .map(str::len).unwrap_or(name.len())
}

#[cfg(test)]
#[path = "rename/tests/selection.rs"]
mod selection_tests;
