use crate::ui::components::tooltip::NativeTooltipExt;
use super::PropertiesWindow;
use super::PropertyPresentation;
use crate::ui::components::icons::{get_icon_type, render_file_icon};
use crate::ui::format::{date_format, format_date, format_size};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use oxyde_lib::models::FileProperties;

pub(super) fn label_width(
    presentation: Option<&PropertyPresentation<'_>>,
    theme: OxydeTheme,
    window: &mut Window,
    cx: &mut Context<PropertiesWindow>,
) -> f32 {
    let mut font = window.text_style().font();
    font.family = "Inter".into();
    font.weight = crate::ui::style::DEFAULT_FONT_WEIGHT;
    [
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
    .chain(presentation
        .filter(|p| matches!(p.location_label, Message::TrashOriginalLocation))
        .map(|_| Message::TrashOriginalLocation))
    .chain(presentation.and_then(|p| p.deleted_time).map(|_| Message::TrashDeletedAt))
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
    .ceil()
}

pub(super) fn divider(theme: OxydeTheme) -> Div {
    div()
        .h(rems(0.0625))
        .w_full()
        .bg(theme.border)
        .my(rems(0.75))
}

pub(super) fn render(
    mut body: Div,
    view: &PropertiesWindow,
    p: &FileProperties,
    presentation: &PropertyPresentation<'_>,
    label_width: f32,
    theme: OxydeTheme,
    cx: &mut Context<PropertiesWindow>,
) -> Div {
    let dialog = view.properties_dialog.as_ref().unwrap();
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
    let icon = get_icon_type(presentation.name, p.is_dir, Some(&p.path), false, false);
    let graphic = if let Some(drive) = &dialog.drive {
        if cfg!(target_os = "windows") && view.use_system_icons(cx) {
            render_file_icon(
                p.path.clone(),
                icon,
                rems(3.0), &theme,
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
            rems(3.0), &theme,
            cfg!(target_os = "windows") && view.use_system_icons(cx),
            true,
        )
    };
    let name = if let Some(drive) = &dialog.drive {
        format!("{} ({})", drive.label, drive.path.trim_end_matches('\\'))
    } else {
        presentation.name.to_owned()
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
                    .native_tooltip(crate::ui::components::tooltip::Tooltip::text(
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
        std::path::Path::new(presentation.name)
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
            body = body.child(divider(theme)).child(
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
                cx.i18n().text(presentation.location_label),
                presentation.location.to_owned(),
            ))
            .child(row(
                cx.i18n().text(Message::ColumnSize),
                if dialog.calculating {
                    cx.i18n().text(Message::PropertiesCalculatingSize).to_string()
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
    body = body.child(divider(theme));
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
    if let Some(timestamp) = presentation.deleted_time {
        body = body.child(row(
            cx.i18n().text(Message::TrashDeletedAt),
            format_date(timestamp, date_format(cx)),
        ));
    }
    body
}
