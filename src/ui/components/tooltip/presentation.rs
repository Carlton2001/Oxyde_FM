use super::placement::PreviewTooltip;
use super::position::CursorTooltip;
use super::{DriveTooltip, DuplicateResultTooltip, Tooltip};
use crate::ui::format::{format_date, format_size};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::scale::rem_pixels;
use crate::ui::style::TNUM;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use unicode_segmentation::UnicodeSegmentation;

fn filename_line_ranges(name: &str, width: f32, x_for_index: impl Fn(usize) -> f32) -> Vec<std::ops::Range<usize>> {
    text_line_ranges(name, width, true, x_for_index)
}

fn text_line_ranges(name: &str, width: f32, preserve_extension: bool, x_for_index: impl Fn(usize) -> f32) -> Vec<std::ops::Range<usize>> {
    let extension = name.rfind('.').filter(|index| preserve_extension && *index > 0 && *index + 1 < name.len());
    let mut ranges = Vec::new();
    let mut start = 0;
    let mut end = 0;
    for next in name.grapheme_indices(true).map(|(index, grapheme)| index + grapheme.len())
        .filter(|next| !extension.is_some_and(|extension| *next > extension && *next < name.len())) {
        if end > start && x_for_index(next) - x_for_index(start) > width {
            ranges.push(start..end);
            start = end;
        }
        end = next;
    }
    ranges.push(start..name.len());
    ranges
}

fn location_line_ranges(text: &str, width: f32, x_for_index: impl Fn(usize) -> f32) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let mut end = start;
        let mut boundary = None;
        for (index, grapheme) in text[start..].grapheme_indices(true) {
            let next = start + index + grapheme.len();
            if end > start && x_for_index(next) - x_for_index(start) > width { break; }
            end = next;
            if matches!(grapheme, "\\" | "/" | " ") { boundary = Some(end); }
        }
        if end < text.len() { end = boundary.filter(|end| *end > start).unwrap_or(end); }
        ranges.push(start..end);
        start = end;
    }
    ranges
}

#[cfg(test)]
mod filename_tests {
    use super::filename_line_ranges;
    use unicode_segmentation::UnicodeSegmentation;

    #[test]
    fn search_location_wraps_long_paths_without_truncating_unicode_or_dotted_folders() {
        let path = r"\\serveur\Été👩‍💻\dossier.trèslongsansespaces\sous-dossier";
        let ranges = super::location_line_ranges(path, 8.0, |index| path[..index].graphemes(true).count() as f32);
        let lines: Vec<_> = ranges.into_iter().map(|range| &path[range]).collect();
        assert_eq!(lines.concat(), path);
        assert!(lines.iter().all(|line| line.graphemes(true).count() <= 8));
    }

    #[test]
    fn location_wrap_prefers_directory_boundaries() {
        let text = r"Emplacement : C:\Users\Biluf\Downloads\zed-1.21.0\crates\gpui\examples";
        let ranges = super::location_line_ranges(text, (text.len() - 7) as f32, |index| index as f32);
        let lines: Vec<_> = ranges.into_iter().map(|range| &text[range]).collect();
        assert_eq!(lines.concat(), text);
        assert_eq!(lines.last(), Some(&"examples"));
    }

    #[test]
    fn network_tooltips_show_only_available_network_properties() {
        let file = oxyde_lib::models::FileEntry {
            name: "PC".into(), path: "\\\\PC".into(),
            network_details: Some(oxyde_lib::models::file_entry::NetworkDetails { category: "Computer".into(), workgroup: "WORKGROUP".into(), location: "Bbox-445D52D4".into() }),
            ..Default::default()
        };
        for language in [crate::ui::i18n::Language::French, crate::ui::i18n::Language::English] {
            let i18n = crate::ui::i18n::I18n::for_language(language);
            let lines = super::network_tooltip_lines(&file, &i18n);
            assert_eq!(lines.len(), 3);
            assert!(lines[0].contains("Computer"));
            assert!(lines[1].contains("WORKGROUP"));
            assert!(lines[2].contains("Bbox-445D52D4"));
            assert!(lines.iter().all(|line| !line.starts_with("Type")));
            let mut device = file.clone(); device.network_details = None;
            assert_eq!(super::network_tooltip_lines(&device, &i18n).len(), 1);
        }
    }

    #[test]
    fn wraps_without_losing_text_or_splitting_the_extension() {
        for name in ["long_filename_without_spaces.pdf", "archive.tar.gz", "été👩‍💻_document.docx", ".gitignore", ""] {
            let ranges = filename_line_ranges(name, 8.0, |index| name[..index].graphemes(true).count() as f32);
            let lines: Vec<_> = ranges.iter().map(|range| &name[range.clone()]).collect();
            assert_eq!(lines.concat(), name);
            if let Some(dot) = name.rfind('.').filter(|index| *index > 0) {
                assert!(lines.last().unwrap().ends_with(&name[dot..]));
            }
            for range in ranges {
                assert!(range.start == 0 || name.grapheme_indices(true).any(|(index, _)| index == range.start));
            }
        }
    }

    #[test]
    fn wider_bubble_keeps_the_full_name_on_two_lines() {
        let name = "abcdefghijklmnop.pdf";
        let measure = |index| index as f32;
        assert!(filename_line_ranges(name, 8.0, measure).len() > 2);
        assert_eq!(filename_line_ranges(name, 10.0, measure).len(), 2);
    }
}

fn drive_usage(total: u64, free: u64) -> Option<(u64, f32)> {
    (total > 0).then(|| {
        let used = total.saturating_sub(free);
        (used, (used as f64 / total as f64) as f32)
    })
}

fn ring_sector(center: Point<Pixels>, radius: f32, inner: f32, start: f32, end: f32) -> Option<Path<Pixels>> {
    if end <= start { return None; }
    let steps = ((end - start) * 32.0).ceil().max(1.0) as usize;
    let vertex = |radius: f32, angle: f32| center + point(px(radius * angle.cos()), px(radius * angle.sin()));
    let mut path = PathBuilder::fill();
    path.move_to(vertex(radius, start));
    for index in 1..=steps { path.line_to(vertex(radius, start + (end - start) * index as f32 / steps as f32)); }
    for index in (0..=steps).rev() { path.line_to(vertex(inner, start + (end - start) * index as f32 / steps as f32)); }
    path.close();
    path.build().ok()
}

impl Render for DriveTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *OxydeTheme::global(cx);
        let usage = drive_usage(self.drive.total_bytes, self.drive.free_bytes);
        let mut body = div().flex().flex_col().items_center().gap(rems(0.5))
            .child(div().max_w(rems(18.0)).text_size(rems(0.8125)).child(self.name.clone()));
        if let Some((used, ratio)) = usage {
            let usage_color = theme.drive_usage_color(self.drive.total_bytes, self.drive.free_bytes);
            let chart = canvas(|bounds, _, _| bounds, move |_, bounds, window, _| {
                let center = bounds.center();
                let radius = bounds.size.width.as_f32() * 0.43;
                let inner = radius * 0.68;
                let start = -std::f32::consts::FRAC_PI_2;
                let split = start + std::f32::consts::TAU * ratio;
                let end = start + std::f32::consts::TAU;
                for (from, to, color) in [(start, split, usage_color), (split, end, theme.border)] {
                    if let Some(path) = ring_sector(center, radius, inner, from, to) { window.paint_path(path, color); }
                }
            }).absolute().inset_0();
            let mut args = FluentArgs::new();
            args.set("used", format_size(used, false, false, cx.i18n().language()));
            args.set("total", format_size(self.drive.total_bytes, false, false, cx.i18n().language()));
            body = body.child(div().relative().size(rems(4.75)).flex().items_center().justify_center()
                .child(chart).child(div().text_size(rems(0.875)).font_weight(FontWeight::SEMIBOLD).child(format!("{}%", (ratio * 100.0).round() as u32))))
                .child(div().text_size(rems(0.6875)).text_color(theme.text_muted).child(cx.i18n().text_with_args(Message::DriveTooltipUsage, &args)));
        } else {
            body = body.child(div().text_size(rems(0.6875)).text_color(theme.text_muted).child(self.drive.path.clone()));
        }
        CursorTooltip::new(div().bg(theme.surface).text_color(theme.text)
            .font_family("Inter").font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .border(rems(0.0625)).border_color(theme.border).rounded_md().shadow(tooltip_shadow())
            .px(rems(0.75)).py(rems(0.5)).child(body).into_any_element())
    }
}

#[cfg(test)]
mod drive_usage_tests {
    #[test]
    fn drive_usage_handles_empty_full_unknown_and_inconsistent_capacity() {
        assert_eq!(super::drive_usage(0, 0), None);
        assert_eq!(super::drive_usage(100, 100), Some((0, 0.0)));
        assert_eq!(super::drive_usage(100, 0), Some((100, 1.0)));
        assert_eq!(super::drive_usage(100, 200), Some((0, 0.0)));
        let (used, ratio) = super::drive_usage(100, 21).unwrap();
        assert_eq!(used, 79); assert!((ratio - 0.79).abs() < 0.0001);
    }
}

impl Render for DuplicateResultTooltip {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *OxydeTheme::global(cx);
        let width = rem_pixels(rems(26.5)).min((window.viewport_size().width - rem_pixels(rems(2.0))).max(px(1.0)));
        let content_width = (width - rem_pixels(rems(1.625))).as_f32().max(1.0);
        let wrap = |text: SharedString, size: Rems, weight: FontWeight, location: bool| {
            let mut font = window.text_style().font();
            font.family = "Inter".into(); font.weight = weight;
            let line = window.text_system().shape_line(text.clone(), rem_pixels(size), &[TextRun {
                len: text.len(), font, color: theme.text, background_color: None,
                underline: None, strikethrough: None,
            }], None);
            let ranges = if location {
                location_line_ranges(&text, content_width, |index| line.x_for_index(index).as_f32())
            } else {
                text_line_ranges(&text, content_width, false, |index| line.x_for_index(index).as_f32())
            };
            ranges.into_iter().map(|range| text[range].to_string()).collect::<Vec<_>>()
        };
        let mut args = FluentArgs::new(); args.set("value", self.location.as_str());
        let location = cx.i18n().text_with_args(Message::TooltipLocation, &args);
        div().w(width).max_w(width).px(rems(0.75)).py(rems(0.5))
            .bg(theme.surface).border(rems(0.0625)).border_color(theme.border).rounded(rems(0.375))
            .shadow(tooltip_shadow()).font_family("Inter")
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT).line_height(relative(1.25))
            .flex().flex_col().gap(rems(0.5))
            .child(div().text_size(rems(0.75)).font_weight(FontWeight::MEDIUM).text_color(theme.text)
                .children(wrap(self.name.clone(), rems(0.75), FontWeight::MEDIUM, false).into_iter().map(|line| div().whitespace_nowrap().child(line))))
            .child(div().text_size(rems(0.6875)).text_color(theme.text_muted)
                .children(wrap(location, rems(0.6875), crate::ui::style::DEFAULT_FONT_WEIGHT, true).into_iter().map(|line| div().whitespace_nowrap().child(line))))
            .child(div().flex().items_center().gap(rems(0.375)).text_size(rems(0.75)).text_color(theme.accent)
                .child(cx.i18n().text(Message::DuplicateGoToFolder))
                .child(svg().path("icons/arrow-right.svg").size(rems(0.875)).text_color(theme.accent)))
    }
}

impl Render for Tooltip {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *OxydeTheme::global(cx);
        let chrome = rem_pixels(rems(1.625));
        let media_width = self.preview_view.clone().and_then(|preview| {
            preview.update(cx, |preview, cx| {
                if matches!(preview.data, Some(super::PreviewData::Text(_))) {
                    Some(rem_pixels(rems(21.875)) + chrome)
                } else {
                    preview.loaded_image(window, cx).map(|image|
                        preview.preferred_size(&image, rem_pixels(rems(48.375))).width + chrome)
                }
            })
        });
        let width_limit = media_width.map(|width| {
            if self.meta.is_some() { width.max(rem_pixels(rems(40.0))) } else { width }
        }).unwrap_or_else(|| rem_pixels(rems(40.0)));
        let maximum_width = if self.native_presentation {
            width_limit
        } else {
            width_limit.min((window.viewport_size().width - rem_pixels(rems(2.0))).max(px(1.0)))
        };
        let mut tooltip_width = maximum_width;
        let title_lines: Vec<SharedString> = if self.file_info.is_some() {
            let mut font = window.text_style().font();
            font.family = "Inter".into();
            font.weight = FontWeight::MEDIUM;
            font.features = TNUM.clone();
            let line = window.text_system().shape_line(self.title.clone(), rem_pixels(rems(0.75)), &[TextRun {
                len: self.title.len(), font: font.clone(), color: theme.accent, background_color: None, underline: None, strikethrough: None,
            }], None);
            // Extra metadata may widen a ready preview's frame.
            font.weight = crate::ui::style::DEFAULT_FONT_WEIGHT;
            let metadata_width = self.meta.as_ref().map(|meta| {
                meta.lines().map(|text| {
                    window.text_system().shape_line(text.to_owned().into(), rem_pixels(rems(0.6875)), &[TextRun {
                        len: text.len(), font: font.clone(), color: theme.text_muted,
                        background_color: None, underline: None, strikethrough: None,
                    }], None).width
                }).fold(px(0.0), |width, line| width.max(line))
            }).unwrap_or(px(0.0));
            let preferred_width = line.width.max(metadata_width + px(1.0));
            tooltip_width = if let Some(width) = media_width {
                width.max(metadata_width + px(1.0) + chrome).min(maximum_width)
            } else {
                (preferred_width.max(rem_pixels(rems(14.0))) + chrome).min(maximum_width)
            };
            let wrap = |width: Pixels| filename_line_ranges(&self.title, (width - chrome).as_f32().max(1.0), |index| line.x_for_index(index).as_f32());
            if wrap(tooltip_width).len() > 2 {
                if wrap(maximum_width).len() <= 2 {
                    let mut lower = tooltip_width.as_f32();
                    let mut upper = maximum_width.as_f32();
                    for _ in 0..14 {
                        let middle = (lower + upper) / 2.0;
                        if wrap(px(middle)).len() <= 2 { upper = middle; } else { lower = middle; }
                    }
                    tooltip_width = px(upper.ceil()).min(maximum_width);
                } else {
                    tooltip_width = maximum_width;
                }
            }
            wrap(tooltip_width).into_iter().map(|range| self.title[range].to_string().into()).collect()
        } else if self.wrap_title {
            let mut font = window.text_style().font();
            font.family = "Inter".into();
            font.weight = FontWeight::MEDIUM;
            font.features = TNUM.clone();
            let line = window.text_system().shape_line(self.title.clone(), rem_pixels(rems(0.75)), &[TextRun {
                len: self.title.len(), font, color: theme.text, background_color: None,
                underline: None, strikethrough: None,
            }], None);
            tooltip_width = (line.width + chrome + px(1.0)).min(maximum_width);
            location_line_ranges(&self.title, (tooltip_width - chrome - px(1.0)).as_f32().max(1.0), |index| line.x_for_index(index).as_f32())
                .into_iter().map(|range| self.title[range].to_string().into()).collect()
        } else if let Some(meta) = &self.meta {
            // Resolve the width before laying out percentage-width flex rows.
            // Standalone tooltips must not inherit a host row's line height.
            let measure = |text: SharedString, font_size: Rems, weight: FontWeight| {
                let mut font = window.text_style().font();
                font.family = "Inter".into(); font.weight = weight; font.features = TNUM.clone();
                window.text_system().shape_line(text.clone(), rem_pixels(font_size), &[TextRun {
                    len: text.len(), font, color: theme.text, background_color: None, underline: None, strikethrough: None,
                }], None).width
            };
            let title_width = measure(self.title.clone(), rems(0.75), FontWeight::MEDIUM);
            let meta_width = meta.lines().map(|line| measure(line.to_string().into(), rems(0.6875), crate::ui::style::DEFAULT_FONT_WEIGHT))
                .fold(px(0.0), |width, line| width.max(line));
            tooltip_width = (title_width.max(meta_width) + chrome).min(maximum_width);
            vec![self.title.clone()]
        } else {
            vec![self.title.clone()]
        };

        let mut box_div = div()
            .max_w(tooltip_width)
            .when(self.file_info.is_some() || self.wrap_title || self.meta.is_some(), |tooltip| tooltip.w(tooltip_width))
            .border(rems(0.0625))
            .border_color(gpui::transparent_black())
            .when(self.preview_view.is_none(), |this| this
                .bg(theme.surface)
                .border_color(theme.border)
                .rounded_md()
                .shadow(tooltip_shadow()))
            .font_family("Inter")
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .line_height(relative(1.25))
            .py_2()
            .px_3()
            .flex()
            .flex_col()
            .gap(rems(0.25))
            .child(
                div()
                    .flex_none()
                    .w_full()
                    .min_w_0()
                    .text_color(if self.file_info.is_some() {
                        theme.accent
                    } else {
                        theme.text
                    })
                    .text_size(rems(0.75))
                    .font_weight(FontWeight::MEDIUM)
                    .flex()
                    .items_center()
                    .gap(rems(0.75))
                    .child(div().flex_1().min_w_0().flex().flex_col()
                        .children(title_lines.into_iter().map(|line| div()
                            .when(self.file_info.is_some() || self.meta.is_none(), |title| title.whitespace_nowrap())
                            .child(line))))
                    .when_some(self.shortcut.clone(), |row, shortcut| {
                        row.child(div().whitespace_nowrap().text_color(theme.text_muted)
                            .text_size(rems(0.6875)).child(shortcut))
                    }),
            )
            .when_some(self.meta.clone(), |this, meta| {
                this.child(
                    div()
                        .flex_none()
                        .max_w((tooltip_width - chrome).max(px(1.0)))
                        .text_color(theme.text_muted)
                        .text_size(rems(0.6875))
                        .child(meta),
                )
            })
            .when_some(self.file_info.as_ref(), |this, info| {
                if let Some(file) = &info.network {
                    return this.child(div().flex().flex_col().text_size(rems(0.6875)).text_color(theme.text_muted).children(network_tooltip_lines(file, cx.i18n()).into_iter().map(|line| div().child(line))));
                }
                let i18n = cx.i18n();
                let mut args = FluentArgs::new();
                let type_label = match info.ext.as_str() {
                    "Réseau" => i18n.text(Message::SidebarNetwork).to_string(),
                    "Dossier" => i18n.text(Message::FileTypeFolder).to_string(),
                    "Fichier" => i18n.text(Message::FileTypeFile).to_string(),
                    "Symlink" => i18n.text(Message::FileTypeSymlink).to_string(),
                    "Junction" => i18n.text(Message::FileTypeJunction).to_string(),
                    _ => info.ext.clone(),
                };
                args.set("value", type_label);
                let file_type = i18n.text_with_args(Message::TooltipType, &args);
                args.set(
                    "value",
                    format_size(info.size.0, info.size.1, info.size.2, i18n.language()),
                );
                let file_size = i18n.text_with_args(Message::TooltipSize, &args);
                args.set("value", format_date(info.date, crate::ui::format::date_format(cx)));
                let modified = i18n.text_with_args(Message::TooltipModified, &args);
                let location_lines = info.location.as_ref().map(|location| {
                    args.set("value", location.clone());
                    let text = i18n.text_with_args(Message::TooltipLocation, &args);
                    let mut font = window.text_style().font();
                    font.family = "Inter".into();
                    font.weight = crate::ui::style::DEFAULT_FONT_WEIGHT;
                    font.features = TNUM.clone();
                    let line = window.text_system().shape_line(text.clone(), rem_pixels(rems(0.6875)), &[TextRun {
                        len: text.len(), font, color: theme.text_muted, background_color: None,
                        underline: None, strikethrough: None,
                    }], None);
                    location_line_ranges(&text, (tooltip_width - chrome - px(1.0)).as_f32().max(1.0), |index| line.x_for_index(index).as_f32())
                        .into_iter().map(|range| text[range].to_string()).collect::<Vec<_>>()
                });
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .text_size(rems(0.6875))
                        .text_color(theme.text_muted)
                        .child(file_type)
                        .when(info.ext != "Réseau", |tooltip| {
                            tooltip.child(file_size).child(modified)
                        })
                        .when_some(location_lines, |tooltip, lines| tooltip.children(lines.into_iter().map(|line| div().whitespace_nowrap().child(line)))),
                )
            });

        box_div.text_style().font_features = Some(TNUM.clone());

        if self.native_presentation {
            if let Some(preview) = self.preview_view.clone() {
                let limit = size((tooltip_width - chrome).max(px(1.0)), rem_pixels(rems(24.0)));
                preview.update(cx, |preview, _| preview.maximum_size = Some(limit));
                return div().w(tooltip_width).flex().flex_col()
                    .bg(theme.surface).border(rems(0.0625)).border_color(theme.border)
                    .rounded_md().shadow(tooltip_shadow())
                    .font_family("Inter").font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
                    .child(div().px_3().pt_2().max_h(limit.height).overflow_hidden().child(preview))
                    .child(box_div).into_any_element();
            }
        }

        // Décalage pour éviter que le tooltip n'apparaisse exactement sous le pointeur de la souris
        if let Some(preview) = self.preview_view.clone() {
            PreviewTooltip::new(box_div.into_any_element(), preview).into_any_element()
        } else {
            if let Some(bounds) = self.fixed_bounds.clone() {
                CursorTooltip::anchored(box_div.into_any_element(), bounds).into_any_element()
            } else {
                CursorTooltip::new(box_div.into_any_element()).into_any_element()
            }
        }
    }
}

fn network_tooltip_lines(file: &oxyde_lib::models::FileEntry, i18n: &crate::ui::i18n::I18n) -> Vec<SharedString> {
    let details = file.network_details.as_ref();
    let category = details.map(|d| d.category.clone()).filter(|value| !value.is_empty()).unwrap_or_else(|| i18n.text(if oxyde_lib::commands::io::is_network_server(&file.path) { Message::NetworkComputer } else { Message::NetworkOtherDevices }).to_string());
    [(Message::TooltipNetworkCategory, category), (Message::TooltipNetworkWorkgroup, details.map(|d| d.workgroup.clone()).unwrap_or_default()), (Message::TooltipNetworkLocation, details.map(|d| d.location.clone()).unwrap_or_default())]
        .into_iter().filter(|(_, value)| !value.is_empty()).map(|(message, value)| {
            let mut args = FluentArgs::new(); args.set("value", value); i18n.text_with_args(message, &args)
        }).collect()
}

// Keep tooltip shadows softer without changing shadows on menus or dialogs.
pub(super) fn tooltip_shadow() -> Vec<BoxShadow> {
    let mut shadows = crate::ui::style::rem_shadow(true);
    for shadow in &mut shadows {
        shadow.color = shadow.color.opacity(0.5);
    }
    shadows
}
