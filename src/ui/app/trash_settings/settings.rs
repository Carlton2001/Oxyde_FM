use crate::ui::components::input::{InputAppearance, TextInputOptions};
use super::*;

impl OxydeApp {
    pub(crate) fn open_trash_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.trash_settings.is_some() {
            cx.defer(super::super::dialog_windows::activate_trash_window);
            return;
        }
        self.context_menus.clear();
        let h = self.handle.as_ref().unwrap().clone();
        let change = h.clone();
        let enter = h.clone();
        let escape = h.clone();
        let context = h;
        let input = cx.new(|cx| {
            TextInput::new(
                cx,
                "",
                String::new(),
                move |value, _, cx| {
                    change.update(cx, |this, cx| {
                        if let Some(dialog) = &mut this.trash_settings {
                            if !dialog.busy && !dialog.loading {
                                dialog.value = value.into();
                                dialog.value_dirty = true;
                            }
                        }
                        cx.notify();
                    });
                },
                move |_, cx| {
                    enter.update(cx, |this, cx| this.save_trash_settings(cx));
                },
                move |window, cx| {
                    escape.update(cx, |this, cx| this.close_trash_settings(window, cx));
                },
                move |input, selection, position, window, cx| {
                    context.update(cx, |this, cx| {
                        this.open_text_context_menu(input, selection, position, window, cx)
                    });
                },
                TextInputOptions {
                    appearance: InputAppearance::Bare,
                    ..Default::default()
                },
                window,
            )
        });
        let focus = cx.focus_handle();
        self.trash_settings = Some(TrashSettings {
            drives: vec![],
            configs: vec![],
            initial: vec![],
            selected: 0,
            percent: false,
            input,
            value: String::new(),
            value_dirty: false,
            confirm: self.config.confirm_delete,
            usage: (0, 0),
            loading: true,
            busy: false,
            error: None,
            focus,
            save_focus: cx.focus_handle(),
        });
        cx.notify();
        let request = self.trash_settings.as_ref().unwrap().input.entity_id();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(|| {
                let runtime = tokio::runtime::Handle::current();
                let drives: Vec<_> = oxyde_lib::platform::platform()
                    .volumes()
                    .list(true)
                    .into_iter()
                    // Drives with a Recycle Bin: Windows has none on USB sticks or SUBST drives.
                    .filter(|drive| matches!(drive.drive_type.as_str(), "fixed" | "removable")
                        && oxyde_lib::platform::platform().has_recycle_bin(std::path::Path::new(&drive.path)))
                    .collect();
                // What can be read shows; the first failure is reported beside it.
                let mut failure = None;
                let usage = runtime
                    .block_on(oxyde_lib::commands::ops::get_total_recycle_bin_usage())
                    .unwrap_or_else(|error| {
                        failure.get_or_insert(error.to_string());
                        (0, 0)
                    });
                let mut readable = Vec::new();
                let mut configs = Vec::new();
                for drive in drives {
                    match oxyde_lib::platform::platform().recycle_bin_config(drive.path.clone()) {
                        Ok(config) => {
                            readable.push(drive);
                            configs.push(config);
                        }
                        Err(error) => {
                            log::warn!("Cannot read the Recycle Bin settings of {}: {error}", drive.path);
                            failure.get_or_insert(error);
                        }
                    }
                }
                Ok::<_, String>((readable, configs, usage, failure))
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                if this
                    .trash_settings
                    .as_ref()
                    .is_none_or(|dialog| dialog.input.entity_id() != request)
                {
                    return;
                }
                if let Some(dialog) = &mut this.trash_settings {
                    dialog.loading = false;
                    match result {
                        Ok(Ok((drives, configs, usage, failure))) => {
                            dialog.drives = drives;
                            dialog.initial = configs.clone();
                            dialog.configs = configs;
                            dialog.usage = usage;
                            dialog.error = failure;
                        }
                        Ok(Err(error)) => dialog.error = Some(error),
                        Err(error) => dialog.error = Some(error.to_string()),
                    }
                }
                this.select_trash_drive(0, false, cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn close_trash_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .trash_settings
            .as_ref()
            .is_some_and(|dialog| dialog.busy)
        {
            return;
        }
        self.trash_settings = None;
        window.remove_window();
        cx.notify();
    }

    fn commit_trash_capacity(&mut self) -> bool {
        let Some(dialog) = &mut self.trash_settings else {
            return false;
        };
        let Some(drive) = dialog.drives.get(dialog.selected) else {
            return false;
        };
        let Some(config) = dialog.configs.get_mut(dialog.selected) else {
            return false;
        };
        if config.nuke_on_delete || !dialog.value_dirty {
            return true;
        }
        let Some(value) = capacity_mb(&dialog.value, dialog.percent, drive.total_bytes) else {
            dialog.error = Some(String::new());
            return false;
        };
        config.max_size_mb = value;
        dialog.value_dirty = false;
        dialog.error = None;
        true
    }

    pub(crate) fn select_trash_drive(
        &mut self,
        index: usize,
        percent: bool,
        cx: &mut Context<Self>,
    ) {
        if self
            .trash_settings
            .as_ref()
            .is_some_and(|dialog| dialog.busy || dialog.loading)
        {
            return;
        }
        // An edited value, emptied included, is checked before another drive shows.
        if self
            .trash_settings
            .as_ref()
            .is_some_and(|dialog| dialog.value_dirty)
            && !self.commit_trash_capacity()
        {
            cx.notify();
            return;
        }
        let Some(dialog) = &mut self.trash_settings else {
            return;
        };
        let Some(config) = dialog.configs.get(index) else {
            return;
        };
        dialog.selected = index;
        dialog.percent = percent;
        dialog.value = if percent {
            capacity_percent(config.max_size_mb, dialog.drives[index].total_bytes).to_string()
        } else {
            config.max_size_mb.to_string()
        };
        dialog.value_dirty = false;
        let value = dialog.value.clone();
        dialog
            .input
            .update(cx, |input, cx| input.set_content(value, cx));
        cx.notify();
    }

    pub(crate) fn save_trash_settings(&mut self, cx: &mut Context<Self>) {
        if self.preferences_load_failed {
            if let Some(dialog) = &mut self.trash_settings {
                dialog.error = Some(String::new());
            }
            cx.notify();
            return;
        }
        if self
            .trash_settings
            .as_ref()
            .is_none_or(|dialog| dialog.loading || dialog.busy)
            || !self.commit_trash_capacity()
        {
            cx.notify();
            return;
        }
        let dialog = self.trash_settings.as_mut().unwrap();
        let changed: Vec<_> = dialog
            .configs
            .iter()
            .zip(&dialog.initial)
            .filter(|(a, b)| a.max_size_mb != b.max_size_mb || a.nuke_on_delete != b.nuke_on_delete)
            .map(|(a, _)| a.clone())
            .collect();
        let confirm = dialog.confirm;
        dialog.busy = true;
        self.trash_delete_policy.invalidate();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let mut saved = Vec::new();
                for config in changed {
                    if let Err(error) = oxyde_lib::platform::platform().set_recycle_bin_config(config.clone()) {
                        return (saved, Some(error));
                    }
                    saved.push(config);
                }
                (saved, None)
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                let (saved, failure) = result.unwrap_or_else(|error| (Vec::new(), Some(error.to_string())));
                // What was written is the state now, whether the rest failed or not.
                this.trash_delete_policy.saved(&saved);
                match failure {
                    None => {
                        this.config.confirm_delete = confirm;
                        this.save_preferences(cx);
                        this.trash_settings = None;
                    }
                    Some(error) => {
                        if let Some(dialog) = &mut this.trash_settings {
                            dialog.busy = false;
                            for config in &saved {
                                if let Some(initial) = dialog.initial.iter_mut().find(|initial| initial.drive_path == config.drive_path) {
                                    *initial = config.clone();
                                }
                            }
                            dialog.error = Some(error);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn empty_trash_from_settings(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = &self.trash_settings else {
            return;
        };
        if dialog.busy || dialog.loading || dialog.usage.1 == 0 {
            return;
        }
        let count = dialog.usage.1;
        self.prompt_empty_trash(count, cx);
    }

    /// Runs once the prompt is accepted, even if Properties closed meanwhile.
    pub(super) fn empty_trash_confirmed(&mut self, cx: &mut Context<Self>) {
        if let Some(dialog) = &mut self.trash_settings {
            if dialog.busy {
                return;
            }
            dialog.busy = true;
        }
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(|| {
                tokio::runtime::Handle::current().block_on(async {
                    // Read again even after a failure: part of the Trash may be gone.
                    let emptied = oxyde_lib::commands::ops::empty_trash().await;
                    let usage = oxyde_lib::commands::ops::get_total_recycle_bin_usage().await;
                    (emptied, usage)
                })
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(dialog) = &mut this.trash_settings {
                    dialog.busy = false;
                    match result {
                        Ok((emptied, usage)) => {
                            if let Ok(usage) = &usage {
                                dialog.usage = *usage;
                            }
                            if let Some(error) = emptied.err().or(usage.err()) {
                                dialog.error = Some(error.to_string());
                            }
                        }
                        Err(error) => dialog.error = Some(error.to_string()),
                    }
                }
                this.refresh_all_tabs(cx);
                cx.notify();
            });
        })
        .detach();
    }
}
