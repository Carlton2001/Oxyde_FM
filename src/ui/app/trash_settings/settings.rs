use super::*;

impl OxydeApp {
    pub(crate) fn open_trash_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                true,
                true,
                false,
                None,
                window,
            )
        });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
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
                    .list_drives(true)
                    .into_iter()
                    .filter(|drive| matches!(drive.drive_type.as_str(), "fixed" | "removable"))
                    .collect();
                let usage = runtime
                    .block_on(oxyde_lib::commands::ops::get_total_recycle_bin_usage())
                    .map_err(|error| error.to_string())?;
                let configs = drives
                    .iter()
                    .map(|drive| {
                        oxyde_lib::platform::platform().recycle_bin_config(drive.path.clone())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok::<_, String>((drives, configs, usage))
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
                        Ok(Ok((drives, configs, usage))) => {
                            dialog.drives = drives;
                            dialog.initial = configs.clone();
                            dialog.configs = configs;
                            dialog.usage = usage;
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
        window.focus(&self.focus_handle, cx);
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
        if self
            .trash_settings
            .as_ref()
            .is_some_and(|dialog| !dialog.value.is_empty())
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
            let total = dialog.drives[index].total_bytes;
            if total == 0 {
                "0".into()
            } else {
                ((config.max_size_mb as u128 * 1024 * 1024 * 100 / total as u128) as u64)
                    .to_string()
            }
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
                for config in &changed {
                    oxyde_lib::platform::platform().set_recycle_bin_config(config.clone())?;
                }
                Ok::<_, String>(changed)
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(configs)) => {
                        this.trash_delete_policy.saved(&configs);
                        this.config.confirm_delete = confirm;
                        this.save_preferences(cx);
                        this.trash_settings = None;
                    }
                    result => {
                        if let Some(dialog) = &mut this.trash_settings {
                            dialog.busy = false;
                            dialog.error = Some(match result {
                                Ok(Err(error)) => error,
                                Err(error) => error.to_string(),
                                _ => unreachable!(),
                            });
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn empty_trash_from_settings(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = &mut self.trash_settings else {
            return;
        };
        if dialog.busy || dialog.loading || dialog.usage.1 == 0 {
            return;
        }
        dialog.busy = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(|| {
                tokio::runtime::Handle::current().block_on(async {
                    oxyde_lib::commands::ops::empty_trash().await?;
                    oxyde_lib::commands::ops::get_total_recycle_bin_usage().await
                })
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(dialog) = &mut this.trash_settings {
                    dialog.busy = false;
                    match result {
                        Ok(Ok(usage)) => dialog.usage = usage,
                        Ok(Err(error)) => dialog.error = Some(error.to_string()),
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
