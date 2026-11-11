use crate::ui::components::tooltip::NativeTooltipExt;
use super::PropertiesWindow;
use super::PropertyPresentation;
use super::section;
use crate::ui::components::icons::{get_icon_type, render_file_icon};
use crate::ui::format::{date_format, format_date, format_size};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use oxyde_lib::models::FileProperties;

pub(super) fn label_width(
    presentation: Option<&PropertyPresentation<'_>>,
    has_file_system: bool,
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
        Message::PropertiesTotalSize,
        Message::PropertiesUsed,
        Message::TrashAvailableSpace,
        Message::PropertiesCapacity,
        Message::PropertiesContains,
        Message::PropertiesCreated,
        Message::ColumnModified,
        Message::PropertiesAccessed,
        Message::PropertiesLinkTarget,
    ]
    .into_iter()
    .chain(presentation
        .filter(|p| matches!(p.location_label, Message::TrashOriginalLocation))
        .map(|_| Message::TrashOriginalLocation))
    .chain(presentation.and_then(|p| p.deleted_time).map(|_| Message::TrashDeletedAt))
    .chain(has_file_system.then_some(Message::PropertiesFileSystem))
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


/// The kind of disk, as a colored badge.
fn media_badge(media: &str, theme: OxydeTheme) -> Div {
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
        .child(media.to_owned())
}

/// Used space as a bar, then what is used and free, and the capacity.
fn space_card(drive: &oxyde_lib::models::DriveInfo, label_width: Pixels, compact: bool, theme: OxydeTheme, cx: &App) -> Div {
    let language = cx.i18n().language();
    let used = drive.total_bytes.saturating_sub(drive.free_bytes);
    let fill = theme.drive_usage_color(drive.total_bytes, drive.free_bytes);
    let legend = |color: Hsla, label: Message, size: u64| {
        div()
            .flex()
            .items_center()
            .gap(rems(0.375))
            .min_w_0()
            .child(div().size(rems(0.5)).rounded_full().flex_none().bg(color))
            .child(div().text_color(theme.text_muted).whitespace_nowrap().child(cx.i18n().text(label)))
            .child(div().whitespace_nowrap().child(format_size(size, false, false, language)))
    };
    let mut card = section::card(theme);
    if drive.total_bytes > 0 {
        let ratio = (used as f64 / drive.total_bytes as f64) as f32;
        card = card
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(rems(0.75))
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .h(rems(0.5))
                            .rounded_full()
                            .bg(theme.border)
                            .overflow_hidden()
                            .child(
                                crate::ui::elements::rounded_fill::rounded_fill(ratio, rems(0.25), fill)
                                    .absolute()
                                    .left_0()
                                    .top_0()
                                    .size_full(),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .font_weight(FontWeight::MEDIUM)
                            .child(format!("{}%", (ratio * 100.0).round() as u32)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_x(rems(1.25))
                    .gap_y(rems(0.25))
                    .child(legend(fill, Message::PropertiesUsed, used))
                    .child(legend(theme.border, Message::TrashAvailableSpace, drive.free_bytes)),
            )
            .child(section::rule(theme));
    }
    card.child(section::row(
        cx.i18n().text(Message::PropertiesCapacity),
        format_size(drive.total_bytes, false, false, language),
        label_width,
        compact,
        theme,
    ))
}

pub(super) fn render(
    body: Div,
    view: &PropertiesWindow,
    p: &FileProperties,
    presentation: &PropertyPresentation<'_>,
    label_width: f32,
    compact: bool,
    theme: OxydeTheme,
    cx: &mut Context<PropertiesWindow>,
) -> Div {
    let dialog = view.properties_dialog.as_ref().unwrap();
    let label_width = px(label_width);
    let row = |label, value: String| section::row(label, value, label_width, compact, theme);
    #[cfg(not(target_os = "windows"))]
    let icon = if p.link_target.is_some() {
        crate::ui::components::icons::link_icon(p.is_dir, p.is_broken_link)
    } else {
        get_icon_type(presentation.name, p.is_dir, Some(&p.path), false, false)
    };
    #[cfg(target_os = "windows")]
    let icon = get_icon_type(presentation.name, p.is_dir, Some(&p.path), false, false);
    let system_icons = cfg!(target_os = "windows") && view.use_system_icons(cx);
    let graphic = match &dialog.drive {
        Some(drive) if !system_icons => svg()
            .path(crate::ui::components::icons::drive_icon_path(drive))
            .size(rems(3.0))
            .text_color(theme.text)
            .into_any_element(),
        _ => render_file_icon(p.path.clone(), icon, rems(3.0), &theme, system_icons, true),
    };
    let name = if let Some(drive) = &dialog.drive {
        if cfg!(target_os = "windows") {
            format!("{} ({})", drive.label, drive.path.trim_end_matches('\\'))
        } else {
            // Linux file managers name volumes alone; the location row below shows the path.
            crate::ui::format::drive_name(drive, cx.i18n())
        }
    } else {
        presentation.name.to_owned()
    };
    let copy_name = name.clone();
    let title = div()
        .id("properties-name")
        .min_w_0()
        .native_tooltip(crate::ui::components::tooltip::Tooltip::text(
            cx.i18n().text(Message::PropertiesCopyNameDoubleClick),
        ))
        .on_mouse_down(MouseButton::Left, move |event, _, cx| {
            if event.click_count == 2 {
                cx.write_to_clipboard(ClipboardItem::new_string(copy_name.clone()));
            }
            cx.stop_propagation();
        })
        .child(name);
    let file_type = if p.is_broken_link {
        cx.i18n().text(Message::FileTypeBrokenLink).to_string()
    } else if p.link_target.is_some() {
        cx.i18n().text(Message::FileTypeSymlink).to_string()
    } else if let Some(mime) = &p.mime_type {
        // As Nautilus: « image PNG (image/png) ». Oxyde names folders itself, since
        // the database often lacks that translation.
        let description = if p.is_dir {
            Some(cx.i18n().text(Message::FileTypeFolder).to_string())
        } else {
            mime.description.clone()
        };
        description.map_or_else(|| mime.name.clone(), |description| format!("{description} ({})", mime.name))
    } else if p.is_dir {
        cx.i18n().text(Message::FileTypeFolder).to_string()
    } else {
        std::path::Path::new(presentation.name)
            .extension()
            .map(|extension| extension.to_string_lossy().to_uppercase())
            .unwrap_or_else(|| cx.i18n().text(Message::FileTypeFile).to_string())
    };
    let size = if dialog.calculating {
        cx.i18n().text(Message::PropertiesCalculatingSize).to_string()
    } else {
        format_size(p.size, p.is_dir, p.is_calculated, cx.i18n().language())
    };
    let subtitle = match &dialog.drive {
        Some(drive) => {
            let kind = cx.i18n().text(Message::PropertiesDrive).to_string();
            let text = match &drive.file_system {
                Some(file_system) => format!("{kind} · {file_system}"),
                None => kind,
            };
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(rems(0.5))
                .child(text)
                .when_some(drive.media_type.as_ref().filter(|media| !media.is_empty()), |line, media| {
                    line.child(media_badge(media, theme))
                })
        }
        None => div().child(format!("{file_type} · {size}")),
    };
    let mut body = body.gap(rems(1.25)).child(section::header(graphic, title, subtitle, theme));
    if let Some(drive) = &dialog.drive {
        body = body.child(section::section(
            cx.i18n().text(Message::PropertiesSectionSpace),
            "icons/hard-drive.svg",
            space_card(drive, label_width, compact, theme, cx),
            theme,
        ));
        if !cfg!(target_os = "windows") {
            body = body.child(section::section(
                cx.i18n().text(Message::PropertiesSectionDetails),
                "icons/info.svg",
                section::card(theme).child(row(cx.i18n().text(Message::ColumnLocation), drive.path.clone())),
                theme,
            ));
        }
    } else {
        let mut details = section::card(theme);
        if let Some(target) = &p.link_target {
            details = details.child(row(cx.i18n().text(Message::PropertiesLinkTarget), target.clone()));
        }
        details = details.child(row(cx.i18n().text(presentation.location_label), super::location_text(presentation.location, cx.i18n())));
        if let (Some(files), Some(folders)) = (p.files_count, p.folders_count) {
            let mut args = FluentArgs::new();
            args.set("files", crate::ui::format::format_number(files, cx.i18n().language()));
            args.set("folders", crate::ui::format::format_number(folders, cx.i18n().language()));
            details = details.child(row(
                cx.i18n().text(Message::PropertiesContains),
                cx.i18n().text_with_args(Message::PropertiesCounts, &args).to_string(),
            ));
        }
        body = body.child(section::section(
            cx.i18n().text(Message::PropertiesSectionDetails),
            "icons/info.svg",
            details,
            theme,
        ));
    }
    let mut dates = section::card(theme);
    for (label, timestamp) in [
        (Message::PropertiesCreated, p.created),
        (Message::ColumnModified, p.modified),
        (Message::PropertiesAccessed, p.accessed),
    ] {
        dates = dates.child(row(cx.i18n().text(label), format_date(timestamp, date_format(cx))));
    }
    if let Some(timestamp) = presentation.deleted_time {
        dates = dates.child(row(cx.i18n().text(Message::TrashDeletedAt), format_date(timestamp, date_format(cx))));
    }
    body.child(section::section(cx.i18n().text(Message::PropertiesSectionDates), "icons/calendar.svg", dates, theme))
}

pub(super) fn divider(theme: OxydeTheme) -> Div {
    div().h(rems(0.0625)).w_full().bg(theme.border).my(rems(0.75))
}
