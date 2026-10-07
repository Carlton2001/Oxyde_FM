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
    shortcut_tab: bool,
    shortcut_inputs: Vec<Entity<TextInput>>,
    run_window: i32,
    run_menu: Option<Point<Pixels>>,
    run_focus: FocusHandle,
    run_bounds: Option<Bounds<Pixels>>,
    saving: bool,
    validation_error: Option<Message>,
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
            shortcut_tab: false,
            shortcut_inputs: Vec::new(),
            run_window: 1,
            run_menu: None,
            run_focus: cx.focus_handle(),
            run_bounds: None,
            saving: false,
            validation_error: None,
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
        if self
            .properties_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.saving)
        {
            return;
        }
        self.properties_dialog = None;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    fn select_properties_tab(
        &mut self,
        shortcut: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = &self.properties_dialog else {
            return;
        };
        if dialog.saving {
            return;
        }
        let values = dialog
            .properties
            .as_ref()
            .and_then(|properties| properties.shortcut.as_ref())
            .map(|info| {
                (
                    vec![
                        info.target.clone(),
                        info.arguments.clone(),
                        info.working_dir.clone(),
                        info.description.clone(),
                    ],
                    info.run_window,
                )
            });
        if shortcut && dialog.shortcut_inputs.is_empty() {
            let Some((values, run_window)) = values else {
                return;
            };
            let handle = cx.entity();
            let mut inputs = Vec::new();
            for value in values {
                let submit = handle.clone();
                let cancel = handle.clone();
                let context = handle.clone();
                inputs.push(cx.new(|cx| {
                    TextInput::new(
                        cx,
                        "",
                        value,
                        |_, _, _| {},
                        move |window, cx| {
                            submit.update(cx, |this, cx| this.accept_properties(window, cx))
                        },
                        move |window, cx| {
                            cancel.update(cx, |this, cx| this.close_file_properties(window, cx))
                        },
                        move |input, selected, point, window, cx| {
                            context.update(cx, |this, cx| {
                                this.open_text_context_menu(input, selected, point, window, cx)
                            })
                        },
                        false,
                        false,
                        false,
                        None,
                        window,
                    )
                }));
            }
            let dialog = self.properties_dialog.as_mut().unwrap();
            dialog.shortcut_inputs = inputs;
            dialog.run_window = run_window;
        }
        let dialog = self.properties_dialog.as_mut().unwrap();
        dialog.shortcut_tab = shortcut;
        dialog.run_menu = None;
        window.focus(&dialog.focus, cx);
        cx.notify();
    }

    fn accept_properties(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = &self.properties_dialog else {
            return;
        };
        if dialog.saving {
            return;
        }
        if dialog.shortcut_inputs.is_empty() {
            self.close_file_properties(window, cx);
            return;
        }
        let Some(mut info) = dialog
            .properties
            .as_ref()
            .and_then(|properties| properties.shortcut.clone())
        else {
            return;
        };
        let fields: Vec<_> = dialog
            .shortcut_inputs
            .iter()
            .map(|input| input.read(cx).content().to_string())
            .collect();
        if fields[0].trim().is_empty() || fields.iter().any(|value| value.contains('\0')) {
            self.properties_dialog.as_mut().unwrap().validation_error =
                Some(Message::ShortcutInvalidTarget);
            cx.notify();
            return;
        }
        info.target = fields[0].clone();
        info.arguments = fields[1].clone();
        info.working_dir = fields[2].clone();
        info.description = fields[3].clone();
        info.run_window = dialog.run_window;
        let original = dialog
            .properties
            .as_ref()
            .unwrap()
            .shortcut
            .as_ref()
            .unwrap();
        if info.target == original.target
            && info.arguments == original.arguments
            && info.working_dir == original.working_dir
            && info.description == original.description
            && info.run_window == original.run_window
        {
            self.close_file_properties(window, cx);
            return;
        }
        let path = dialog.path.clone();
        let revision = dialog.revision;
        let dialog = self.properties_dialog.as_mut().unwrap();
        dialog.saving = true;
        dialog.error = None;
        dialog.validation_error = None;
        dialog.run_menu = None;
        window.focus(&dialog.ok_focus, cx);
        cx.notify();
        let window_handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let result = oxyde_lib::commands::io::set_shortcut_info(path, info).await;
            let _ = window_handle.update(cx, |_, window, cx| {
                this.update(cx, |this, cx| {
                    let Some(dialog) = this
                        .properties_dialog
                        .as_mut()
                        .filter(|dialog| dialog.revision == revision)
                    else {
                        return;
                    };
                    dialog.saving = false;
                    match result {
                        Ok(()) => {
                            this.close_file_properties(window, cx);
                            this.refresh_all(cx);
                        }
                        Err(error) => dialog.error = Some(error.to_string()),
                    }
                    cx.notify();
                })
            });
        })
        .detach();
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
    if let Some(p) = dialog.properties.as_ref().filter(|_| !dialog.shortcut_tab) {
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
    } else if dialog.shortcut_tab {
        for (index, message) in [
            Message::ShortcutTarget,
            Message::ShortcutArguments,
            Message::ShortcutWorkingDirectory,
            Message::ShortcutComment,
        ]
        .into_iter()
        .enumerate()
        {
            body = body.child(crate::ui::components::modal::form_row_with_label_width(
                cx.i18n().text(message),
                div()
                    .h(rems(2.0))
                    .flex_1()
                    .min_w_0()
                    .child(dialog.shortcut_inputs[index].clone()),
                rems(6.0),
                cx,
            ));
        }
        let picker = handle.clone();
        let measure = handle.clone();
        body =
            body.child(divider())
                .child(crate::ui::components::modal::form_row_with_label_width(
                    cx.i18n().text(Message::ShortcutRun),
                    div()
                        .id("shortcut-run")
                        .relative()
                        .track_focus(&dialog.run_focus)
                        .flex_1()
                        .min_w_0()
                        .h(rems(2.0))
                        .px(rems(0.625))
                        .flex()
                        .items_center()
                        .justify_between()
                        .border(rems(0.0625))
                        .border_color(theme.border)
                        .rounded_sm()
                        .child(
                            canvas(
                                |bounds, _, _| bounds,
                                move |bounds, _, _, cx| {
                                    measure.update(cx, |this, _| {
                                        if let Some(dialog) = &mut this.properties_dialog {
                                            dialog.run_bounds = Some(bounds);
                                        }
                                    })
                                },
                            )
                            .absolute()
                            .size_full(),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(cx.i18n().text(run_window_message(dialog.run_window))),
                        )
                        .child(
                            svg()
                                .path("icons/chevron-down.svg")
                                .size(rems(0.75))
                                .flex_none()
                                .text_color(theme.text_muted),
                        )
                        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                            picker.update(cx, |this, cx| {
                                if let Some(dialog) = &mut this.properties_dialog {
                                    if !dialog.saving {
                                        window.focus(&dialog.run_focus, cx);
                                        dialog.run_menu = dialog.run_menu.is_none().then_some(
                                            dialog
                                                .run_bounds
                                                .map(|bounds| point(bounds.left(), bounds.bottom()))
                                                .unwrap_or(event.position),
                                        );
                                        cx.notify();
                                    }
                                }
                            });
                        }),
                    rems(6.0),
                    cx,
                ));
    } else if dialog.error.is_none() {
        body = body.child(cx.i18n().text(Message::PreviewLoading));
    }
    let has_shortcut = dialog
        .properties
        .as_ref()
        .is_some_and(|properties| properties.shortcut.is_some());
    let mut property_tabs = None;
    if has_shortcut {
        let mut tabs = div()
            .flex()
            .flex_none()
            .items_center()
            .px(rems(1.25))
            .bg(theme.dialog_tabs)
            .gap(rems(0.25))
            .border_b(rems(0.0625))
            .border_color(theme.border);
        for (shortcut, message, icon) in [
            (false, Message::PropertiesGeneral, "icons/file-text.svg"),
            (true, Message::PropertiesShortcut, "icons/link.svg"),
        ] {
            let tab = handle.clone();
            let active = shortcut == dialog.shortcut_tab;
            tabs =
                tabs.child(
                    div()
                        .id(if shortcut {
                            "properties-shortcut-tab"
                        } else {
                            "properties-general-tab"
                        })
                        .px(rems(1.0))
                        .py(rems(0.75))
                        .text_size(rems(0.8125))
                        .font_weight(FontWeight::MEDIUM)
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(rems(0.5))
                        .when(!active, |tab| {
                            tab.hover(move |style| style.bg(theme.hover).text_color(theme.text))
                        })
                        .child(svg().path(icon).size(rems(0.875)).flex_none().text_color(
                            if active {
                                theme.accent
                            } else {
                                theme.text_muted
                            },
                        ))
                        .rounded_none()
                        .border_b(rems(0.0625))
                        .border_color(if active {
                            theme.accent
                        } else {
                            transparent_black()
                        })
                        .text_color(if active {
                            theme.accent
                        } else {
                            theme.text_muted
                        })
                        .child(cx.i18n().text(message))
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            tab.update(cx, |this, cx| {
                                this.select_properties_tab(shortcut, window, cx)
                            });
                        }),
                );
        }
        property_tabs = Some(tabs);
    }
    if dialog.saving {
        body = body.child(cx.i18n().text(Message::ShortcutSaving));
    }
    if let Some(error) = dialog.validation_error {
        body = body.child(div().text_color(theme.error).child(cx.i18n().text(error)));
    }
    if let Some(position) = dialog.run_menu {
        let mut menu = div()
            .occlude()
            .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
            .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_move(|_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .w(rems(17.0))
            .py(rems(0.25))
            .bg(theme.menu_surface)
            .border(rems(0.0625))
            .border_color(theme.menu_border)
            .shadow(crate::ui::style::rem_shadow(false));
        for mode in [1, 3, 7] {
            let choice = handle.clone();
            menu = menu.child(
                div()
                    .id(("shortcut-run-mode", mode as usize))
                    .px(rems(0.75))
                    .py(rems(0.5))
                    .hover(|row| row.bg(theme.menu_hover))
                    .child(cx.i18n().text(run_window_message(mode)))
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        choice.update(cx, |this, cx| {
                            if let Some(dialog) = &mut this.properties_dialog {
                                dialog.run_window = mode;
                                dialog.run_menu = None;
                                cx.notify();
                            }
                        })
                    }),
            );
        }
        let close = handle.clone();
        body = body.child(deferred(
            anchored()
                .position(position)
                .snap_to_window()
                .child(menu.on_mouse_down_out(move |_, _, cx| {
                    close.update(cx, |this, cx| {
                        if let Some(dialog) = &mut this.properties_dialog {
                            dialog.run_menu = None;
                            cx.notify();
                        }
                    })
                })),
        ));
    }
    body = body.when_some(dialog.error.as_ref(), |body, error| {
        body.child(
            div()
                .text_color(theme.error)
                .child(cx.i18n().text(Message::OperationFailed))
                .child(error.clone()),
        )
    });
    if let Some(tabs) = property_tabs {
        body = div()
            .flex()
            .flex_col()
            .child(tabs)
            .child(div().px(rems(1.0)).py(rems(1.25)).child(body));
    }
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
                "escape" => {
                    if let Some(dialog) = &mut this.properties_dialog {
                        if dialog.run_menu.take().is_some() {
                            cx.notify();
                            return;
                        }
                    }
                    this.close_file_properties(window, cx);
                }
                "enter" | "space"
                    if this
                        .properties_dialog
                        .as_ref()
                        .is_some_and(|dialog| dialog.run_focus.is_focused(window)) =>
                {
                    if let Some(dialog) = &mut this.properties_dialog {
                        if dialog.run_menu.take().is_none() {
                            dialog.run_menu = dialog
                                .run_bounds
                                .map(|bounds| point(bounds.left(), bounds.bottom()));
                        }
                        cx.notify();
                    }
                }
                "up" | "down"
                    if this
                        .properties_dialog
                        .as_ref()
                        .is_some_and(|dialog| dialog.run_menu.is_some()) =>
                {
                    if let Some(dialog) = &mut this.properties_dialog {
                        let modes = [1, 3, 7];
                        let index = modes
                            .iter()
                            .position(|mode| *mode == dialog.run_window)
                            .unwrap_or(0);
                        dialog.run_window =
                            modes[(index + if event.keystroke.key == "up" { 2 } else { 1 }) % 3];
                        cx.notify();
                    }
                }
                "enter" | "space"
                    if this
                        .properties_dialog
                        .as_ref()
                        .is_some_and(|dialog| dialog.focus.is_focused(window)) =>
                {
                    this.close_file_properties(window, cx)
                }
                "enter" => this.accept_properties(window, cx),
                "space"
                    if this.properties_dialog.as_ref().is_some_and(|dialog| {
                        dialog.focus.is_focused(window) || dialog.ok_focus.is_focused(window)
                    }) =>
                {
                    this.accept_properties(window, cx)
                }
                "tab" => {
                    if let Some(dialog) = &this.properties_dialog {
                        let mut focuses = vec![dialog.focus.clone()];
                        if dialog.shortcut_tab {
                            focuses.extend(
                                dialog
                                    .shortcut_inputs
                                    .iter()
                                    .map(|input| input.read(cx).focus_handle.clone()),
                            );
                            focuses.push(dialog.run_focus.clone());
                        }
                        focuses.push(dialog.ok_focus.clone());
                        let current = focuses
                            .iter()
                            .position(|focus| focus.is_focused(window))
                            .unwrap_or(0);
                        let next = if event.keystroke.modifiers.shift {
                            (current + focuses.len() - 1) % focuses.len()
                        } else {
                            (current + 1) % focuses.len()
                        };
                        window.focus(&focuses[next], cx);
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
                .when(has_shortcut, |modal| {
                    modal.body_padding(rems(0.0), rems(0.0))
                })
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
                            ok.update(cx, |this, cx| this.accept_properties(window, cx));
                        }),
                ),
        )
        .when(dialog.saving, |root| {
            root.child(
                div()
                    .absolute()
                    .inset_0()
                    .on_any_mouse_down(|_, _, cx| cx.stop_propagation()),
            )
        })
}

fn run_window_message(mode: i32) -> Message {
    match mode {
        3 => Message::ShortcutMaximized,
        7 => Message::ShortcutMinimized,
        _ => Message::ShortcutNormal,
    }
}
