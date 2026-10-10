use super::*;
use crate::ui::components::modal::{action_button, Modal};
use crate::ui::theme::ThemeContext;
use oxyde_lib::commands::ops::TrashSystemConfig;
use oxyde_lib::models::DriveInfo;

pub(crate) struct TrashSettings {
    drives: Vec<DriveInfo>,
    configs: Vec<TrashSystemConfig>,
    initial: Vec<TrashSystemConfig>,
    selected: usize,
    percent: bool,
    input: Entity<TextInput>,
    value: String,
    value_dirty: bool,
    confirm: bool,
    usage: (u64, u64),
    loading: bool,
    busy: bool,
    error: Option<String>,
    focus: FocusHandle,
    save_focus: FocusHandle,
}

fn capacity_mb(value: &str, percent: bool, total_bytes: u64) -> Option<u64> {
    let number: u64 = value.parse().ok()?;
    if percent {
        if number > 100 {
            return None;
        }
        if total_bytes == 0 {
            return None;
        }
        let capacity = ((total_bytes as u128 * number as u128) / (100 * 1024 * 1024)) as u64;
        (capacity <= u32::MAX as u64).then_some(capacity)
    } else {
        (number <= u32::MAX as u64).then_some(number)
    }
}

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
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                for config in changed {
                    oxyde_lib::platform::platform().set_recycle_bin_config(config)?;
                }
                Ok::<_, String>(())
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(())) => {
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

fn toggle(value: bool, theme: crate::ui::theme::OxydeTheme) -> impl IntoElement {
    div()
        .w(rems(1.875))
        .h(rems(1.125))
        .rounded_full()
        .bg(if value { theme.accent } else { theme.border })
        .flex()
        .items_center()
        .flex_none()
        .px(rems(0.125))
        .when(value, |d| d.justify_end())
        .child(
            div()
                .size(rems(0.875))
                .rounded_full()
                .bg(theme.surface)
                .flex_none(),
        )
}

pub(crate) struct PendingDelete {
    pub paths: Vec<String>,
    pub permanent: bool,
    pub focus: FocusHandle,
    pub confirm_focus: FocusHandle,
}

pub(crate) fn render(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let dialog = view.trash_settings.as_ref().unwrap();
    let theme = *cx.theme();
    let disabled = dialog.loading || dialog.busy;
    let size = crate::ui::format::format_size(dialog.usage.0, false, false, cx.i18n().language());
    let empty = handle.clone();
    let cancel = handle.clone();
    let save = handle.clone();
    let close = handle.clone();
    let keyboard = handle.clone();
    let title_close = handle.clone();
    let section_title = |message: Message, icon: &'static str| {
        div()
            .flex()
            .items_center()
            .gap(rems(0.5))
            .text_size(rems(0.6875))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.text_muted)
            .child(
                svg()
                    .path(icon)
                    .size(rems(0.8125))
                    .flex_none()
                    .text_color(theme.text_muted),
            )
            .child(cx.i18n().text(message).to_uppercase())
    };
    let mut count_args = FluentArgs::new();
    count_args.set("count", dialog.usage.1);
    count_args.set(
        "formatted",
        crate::ui::format::format_number(dialog.usage.1, cx.i18n().language()),
    );
    let mut table = div()
        .rounded_md()
        .border(rems(0.0625))
        .border_color(theme.border)
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .justify_between()
                .px(rems(0.75))
                .py(rems(0.5))
                .bg(theme.header)
                .text_size(rems(0.6875))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .child(cx.i18n().text(Message::ColumnLocation).to_uppercase())
                .child(cx.i18n().text(Message::TrashAvailableSpace).to_uppercase()),
        );
    let mut rows = div()
        .id("trash-drive-rows")
        .max_h(rems(8.0))
        .overflow_y_scroll()
        .flex()
        .flex_col();
    for (index, drive) in dialog.drives.iter().enumerate() {
        let h = handle.clone();
        let selected = index == dialog.selected;
        rows = rows.child(
            div()
                .id(("trash-drive", index))
                .flex()
                .items_center()
                .gap(rems(0.5))
                .px(rems(0.75))
                .py(rems(0.625))
                .border_t(rems(0.0625))
                .border_color(theme.border)
                .bg(if selected {
                    theme.selected
                } else {
                    theme.surface
                })
                .text_color(if selected { theme.accent } else { theme.text })
                .when(!selected, |row| row.hover(|row| row.bg(theme.hover)))
                .child(
                    svg()
                        .path("icons/hard-drive.svg")
                        .size(rems(0.8125))
                        .flex_none()
                        .text_color(if selected {
                            theme.accent
                        } else {
                            theme.icon_muted
                        }),
                )
                .child(div().flex_1().min_w_0().truncate().child(format!(
                    "{} ({})",
                    drive.label,
                    drive.path.trim_end_matches('\\')
                )))
                .child(
                    div()
                        .flex_none()
                        .text_color(if selected {
                            theme.accent
                        } else {
                            theme.text_muted
                        })
                        .child(crate::ui::format::format_size(
                            drive.free_bytes,
                            false,
                            false,
                            cx.i18n().language(),
                        )),
                )
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    h.update(cx, |this, cx| this.select_trash_drive(index, false, cx));
                }),
        );
    }
    table = table.child(rows);
    let mut body = div()
        .flex()
        .flex_col()
        .gap(rems(1.5))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(rems(0.75))
                .px(rems(1.25))
                .py(rems(1.0))
                .border(rems(0.0625))
                .border_color(theme.border)
                .rounded_lg()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(rems(0.75))
                        .child(
                            svg()
                                .path("icons/trash-2.svg")
                                .size(rems(1.0))
                                .flex_none()
                                .text_color(theme.accent),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_size(rems(1.125))
                                        .font_weight(FontWeight::BOLD)
                                        .child(size),
                                )
                                .child(
                                    div().text_color(theme.text_muted).child(
                                        cx.i18n()
                                            .text_with_args(Message::TrashItemCount, &count_args),
                                    ),
                                ),
                        ),
                )
                .child(
                    action_button("trash-settings-empty")
                        .flex_none()
                        .bg(theme.control_hover)
                        .opacity(if disabled || dialog.usage.1 == 0 {
                            0.5
                        } else {
                            1.0
                        })
                        .child(cx.i18n().text(Message::TrashDeleteAll))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            empty.update(cx, |this, cx| this.empty_trash_from_settings(cx));
                        }),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(rems(0.5))
                .child(section_title(Message::TrashLocations, "icons/database.svg"))
                .child(table),
        );
    if let Some(config) = dialog.configs.get(dialog.selected) {
        let index = dialog.selected;
        let mb = handle.clone();
        let percent = handle.clone();
        let nuke = handle.clone();
        let card = div()
            .flex()
            .flex_col()
            .gap(rems(1.25))
            .p(rems(1.25))
            .rounded_lg()
            .bg(theme.header)
            .border(rems(0.0625))
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(rems(0.75))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(rems(0.125))
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(cx.i18n().text(Message::TrashCustomSize)),
                            )
                            .child(
                                div()
                                    .text_color(theme.text_muted)
                                    .child(cx.i18n().text(Message::TrashMaxCapacity)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_none()
                            .h(rems(1.75))
                            .bg(theme.surface)
                            .border(rems(0.0625))
                            .border_color(theme.border)
                            .rounded_sm()
                            .overflow_hidden()
                            .opacity(if config.nuke_on_delete || disabled {
                                0.5
                            } else {
                                1.0
                            })
                            .child(
                                div()
                                    .w(rems(5.5))
                                    .h_full()
                                    .px(rems(0.625))
                                    .when(!disabled && !config.nuke_on_delete, |d| {
                                        d.child(dialog.input.clone())
                                    })
                                    .when(disabled || config.nuke_on_delete, |d| {
                                        d.flex().items_center().child(dialog.value.clone())
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .h_full()
                                    .p(rems(0.125))
                                    .border_l(rems(0.0625))
                                    .border_color(theme.border)
                                    .child(
                                        action_button("trash-unit-mb")
                                            .h_full()
                                            .px(rems(0.5))
                                            .py_0()
                                            .flex()
                                            .items_center()
                                            .text_size(rems(0.6875))
                                            .rounded_sm()
                                            .bg(if !dialog.percent {
                                                theme.accent
                                            } else {
                                                theme.surface
                                            })
                                            .text_color(if !dialog.percent {
                                                theme.text_on_accent
                                            } else {
                                                theme.text_muted
                                            })
                                            .child(cx.i18n().text(Message::TrashMegabytes))
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                mb.update(cx, |this, cx| {
                                                    this.select_trash_drive(index, false, cx)
                                                });
                                            }),
                                    )
                                    .child(
                                        action_button("trash-unit-percent")
                                            .h_full()
                                            .px(rems(0.5))
                                            .py_0()
                                            .flex()
                                            .items_center()
                                            .text_size(rems(0.6875))
                                            .rounded_sm()
                                            .bg(if dialog.percent {
                                                theme.accent
                                            } else {
                                                theme.surface
                                            })
                                            .text_color(if dialog.percent {
                                                theme.text_on_accent
                                            } else {
                                                theme.text_muted
                                            })
                                            .child("%")
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                percent.update(cx, |this, cx| {
                                                    this.select_trash_drive(index, true, cx)
                                                });
                                            }),
                                    ),
                            ),
                    ),
            )
            .child(div().h(rems(0.0625)).w_full().bg(theme.border))
            .child(
                div()
                    .id("trash-nuke")
                    .flex()
                    .items_center()
                    .gap(rems(1.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(rems(0.125))
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(cx.i18n().text(Message::TrashDontMove)),
                            )
                            .child(
                                div()
                                    .text_color(theme.text_muted)
                                    .child(cx.i18n().text(Message::TrashDeleteImmediately)),
                            ),
                    )
                    .child(toggle(config.nuke_on_delete, theme))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        nuke.update(cx, |this, cx| {
                            if let Some(d) = &mut this.trash_settings {
                                if !d.busy && !d.loading {
                                    d.configs[d.selected].nuke_on_delete =
                                        !d.configs[d.selected].nuke_on_delete;
                                }
                            }
                            cx.notify();
                        });
                    }),
            );
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap(rems(0.5))
                .child(section_title(
                    Message::TrashSelectedSettings,
                    "icons/settings-2.svg",
                ))
                .child(card),
        );
    }
    let confirm = handle.clone();
    body = body
        .child(
            div()
                .id("trash-confirm")
                .flex()
                .items_center()
                .gap(rems(1.0))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(cx.i18n().text(Message::TrashConfirmDelete)),
                )
                .child(toggle(dialog.confirm, theme))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    confirm.update(cx, |this, cx| {
                        if let Some(d) = &mut this.trash_settings {
                            if !d.busy && !d.loading {
                                d.confirm = !d.confirm;
                            }
                        }
                        cx.notify();
                    });
                }),
        )
        .when_some(dialog.error.as_ref(), |body, error| {
            body.child(
                div()
                    .text_color(theme.error)
                    .child(cx.i18n().text(Message::TrashSettingsError))
                    .child(error.clone()),
            )
        })
        .when(disabled, |body| {
            body.child(cx.i18n().text(Message::PreviewLoading))
        });
    div()
        .absolute()
        .inset_0()
        .track_focus(&dialog.focus)
        .on_key_down(move |event, window, cx| {
            keyboard.update(cx, |this, cx| match event.keystroke.key.as_str() {
                "escape" => this.close_trash_settings(window, cx),
                "tab" => {
                    if let Some(dialog) = &this.trash_settings {
                        let input_focus = dialog.input.read(cx).focus_handle.clone();
                        let controls = [&dialog.focus, &input_focus, &dialog.save_focus];
                        let current = controls
                            .iter()
                            .position(|focus| focus.is_focused(window))
                            .unwrap_or(0);
                        let next = if event.keystroke.modifiers.shift {
                            (current + 2) % 3
                        } else {
                            (current + 1) % 3
                        };
                        window.focus(controls[next], cx);
                    }
                }
                "enter" | "space" => {
                    if this
                        .trash_settings
                        .as_ref()
                        .is_some_and(|dialog| dialog.save_focus.is_focused(window))
                    {
                        this.save_trash_settings(cx);
                    } else {
                        this.close_trash_settings(window, cx);
                    }
                }
                _ => {}
            });
            cx.stop_propagation();
        })
        .on_action(|_: &Supprimer, _, cx| cx.stop_propagation())
        .on_action(|_: &Ouvrir, _, cx| cx.stop_propagation())
        .on_action(|_: &Renommer, _, cx| cx.stop_propagation())
        .on_action(|_: &Copier, _, cx| cx.stop_propagation())
        .on_action(|_: &Couper, _, cx| cx.stop_propagation())
        .on_action(|_: &Coller, _, cx| cx.stop_propagation())
        .child(
            Modal::new(
                "trash-settings",
                cx.i18n().text(Message::TrashProperties),
                body,
            )
            .title_icon("icons/trash-2.svg")
            .header(
                div()
                    .flex()
                    .items_center()
                    .gap(rems(0.5))
                    .child(
                        svg()
                            .path("icons/trash-2.svg")
                            .size(rems(1.0))
                            .text_color(theme.text_muted),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(cx.i18n().text(Message::TrashProperties)),
                    )
                    .child(
                        div()
                            .id("trash-settings-close")
                            .child(svg().path("icons/x.svg").size(rems(1.0)))
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                cx.stop_propagation();
                                title_close
                                    .update(cx, |this, cx| this.close_trash_settings(window, cx));
                            }),
                    ),
            )
            .width(rems(28.0))
            .on_backdrop_dismiss(move |window, cx| {
                close.update(cx, |this, cx| this.close_trash_settings(window, cx));
            })
            .action(
                action_button("trash-settings-cancel")
                    .bg(theme.control_hover)
                    .track_focus(&dialog.focus)
                    .child(cx.i18n().text(Message::Cancel))
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        cancel.update(cx, |this, cx| this.close_trash_settings(window, cx));
                    }),
            )
            .action(
                action_button("trash-settings-save")
                    .track_focus(&dialog.save_focus)
                    .bg(theme.accent)
                    .text_color(theme.text_on_accent)
                    .opacity(if disabled { 0.5 } else { 1.0 })
                    .child(cx.i18n().text(Message::TrashSettingsOk))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        save.update(cx, |this, cx| this.save_trash_settings(cx));
                    }),
            ),
        )
}

pub(crate) fn render_delete_confirmation(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let pending = view.pending_trash_delete.as_ref().unwrap();
    if !pending.focus.is_focused(window) && !pending.confirm_focus.is_focused(window) {
        window.focus(&pending.focus, cx);
    }
    let theme = *cx.theme();
    let cancel = handle.clone();
    let close = handle.clone();
    let keyboard = handle.clone();
    div()
        .absolute()
        .inset_0()
        .track_focus(&pending.focus)
        .on_key_down(move |event, window, cx| {
            keyboard.update(cx, |this, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => {
                        this.pending_trash_delete = None;
                        window.focus(&this.focus_handle, cx);
                    }
                    "tab" => {
                        if let Some(pending) = &this.pending_trash_delete {
                            window.focus(
                                if pending.confirm_focus.is_focused(window) {
                                    &pending.focus
                                } else {
                                    &pending.confirm_focus
                                },
                                cx,
                            );
                        }
                    }
                    "enter" | "space" => {
                        if let Some(pending) = this.pending_trash_delete.take() {
                            if pending.confirm_focus.is_focused(window) {
                                this.queue_trash_delete(pending.paths, pending.permanent, cx);
                            }
                            window.focus(&this.focus_handle, cx);
                        }
                    }
                    _ => {}
                }
                cx.notify();
            });
            cx.stop_propagation();
        })
        .on_action(|_: &Supprimer, _, cx| cx.stop_propagation())
        .on_action(|_: &Ouvrir, _, cx| cx.stop_propagation())
        .on_action(|_: &Renommer, _, cx| cx.stop_propagation())
        .on_action(|_: &Copier, _, cx| cx.stop_propagation())
        .on_action(|_: &Couper, _, cx| cx.stop_propagation())
        .on_action(|_: &Coller, _, cx| cx.stop_propagation())
        .child(
            Modal::new(
                "trash-delete-confirmation",
                cx.i18n().text(if pending.permanent { Message::TrashDeletePermanent } else { Message::Delete }),
                div().child(cx.i18n().text(if pending.permanent { Message::TrashPermanentDeletePrompt } else { Message::TrashDeletePrompt })),
            )
            .title_icon("icons/trash-2.svg")
            .on_backdrop_dismiss(move |window, cx| {
                close.update(cx, |this, cx| {
                    this.pending_trash_delete = None;
                    window.focus(&this.focus_handle, cx);
                    cx.notify();
                });
            })
            .action(
                action_button("trash-delete-cancel")
                    .track_focus(&pending.focus)
                    .child(cx.i18n().text(Message::Cancel))
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        cancel.update(cx, |this, cx| {
                            this.pending_trash_delete = None;
                            window.focus(&this.focus_handle, cx);
                            cx.notify();
                        });
                    }),
            )
            .action(
                action_button("trash-delete-confirm")
                    .track_focus(&pending.confirm_focus)
                    .bg(theme.accent)
                    .text_color(theme.text_on_accent)
                    .child(cx.i18n().text(if pending.permanent { Message::TrashDeletePermanent } else { Message::Delete }))
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        handle.update(cx, |this, cx| {
                            if let Some(pending) = this.pending_trash_delete.take() {
                                this.queue_trash_delete(pending.paths, pending.permanent, cx);
                            }
                            window.focus(&this.focus_handle, cx);
                            cx.notify();
                        });
                    }),
            ),
        )
}

#[cfg(test)]
mod tests {
    #[test]
    fn capacity_conversion_rejects_invalid_input_and_percent_overflow() {
        assert_eq!(super::capacity_mb("25", true, 400 * 1024 * 1024), Some(100));
        assert_eq!(super::capacity_mb("101", true, 400 * 1024 * 1024), None);
        assert_eq!(super::capacity_mb("-1", false, 0), None);
        assert_eq!(super::capacity_mb("4294967296", false, 0), None);
        assert_eq!(super::capacity_mb("123", false, 0), Some(123));
    }
}
