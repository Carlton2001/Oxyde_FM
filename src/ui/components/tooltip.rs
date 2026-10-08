use crate::ui::components::icons::{get_preview_type, PreviewType};
use crate::ui::format::{format_date, format_size};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::scale::rem_pixels;
use crate::ui::style::TNUM;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::sync::Arc;
use std::time::Duration;
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

/// Un composant de bulle d'aide (Tooltip) dans le style Oxyde.
pub struct Tooltip {
    title: SharedString,
    meta: Option<SharedString>,
    shortcut: Option<SharedString>,
    file_info: Option<FileInfo>,
    preview_view: Option<Entity<PreviewTask>>,
}

struct FileInfo {
    location: Option<String>,
    network: Option<oxyde_lib::models::FileEntry>,
    ext: String,
    size: (u64, bool, bool),
    date: u64,
}

struct TooltipPreviewAsset;

impl Asset for TooltipPreviewAsset {
    type Source = SharedString;
    type Output = Result<Arc<RenderImage>, ImageCacheError>;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl std::future::Future<Output = Self::Output> + Send + 'static {
        let path = source.to_string();
        let task = cx.background_executor().spawn(async move {
            let result = ::oxyde_lib::platform::platform().pdf_thumbnail(path);
            result.map_err(|error| {
                log::warn!("Could not load tooltip media preview: {error}");
                ImageCacheError::Asset(error.to_string().into())
            })
        });
        async move {
            let image = task.await?;
            Ok(Arc::new(RenderImage::new(vec![image::Frame::new(image)])))
        }
    }
}

#[derive(Clone)]
enum PreviewData {
    Image(Arc<RenderImage>),
    Text(String),
}

impl Tooltip {
    pub fn for_drive(name: String, drive: oxyde_lib::models::DriveInfo) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |_, cx| cx.new(|_| DriveTooltip { name: name.clone().into(), drive: drive.clone() }).into()
    }
    pub fn for_delete(handle: Entity<crate::ui::app::OxydeApp>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |window, cx| {
            let title = cx.i18n().text(if window.modifiers().shift { Message::TrashDeletePermanent } else { Message::Delete });
            cx.new(|cx| {
                cx.observe(&handle, |tooltip: &mut Self, app, cx| {
                    tooltip.title = cx.i18n().text(if app.read(cx).shift_delete { Message::TrashDeletePermanent } else { Message::Delete });
                    tooltip.shortcut = Some(if app.read(cx).shift_delete { "Shift+Del" } else { "Del" }.into());
                    cx.notify();
                }).detach();
                Self { title, meta: None, shortcut: Some(if window.modifiers().shift { "Shift+Del" } else { "Del" }.into()), file_info: None, preview_view: None }
            }).into()
        }
    }
    pub fn for_trash(file: oxyde_lib::models::FileEntry, ext: String, previews: bool) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |_, cx| {
            let view = Self::new_for_file(file.name.clone(), file.path.clone(), ext.clone(),
                (file.size, file.is_dir, file.is_calculated), file.modified, previews, None, cx);
            let location = file.original_path.as_deref().and_then(|path| std::path::Path::new(path).parent())
                .map(|path| path.to_string_lossy().into_owned()).unwrap_or_default();
            let date = file.deleted_time.and_then(|date| u64::try_from(date).ok()).unwrap_or(0);
            let metadata = format!("{} : {}\n{} : {}", cx.i18n().text(Message::TrashOriginalLocation), location,
                cx.i18n().text(Message::TrashDeletedAt), format_date(date, crate::ui::format::date_format(cx)));
            view.update(cx, |tooltip, _| tooltip.meta = Some(metadata.into()));
            view.into()
        }
    }
    pub fn for_network(file: oxyde_lib::models::FileEntry) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |_, cx| cx.new(|_| Self {
            title: file.name.clone().into(), meta: None, shortcut: None,
            file_info: Some(FileInfo { location: None, network: Some(file.clone()), ext: "Réseau".into(), size: (0, false, false), date: 0 }),
            preview_view: None,
        }).into()
    }
    /// Keep action labels and their optional keyboard shortcut on one line.
    pub fn with_shortcut(
        title: impl Into<SharedString>,
        shortcut: Option<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let title = title.into();
        move |_, cx| {
            cx.new(|_| Self {
                title: title.clone(), meta: None, shortcut: shortcut.clone(),
                file_info: None, preview_view: None,
            }).into()
        }
    }
    /// Crée un constructeur de tooltip simple affichant un texte.
    pub fn text(
        title: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let title = title.into();
        move |_, cx| {
            cx.new(|_| Self {
                title: title.clone(),
                meta: None,
                shortcut: None,
                file_info: None,
                preview_view: None,
            })
            .into()
        }
    }

    /// Crée un constructeur de tooltip avec un texte principal et un sous-texte (méta/raccourci).
    pub fn with_meta(
        title: impl Into<SharedString>,
        meta: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let title = title.into();
        let meta = meta.into();
        move |_, cx| {
            cx.new(|_| Self {
                title: title.clone(),
                meta: Some(meta.clone()),
                shortcut: None,
                file_info: None,
                preview_view: None,
            })
            .into()
        }
    }

    /// Crée un constructeur de tooltip pour un fichier sans aperçu média (nom + métadonnées uniquement).
    pub fn for_file_no_preview(
        name: String,
        ext: String,
        size: (u64, bool, bool),
        date: u64,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |_, cx| {
            cx.new(|_| Self {
                title: SharedString::from(name.clone()),
                meta: None,
                shortcut: None,
                file_info: Some(FileInfo {
                    location: None,
                    network: None,
                    ext: ext.clone(),
                    size: size.clone(),
                    date: date.clone(),
                }),
                preview_view: None,
            })
            .into()
        }
    }

    /// Crée un constructeur de tooltip complet pour un fichier avec preview possible.
    pub fn for_file_full(
        name: String,
        path: String,
        ext: String,
        size: (u64, bool, bool),
        date: u64,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let path_clone = path.clone();
        move |_, cx| {
            Self::new_for_file_full(
                name.clone(),
                path_clone.clone(),
                ext.clone(),
                size.clone(),
                date.clone(),
                cx,
            )
            .into()
        }
    }

    pub fn new_for_file_full(
        name: String,
        path: String,
        ext: String,
        size: (u64, bool, bool),
        date: u64,
        cx: &mut App,
    ) -> Entity<Self> {
        Self::new_for_file(name, path, ext, size, date, true, None, cx)
    }

    pub fn for_search_result(
        file: oxyde_lib::models::FileEntry,
        ext: String,
        previews: bool,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |_, cx| {
            let location = std::path::Path::new(&file.path).parent()
                .map(|parent| parent.to_string_lossy().into_owned());
            Self::new_for_file(file.name.clone(), file.path.clone(), ext.clone(),
                (file.size, file.is_dir, file.is_calculated), file.modified, previews, location, cx).into()
        }
    }

    pub fn duplicate_result(name: String, location: String, cx: &mut App) -> AnyView {
        cx.new(|_| DuplicateResultTooltip { name: name.into(), location }).into()
    }

    #[allow(clippy::too_many_arguments)]
    fn new_for_file(
        name: String, path: String, ext: String, size: (u64, bool, bool),
        date: u64, previews: bool, location: Option<String>, cx: &mut App,
    ) -> Entity<Self> {
        let preview_type = get_preview_type(&name);
        cx.new(|cx| {
            let preview_view = if previews && preview_type != PreviewType::None {
                Some(cx.new(|cx| PreviewTask::new(path, preview_type, cx)))
            } else {
                None
            };
            if let Some(preview) = &preview_view {
                cx.observe(preview, |_, _, cx| cx.notify()).detach();
            }

            Self {
                title: SharedString::from(name),
                meta: None,
                shortcut: None,
                file_info: Some(FileInfo { location, network: None, ext, size, date }),
                preview_view,
            }
        })
    }
}

struct DriveTooltip {
    name: SharedString,
    drive: oxyde_lib::models::DriveInfo,
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
            let chart = canvas(|bounds, _, _| bounds, move |_, bounds, window, _| {
                let center = bounds.center();
                let radius = bounds.size.width.as_f32() * 0.43;
                let inner = radius * 0.68;
                let start = -std::f32::consts::FRAC_PI_2;
                let split = start + std::f32::consts::TAU * ratio;
                let end = start + std::f32::consts::TAU;
                for (from, to, color) in [(start, split, theme.accent), (split, end, theme.border)] {
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
        div().pl(rems(0.5)).pt(rems(0.625)).child(div().bg(theme.surface).text_color(theme.text)
            .font_family("Inter").font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .border(rems(0.0625)).border_color(theme.border).rounded_md().shadow(crate::ui::style::rem_shadow(true))
            .px(rems(0.75)).py(rems(0.5)).child(body))
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

struct DuplicateResultTooltip {
    name: SharedString,
    location: String,
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
            .shadow(crate::ui::style::rem_shadow(true)).font_family("Inter")
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
        let maximum_width = rem_pixels(rems(26.5)).min((window.viewport_size().width - rem_pixels(rems(2.0))).max(px(1.0)));
        let chrome = rem_pixels(rems(1.625));
        let mut tooltip_width = maximum_width;
        let title_lines: Vec<SharedString> = if self.file_info.is_some() {
            let mut font = window.text_style().font();
            font.family = "Inter".into();
            font.weight = FontWeight::MEDIUM;
            font.features = TNUM.clone();
            let line = window.text_system().shape_line(self.title.clone(), rem_pixels(rems(0.75)), &[TextRun {
                len: self.title.len(), font: font.clone(), color: theme.accent, background_color: None, underline: None, strikethrough: None,
            }], None);
            let preview_width = self.preview_view.as_ref().map(|view| view.read(cx).clone())
                .map(|preview| {
                    preview.loaded_image(window, cx)
                        .map(|image| fitted_preview_size(&image, maximum_width - chrome).width)
                        .unwrap_or_else(|| if matches!(preview.data, Some(PreviewData::Text(_))) { rem_pixels(rems(21.875)).min(maximum_width - chrome) } else { px(0.0) })
                })
                .unwrap_or(px(0.0));
            // Prefer the preview width, then widen only as needed to show the
            // complete filename on two lines, keeping the extension together.
            let preferred_width = if preview_width > px(0.0) { preview_width } else { line.width };
            tooltip_width = (preferred_width.max(rem_pixels(rems(14.0))) + chrome).min(maximum_width);
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
        } else {
            vec![self.title.clone()]
        };

        let mut box_div = div()
            .max_w(tooltip_width)
            .when(self.file_info.is_some(), |tooltip| tooltip.w(tooltip_width))
            .bg(theme.surface)
            .border(rems(0.0625))
            .border_color(theme.border)
            .rounded_md()
            .shadow(crate::ui::style::rem_shadow(true))
            .font_family("Inter")
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .py_2()
            .px_3()
            .flex()
            .flex_col()
            .gap(rems(0.25))
            .when_some(self.preview_view.clone(), |this, view| {
                this.child(
                    div()
                        .mb_2()
                        .pb_2()
                        .border_b(rems(0.0625))
                        .border_color(theme.border)
                        .child(view),
                )
            })
            .child(
                div()
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
                        .max_w(rems(21.875))
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

        // Décalage pour éviter que le tooltip n'apparaisse exactement sous le pointeur de la souris
        div().pl(rems(0.5)).pt(rems(0.625)).child(box_div)
    }
}

fn send_video_frame<T>(
    sender: &tokio::sync::mpsc::Sender<T>,
    mut frame: T,
    cancelled: &std::sync::atomic::AtomicBool,
) -> bool {
    loop {
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }
        match sender.try_send(frame) {
            Ok(()) => return true,
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => return false,
            Err(tokio::sync::mpsc::error::TrySendError::Full(value)) => {
                frame = value;
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

struct CancelVideoPreview(Arc<std::sync::atomic::AtomicBool>);
impl Drop for CancelVideoPreview {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[derive(Clone)]
pub struct PreviewTask {
    data: Option<PreviewData>,
    media_asset: Option<SharedString>,
    original_image: Option<std::path::PathBuf>,
    _video_cancellation: Option<Arc<CancelVideoPreview>>,
}

impl PreviewTask {
    fn loaded_image(&self, window: &mut Window, cx: &mut App) -> Option<Arc<RenderImage>> {
        if let Some(source) = &self.media_asset {
            return window.use_asset::<TooltipPreviewAsset>(source, cx).and_then(Result::ok);
        }
        if let Some(source) = &self.original_image {
            return window.use_asset::<ImageAssetLoader>(&source.clone().into(), cx).and_then(Result::ok);
        }
        match &self.data { Some(PreviewData::Image(image)) => Some(image.clone()), _ => None }
    }
    fn new(path: String, preview_type: PreviewType, cx: &mut Context<Self>) -> Self {
        if preview_type == PreviewType::Image {
            return Self {
                data: None,
                media_asset: None,
                original_image: Some(path.into()),
                _video_cancellation: None,
            };
        }
        if preview_type == PreviewType::Pdf {
            return Self {
                data: None,
                media_asset: Some(path.into()),
                original_image: None,
                _video_cancellation: None,
            };
        }
        let cancellation = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let video_cancellation = (preview_type == PreviewType::Video)
            .then(|| Arc::new(CancelVideoPreview(cancellation.clone())));
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                if preview_type == PreviewType::Video {
                    // Channel size 4: gives the decode thread a small buffer so it is never
                    // starved while the async side is sleeping, without letting it run
                    // arbitrarily far ahead.
                    let (tx, mut rx) =
                        tokio::sync::mpsc::channel::<(Arc<RenderImage>, Duration)>(4);

                    let path_clone = path.clone();
                    std::thread::spawn(move || {
                        let receiver = tx.clone();
                        let send_cancellation = cancellation.clone();
                        let result = ::oxyde_lib::platform::platform().video_frames(
                            path_clone,
                            Box::new(move |rgba, pts| {
                                let frame = Arc::new(RenderImage::new(vec![image::Frame::new(rgba)]));
                                send_video_frame(&tx, (frame, pts), &send_cancellation)
                            }),
                            Box::new(move || {
                                cancellation.load(std::sync::atomic::Ordering::Relaxed) || receiver.is_closed()
                            }),
                        );
                        if let Err(error) = result {
                            log::warn!("Could not stream tooltip video preview: {error}");
                        }
                    });

                    // PTS-based playback: sleep until wall-clock time matches the frame's
                    // presentation timestamp.  `start` is reset whenever the PTS resets to
                    // near zero (video looped), keeping timing accurate across restarts.
                    let mut start = std::time::Instant::now();
                    let mut last_pts = Duration::ZERO;

                    while let Some((frame, pts)) = rx.recv().await {
                        // Detect loop restart: PTS jumped back by more than 500 ms.
                        if pts + Duration::from_millis(500) < last_pts {
                            start = std::time::Instant::now();
                        }
                        last_pts = pts;

                        let res = this.update(&mut cx, |this, cx| {
                            this.data = Some(PreviewData::Image(frame));
                            cx.notify();
                        });
                        if res.is_err() {
                            break;
                        }

                        // Sleep only as long as needed to stay on schedule.
                        // If we are already late, skip the sleep (drop-frame behaviour).
                        let elapsed = start.elapsed();
                        if pts > elapsed {
                            cx.background_executor().timer(pts - elapsed).await;
                        }
                    }
                    return;
                }

                let preview = match preview_type {
                    PreviewType::Image | PreviewType::Office | PreviewType::Pdf => {
                        let rgba_img = match preview_type {
                            PreviewType::Image => {
                                let path = path.clone();
                                match cx
                                    .background_executor()
                                    .spawn(async move {
                                        ::oxyde_lib::platform::platform().thumbnail(path)
                                    })
                                    .await
                                {
                                    Ok(image) => Some(image),
                                    Err(error) => {
                                        log::warn!("Could not load tooltip image preview: {error}");
                                        None
                                    }
                                }
                            }
                            PreviewType::Office => {
                                let path = path.clone();
                                match cx
                                    .background_executor()
                                    .spawn(async move {
                                        ::oxyde_lib::platform::platform().office_thumbnail(path)
                                    })
                                    .await
                                {
                                    Ok(image) => Some(image),
                                    Err(error) => {
                                        log::warn!(
                                            "Could not load tooltip office preview: {error}"
                                        );
                                        None
                                    }
                                }
                            }
                            PreviewType::Pdf => {
                                let path = path.clone();
                                match cx
                                    .background_executor()
                                    .spawn(async move {
                                        ::oxyde_lib::platform::platform().pdf_thumbnail(path)
                                    })
                                    .await
                                {
                                    Ok(image) => Some(image),
                                    Err(error) => {
                                        log::warn!("Could not load tooltip PDF preview: {error}");
                                        None
                                    }
                                }
                            }
                            _ => None,
                        };

                        if let Some(rgba) = rgba_img {
                            // Backend now consistently returns BGRA for all types
                            let frame = image::Frame::new(rgba);
                            Some(PreviewData::Image(Arc::new(RenderImage::new(vec![frame]))))
                        } else if preview_type == PreviewType::Office {
                            // Fallback to text for Office if no image
                            let path_for_text = path.clone();
                            let content = match cx
                                .background_executor()
                                .spawn(async move {
                                    ::oxyde_lib::platform::platform()
                                        .office_text_preview(path_for_text)
                                })
                                .await
                            {
                                Ok(content) => Some(content),
                                Err(error) => {
                                    log::warn!(
                                        "Could not load tooltip office text preview: {error}"
                                    );
                                    None
                                }
                            };
                            if let Some(content) = content {
                                let snippet =
                                    content.lines().take(15).collect::<Vec<_>>().join("\n");
                                Some(PreviewData::Text(snippet))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    PreviewType::Text => {
                        let content = cx
                            .background_executor()
                            .spawn(async move {
                                std::fs::read_to_string(path).map_err(|error| {
                                    ::oxyde_lib::models::CommandError::IoError(error.to_string())
                                })
                            })
                            .await;
                        if let Ok(content) = content {
                            let snippet = content.lines().take(15).collect::<Vec<_>>().join("\n");
                            Some(PreviewData::Text(snippet))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                if let Some(p) = preview {
                    let _ = this.update(&mut cx, |this, cx| {
                        this.data = Some(p);
                        cx.notify();
                    });
                }
            }
        })
        .detach();

        Self {
            data: None,
            media_asset: None,
            original_image: None,
            _video_cancellation: video_cancellation,
        }
    }
}

fn fitted_preview_size(image: &RenderImage, maximum_width: Pixels) -> Size<Pixels> {
    let natural = image.size(0);
    let ratio = natural.width.0 as f32 / (natural.height.0 as f32).max(1.0);
    let width = maximum_width.min(rem_pixels(rems(23.75)) * ratio);
    size(width, width / ratio.max(0.001))
}

fn network_tooltip_lines(file: &oxyde_lib::models::FileEntry, i18n: &crate::ui::i18n::I18n) -> Vec<SharedString> {
    let details = file.network_details.as_ref();
    let category = details.map(|d| d.category.clone()).filter(|value| !value.is_empty()).unwrap_or_else(|| i18n.text(if oxyde_lib::commands::io::is_network_server(&file.path) { Message::NetworkComputer } else { Message::NetworkOtherDevices }).to_string());
    [(Message::TooltipNetworkCategory, category), (Message::TooltipNetworkWorkgroup, details.map(|d| d.workgroup.clone()).unwrap_or_default()), (Message::TooltipNetworkLocation, details.map(|d| d.location.clone()).unwrap_or_default())]
        .into_iter().filter(|(_, value)| !value.is_empty()).map(|(message, value)| {
            let mut args = FluentArgs::new(); args.set("value", value); i18n.text_with_args(message, &args)
        }).collect()
}

impl Render for PreviewTask {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *OxydeTheme::global(cx);
        if let Some(image) = self.loaded_image(window, cx) {
            let maximum = rem_pixels(rems(24.875)).min((window.viewport_size().width - rem_pixels(rems(3.625))).max(px(1.0)));
            let size = fitted_preview_size(&image, maximum);
            return div().w_full().flex().child(
                img(image)
                    .w(size.width)
                    .h(size.height)
                    .object_fit(ObjectFit::Contain),
            );
        }
        if let Some(preview) = &self.data {
            div().w_full().flex().child(match preview {
                PreviewData::Image(render_img) => img(render_img.clone())
                    .w(rems(25.0))
                    .max_h(rems(23.75))
                    .object_fit(ObjectFit::Contain)
                    .into_any_element(),
                PreviewData::Text(t) => div()
                    .max_w(rems(21.875))
                    .max_h(rems(12.5))
                    .overflow_hidden()
                    .rounded_md()
                    .bg(theme.active_background)
                    .px(rems(0.625))
                    .py(rems(0.5))
                    .text_size(rems(0.625))
                    .font_family("JetBrains Mono")
                    .text_color(theme.text)
                    .child(t.clone())
                    .into_any_element(),
            })
        } else {
            div()
                .text_size(rems(0.625))
                .text_color(theme.text_muted)
                .child(cx.i18n().text(Message::PreviewLoading))
        }
    }
}

#[cfg(test)]
mod video_cancellation_tests {
    use super::{send_video_frame, CancelVideoPreview};
    use std::{sync::Arc, time::Duration};
    #[test]
    fn a_full_frame_queue_can_be_cancelled_without_a_consumer() {
        let (sender, _receiver) = tokio::sync::mpsc::channel(1);
        sender.try_send(1).unwrap();
        let signal = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_signal = signal.clone();
        let (done, result) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            done.send(send_video_frame(&sender, 2, &worker_signal))
                .unwrap();
        });
        std::thread::sleep(Duration::from_millis(20));
        signal.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(!result.recv_timeout(Duration::from_secs(1)).unwrap());
        worker.join().unwrap();
    }
    #[test]
    fn final_preview_owner_cancels_without_receiving_any_frame() {
        let signal = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let owner = Arc::new(CancelVideoPreview(signal.clone()));
        let clone = owner.clone();
        drop(owner);
        assert!(!signal.load(std::sync::atomic::Ordering::Relaxed));
        drop(clone);
        assert!(signal.load(std::sync::atomic::Ordering::Relaxed));
    }
}
