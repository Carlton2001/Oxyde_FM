use super::*;
use crate::ui::components::{
    icons::{get_icon_type, render_file_icon},
    modal::{action_button, Modal},
};
use crate::ui::format::{date_format, format_date, format_size};
use oxyde_lib::models::{DriveInfo, FileProperties};

pub(crate) struct PropertiesDialog {
    path: String,
    revision: u64,
    properties: Option<FileProperties>,
    drive: Option<DriveInfo>,
    calculating: bool,
    error: Option<String>,
    focus: FocusHandle,
    ok_focus: FocusHandle,
}

fn same_root(a: &str, b: &str) -> bool {
    a.trim_end_matches(['\\', '/'])
        .eq_ignore_ascii_case(b.trim_end_matches(['\\', '/']))
}

impl OxydeApp {
    pub(crate) fn open_file_properties(
        &mut self,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if path == "trash://" {
            self.open_trash_settings(window, cx);
            return;
        }
        self.context_menus.clear();
        if path == "__network_vincinity__" {
            cx.spawn(async move |_, _| {
                let _ = tokio::task::spawn_blocking(move || {
                    oxyde_lib::platform::platform().show_properties_dialog(&path)
                })
                .await;
            })
            .detach();
            cx.notify();
            return;
        }
        self.properties_revision += 1;
        let revision = self.properties_revision;
        let drive = self
            .state
            .drives
            .iter()
            .find(|drive| same_root(&drive.path, &path))
            .cloned();
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        self.properties_dialog = Some(PropertiesDialog {
            path: path.clone(),
            revision,
            properties: None,
            drive,
            calculating: false,
            error: None,
            focus,
            ok_focus: cx.focus_handle(),
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                oxyde_lib::commands::io::get_file_properties(path)
            })
            .await;
            let scan = this
                .update(cx, |this, cx| {
                    let Some(dialog) = this
                        .properties_dialog
                        .as_mut()
                        .filter(|dialog| dialog.revision == revision)
                    else {
                        return None;
                    };
                    let scan = match result {
                        Ok(Ok(properties)) => {
                            let scan = (properties.is_dir && dialog.drive.is_none())
                                .then(|| properties.path.clone());
                            dialog.calculating = scan.is_some();
                            dialog.properties = Some(properties);
                            scan
                        }
                        Ok(Err(error)) => {
                            dialog.error = Some(error.to_string());
                            None
                        }
                        Err(error) => {
                            dialog.error = Some(error.to_string());
                            None
                        }
                    };
                    cx.notify();
                    scan
                })
                .ok()
                .flatten();
            if let Some(path) = scan {
                let result = oxyde_lib::commands::io::calculate_folder_size(path).await;
                let _ = this.update(cx, |this, cx| {
                    let Some(dialog) = this
                        .properties_dialog
                        .as_mut()
                        .filter(|dialog| dialog.revision == revision)
                    else {
                        return;
                    };
                    dialog.calculating = false;
                    match result {
                        Ok(size) => {
                            if let Some(properties) = &mut dialog.properties {
                                properties.size = size.size;
                                properties.files_count = Some(size.files_count);
                                properties.folders_count = Some(size.folders_count);
                                properties.is_calculated = true;
                            }
                        }
                        Err(error) => dialog.error = Some(error.to_string()),
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(crate) fn close_file_properties(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.properties_dialog = None;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    fn system_file_properties(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = &self.properties_dialog else {
            return;
        };
        let path = dialog.path.clone();
        cx.spawn(async move |_, _| {
            let _ = tokio::task::spawn_blocking(move || {
                oxyde_lib::platform::platform().show_properties_dialog(&path)
            })
            .await;
        })
        .detach();
    }
}

pub(crate) fn render(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let dialog = view.properties_dialog.as_ref().unwrap();
    let theme = *cx.theme();
    let mut font = window.text_style().font();
    font.family = "Inter".into();
    font.weight = crate::ui::style::DEFAULT_FONT_WEIGHT;
    let label_width = [
        Message::ColumnType,
        Message::ColumnLocation,
        Message::ColumnSize,
        Message::PropertiesUsed,
        Message::TrashAvailableSpace,
        Message::PropertiesCapacity,
        Message::PropertiesContains,
        Message::PropertiesCreated,
        Message::ColumnModified,
        Message::PropertiesAccessed,
    ]
    .into_iter()
    .map(|message| {
        let label = cx.i18n().text(message);
        window
            .text_system()
            .shape_line(
                label.clone(),
                crate::ui::scale::rem_pixels(rems(0.8125)),
                &[TextRun {
                    len: label.len(),
                    font: font.clone(),
                    color: theme.text_muted,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }],
                None,
            )
            .width
            .as_f32()
    })
    .fold(crate::ui::scale::rem_pixels(rems(5.5)).as_f32(), f32::max)
    .ceil();
    let row = |label: SharedString, value: String| {
        div()
            .flex()
            .gap(rems(0.75))
            .items_start()
            .child(
                div()
                    .w(px(label_width))
                    .flex_none()
                    .whitespace_nowrap()
                    .text_color(theme.text_muted)
                    .child(label),
            )
            .child(div().flex_1().min_w_0().child(value))
    };
    let divider = || {
        div()
            .h(rems(0.0625))
            .w_full()
            .bg(theme.border)
            .my(rems(0.75))
    };
    let mut body = div().flex().flex_col().gap(rems(0.375));
    if let Some(p) = &dialog.properties {
        let icon = get_icon_type(&p.name, p.is_dir, Some(&p.path), false, false);
        let graphic = if let Some(drive) = &dialog.drive {
            if cfg!(target_os = "windows") && view.use_system_icons() {
                render_file_icon(
                    p.path.clone(),
                    icon,
                    rems(3.0),
                    theme.icon_muted,
                    true,
                    true,
                )
            } else {
                svg()
                    .path(crate::ui::components::icons::drive_icon_path(drive))
                    .size(rems(3.0))
                    .text_color(theme.icon_muted)
                    .into_any_element()
            }
        } else {
            render_file_icon(
                p.path.clone(),
                icon,
                rems(3.0),
                if p.is_dir {
                    theme.accent
                } else {
                    theme.icon_muted
                },
                cfg!(target_os = "windows") && view.use_system_icons(),
                true,
            )
        };
        let name = if let Some(drive) = &dialog.drive {
            format!("{} ({})", drive.label, drive.path.trim_end_matches('\\'))
        } else {
            p.name.clone()
        };
        let copy_name = name.clone();
        body = body.child(
            div()
                .flex()
                .items_center()
                .gap(rems(1.25))
                .mb(rems(1.25))
                .child(div().flex_none().child(graphic))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .px(rems(0.625))
                        .py(rems(0.5))
                        .border(rems(0.0625))
                        .border_color(theme.border)
                        .rounded_sm()
                        .h(rems(2.0))
                        .py_0()
                        .flex()
                        .items_center()
                        .id("properties-name")
                        .tooltip(crate::ui::components::tooltip::Tooltip::text(
                            cx.i18n().text(Message::PropertiesCopyNameDoubleClick),
                        ))
                        .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                            if event.click_count == 2 {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy_name.clone()));
                            }
                            cx.stop_propagation();
                        })
                        .child(name),
                ),
        );
        let file_type = if p.is_dir {
            cx.i18n().text(Message::FileTypeFolder).to_string()
        } else {
            std::path::Path::new(&p.name)
                .extension()
                .map(|extension| extension.to_string_lossy().to_uppercase())
                .unwrap_or_else(|| cx.i18n().text(Message::FileTypeFile).to_string())
        };
        body = body.child(
            div()
                .flex()
                .items_center()
                .gap(rems(0.75))
                .child(
                    div()
                        .w(px(label_width))
                        .flex_none()
                        .whitespace_nowrap()
                        .text_color(theme.text_muted)
                        .child(cx.i18n().text(Message::ColumnType)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(rems(0.625))
                        .child(if dialog.drive.is_some() {
                            cx.i18n().text(Message::PropertiesDrive).to_string()
                        } else {
                            file_type
                        })
                        .when_some(
                            dialog
                                .drive
                                .as_ref()
                                .and_then(|drive| drive.media_type.as_ref())
                                .filter(|media| !media.is_empty()),
                            |row, media| {
                                let color = if media.contains("SSD") {
                                    theme.media_ssd
                                } else if media.contains("HDD") {
                                    theme.media_hdd
                                } else if media.starts_with("USB") {
                                    theme.media_removable
                                } else if media == "NAS" {
                                    theme.media_network
                                } else {
                                    theme.badge_surface
                                };
                                row.child(
                                    div()
                                        .flex_none()
                                        .whitespace_nowrap()
                                        .rounded(rems(0.2))
                                        .px(rems(0.35))
                                        .py(rems(0.0625))
                                        .text_size(rems(0.625))
                                        .font_weight(FontWeight::MEDIUM)
                                        .bg(color)
                                        .text_color(theme.text_on_badge)
                                        .child(media.clone()),
                                )
                            },
                        ),
                ),
        );
        if let Some(drive) = &dialog.drive {
            let used = drive.total_bytes.saturating_sub(drive.free_bytes);
            for (label, size) in [
                (Message::PropertiesUsed, used),
                (Message::TrashAvailableSpace, drive.free_bytes),
                (Message::PropertiesCapacity, drive.total_bytes),
            ] {
                body = body.child(row(
                    cx.i18n().text(label),
                    format_size(size, false, false, cx.i18n().language()),
                ));
            }
            if drive.total_bytes > 0 {
                let ratio = (used as f64 / drive.total_bytes as f64) as f32;
                body = body.child(divider()).child(
                    div()
                        .relative()
                        .w_full()
                        .h(rems(1.125))
                        .bg(theme.border)
                        .rounded_sm()
                        .overflow_hidden()
                        .child(
                            div()
                                .absolute()
                                .left_0()
                                .top_0()
                                .h_full()
                                .w(relative(ratio))
                                .bg(theme.accent),
                        )
                        .child(
                            div()
                                .relative()
                                .w_full()
                                .h_full()
                                .flex()
                                .justify_center()
                                .items_center()
                                .text_size(rems(0.6875))
                                .text_color(if ratio > 0.5 {
                                    theme.text_on_accent
                                } else {
                                    theme.text
                                })
                                .child(format!("{}%", (ratio * 100.0).round() as u32)),
                        ),
                );
            }
        } else {
            body = body
                .child(row(
                    cx.i18n().text(Message::ColumnLocation),
                    p.parent.clone(),
                ))
                .child(row(
                    cx.i18n().text(Message::ColumnSize),
                    if dialog.calculating {
                        cx.i18n().text(Message::PreviewLoading).to_string()
                    } else {
                        format_size(p.size, p.is_dir, p.is_calculated, cx.i18n().language())
                    },
                ));
            if let (Some(files), Some(folders)) = (p.files_count, p.folders_count) {
                let mut args = FluentArgs::new();
                args.set(
                    "files",
                    crate::ui::format::format_number(files, cx.i18n().language()),
                );
                args.set(
                    "folders",
                    crate::ui::format::format_number(folders, cx.i18n().language()),
                );
                body = body.child(row(
                    cx.i18n().text(Message::PropertiesContains),
                    cx.i18n()
                        .text_with_args(Message::PropertiesCounts, &args)
                        .to_string(),
                ));
            }
        }
        body = body.child(divider());
        for (label, timestamp) in [
            (Message::PropertiesCreated, p.created),
            (Message::ColumnModified, p.modified),
            (Message::PropertiesAccessed, p.accessed),
        ] {
            body = body.child(row(
                cx.i18n().text(label),
                format_date(timestamp, date_format(cx)),
            ));
        }
    } else if dialog.error.is_none() {
        body = body.child(cx.i18n().text(Message::PreviewLoading));
    }
    body = body.when_some(dialog.error.as_ref(), |body, error| {
        body.child(
            div()
                .text_color(theme.error)
                .child(cx.i18n().text(Message::OperationFailed))
                .child(error.clone()),
        )
    });
    let keyboard = handle.clone();
    let close = handle.clone();
    let cancel = handle.clone();
    let ok = handle.clone();
    let system = handle.clone();
    let title_close = handle;
    div()
        .absolute()
        .inset_0()
        .track_focus(&dialog.focus)
        .on_key_down(move |event, window, cx| {
            keyboard.update(cx, |this, cx| match event.keystroke.key.as_str() {
                "escape" | "enter" | "space" => this.close_file_properties(window, cx),
                "tab" => {
                    if let Some(dialog) = &this.properties_dialog {
                        window.focus(
                            if dialog.ok_focus.is_focused(window) {
                                &dialog.focus
                            } else {
                                &dialog.ok_focus
                            },
                            cx,
                        );
                    }
                }
                _ => {}
            });
            cx.stop_propagation();
        })
        .on_action(|_: &Supprimer, _, cx| cx.stop_propagation())
        .on_action(|_: &Ouvrir, _, cx| cx.stop_propagation())
        .on_action(|_: &Renommer, _, cx| cx.stop_propagation())
        .on_action(|_: &Couper, _, cx| cx.stop_propagation())
        .on_action(|_: &Copier, _, cx| cx.stop_propagation())
        .on_action(|_: &Coller, _, cx| cx.stop_propagation())
        .child(
            Modal::new("file-properties", cx.i18n().text(Message::Properties), body)
                .width(rems(28.0))
                .title_icon("icons/settings-2.svg")
                .header(
                    div()
                        .flex()
                        .items_center()
                        .gap(rems(0.5))
                        .child(
                            svg()
                                .path("icons/settings-2.svg")
                                .size(rems(1.0))
                                .flex_none()
                                .text_color(theme.text_muted),
                        )
                        .child(div().flex_1().child(cx.i18n().text(Message::Properties)))
                        .child(
                            div()
                                .id("properties-close")
                                .child(svg().path("icons/x.svg").size(rems(1.0)))
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    cx.stop_propagation();
                                    title_close.update(cx, |this, cx| {
                                        this.close_file_properties(window, cx)
                                    });
                                }),
                        ),
                )
                .on_backdrop_dismiss(move |window, cx| {
                    close.update(cx, |this, cx| this.close_file_properties(window, cx));
                })
                .action(
                    action_button("properties-system")
                        .bg(theme.control_hover)
                        .child(cx.i18n().text(Message::PropertiesSystem))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            system.update(cx, |this, cx| this.system_file_properties(cx));
                        }),
                )
                .action(
                    action_button("properties-cancel")
                        .track_focus(&dialog.focus)
                        .bg(theme.control_hover)
                        .child(cx.i18n().text(Message::Cancel))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            cancel.update(cx, |this, cx| this.close_file_properties(window, cx));
                        }),
                )
                .action(
                    action_button("properties-ok")
                        .track_focus(&dialog.ok_focus)
                        .bg(theme.accent)
                        .text_color(theme.text_on_accent)
                        .child(cx.i18n().text(Message::TrashSettingsOk))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            ok.update(cx, |this, cx| this.close_file_properties(window, cx));
                        }),
                ),
        )
}
