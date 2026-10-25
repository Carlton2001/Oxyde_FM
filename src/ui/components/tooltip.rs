mod assets;
mod pixels;
mod presentation;
mod preview;
mod video;
mod viewer;
mod placement;
pub(crate) mod hover;
mod pdf_cache;
mod image_cache;
mod native;
pub(crate) use native::NativeTooltipExt;
mod position;

/// Image and PDF caches respectively; each total overlaps the preview budget.
pub(crate) fn diagnostic_cache_snapshot() -> [Option<(usize, usize)>; 2] {
    [image_cache::diagnostic_snapshot(), pdf_cache::diagnostic_snapshot()]
}

use video::CancelVideoPreview;
use crate::ui::components::icons::{get_preview_type, PreviewType};
use crate::ui::format::format_date;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use gpui::*;
use std::sync::Arc;

pub(crate) fn invalidate_media_preview(path: &str) {
    crate::ui::audio_covers::invalidate(path);
    pdf_cache::invalidate(path);
    image_cache::invalidate(std::path::Path::new(path));
}

// Worker-only lookup: signature checks must stay outside UI rendering.
pub(crate) fn cached_media_pixels(path: &str, pdf: bool) -> Option<Arc<RenderImage>> {
    if pdf { pdf_cache::cached(path) } else {
        image_cache::cached(std::path::Path::new(path)).map(|preview| preview.image)
    }
}

/// Un composant de bulle d'aide (Tooltip) dans le style Oxyde.
pub struct Tooltip {
    native_presentation: bool,
    title: SharedString,
    wrap_title: bool,
    meta: Option<SharedString>,
    shortcut: Option<SharedString>,
    secondary_action: Option<(SharedString, SharedString)>,
    file_info: Option<FileInfo>,
    preview_view: Option<Entity<PreviewTask>>,
    fixed_bounds: Option<std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>>,
}

struct FileInfo {
    type_label: String,
    /// Complete location line; wrapped after path separators.
    location: Option<SharedString>,
    /// Context lines after the location, such as the deletion date.
    extra: Vec<SharedString>,
    notice: Vec<SharedString>,
    network: Option<oxyde_lib::models::FileEntry>,
    size: (u64, bool, bool),
    date: u64,
}

#[derive(Clone)]
enum PreviewData {
    Image(Arc<RenderImage>),
    Text(String),
}

type PreviewPixels = (Arc<RenderImage>, Option<(u32, u32)>);

impl Tooltip {
    pub(super) fn ready_for_initial_display(&self, cx: &mut App) -> bool {
        self.preview_view.as_ref().map(|preview| preview.read(cx).clone())
            .is_none_or(|preview| preview.ready_for_initial_display(cx))
    }

    pub(super) fn cached_for_handover(&self, cx: &mut App) -> bool {
        self.preview_view.as_ref().map(|preview| preview.read(cx).clone()).is_some_and(|preview| preview.cached_for_handover(cx))
    }

    pub(super) fn ready_for_handover(&self, cx: &mut App) -> bool {
        self.preview_view.as_ref().map(|preview| preview.read(cx).clone())
            .is_none_or(|preview| preview.ready_for_handover(cx))
    }
    pub(crate) fn fixed_path(title: SharedString, bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |_, cx| cx.new(|_| Self {
            native_presentation: false, secondary_action: None, title: title.clone(), wrap_title: true, meta: None, shortcut: None,
            file_info: None, preview_view: None, fixed_bounds: Some(bounds.clone()),
        }).into()
    }
    /// Place any plain tooltip below its source element instead of the pointer.
    pub(crate) fn anchored(
        builder: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
        bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |window, cx| {
            let view = builder(window, cx);
            if let Ok(tooltip) = view.clone().downcast::<Self>() {
                tooltip.update(cx, |tooltip, _| tooltip.fixed_bounds = Some(bounds.clone()));
            }
            view
        }
    }
    pub fn for_drive(name: String, drive: oxyde_lib::models::DriveInfo) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |_, cx| cx.new(|_| DriveTooltip { name: name.clone().into(), drive: drive.clone() }).into()
    }
    pub fn for_delete(handle: Entity<crate::ui::app::OxydeApp>, pane_id: String) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |window, cx| {
            let permanent = window.modifiers().shift || handle.read(cx).pane_delete_is_permanent(&pane_id);
            let title = cx.i18n().text(if permanent { Message::TrashDeletePermanent } else { Message::Delete });
            let pane_id = pane_id.clone();
            cx.new(|cx| {
                cx.observe(&handle, move |tooltip: &mut Self, app, cx| {
                    let app = app.read(cx);
                    let permanent = app.shift_delete || app.pane_delete_is_permanent(&pane_id);
                    tooltip.title = cx.i18n().text(if permanent { Message::TrashDeletePermanent } else { Message::Delete });
                    tooltip.shortcut = Some(if app.shift_delete { "Shift+Del" } else { "Del" }.into());
                    cx.notify();
                }).detach();
                Self { native_presentation: false, secondary_action: None, title, wrap_title: false, meta: None, shortcut: Some(if window.modifiers().shift { "Shift+Del" } else { "Del" }.into()), file_info: None, fixed_bounds: None, preview_view: None }
            }).into()
        }
    }
    /// The single entry point for pane items, in list and grid views alike.
    /// Order: name, type, size, modification date, location, context lines.
    pub(crate) fn for_item(file: oxyde_lib::models::FileEntry, search_result: bool, previews: bool, provisional_size: bool) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |_, cx| {
            if file.original_path.is_none() && crate::ui::components::file_list::network_entry(&file) {
                return cx.new(|_| Self {
                    native_presentation: false, secondary_action: None, title: file.name.clone().into(), wrap_title: false, meta: None, shortcut: None,
                    file_info: Some(FileInfo { type_label: String::new(), location: None, extra: Vec::new(), notice: Vec::new(), network: Some(file.clone()), size: (0, false, false), date: 0 }),
                    fixed_bounds: None, preview_view: None,
                }).into();
            }
            let parent = |path: &str| std::path::Path::new(path).parent().map(|parent| parent.to_string_lossy().into_owned());
            let line = |message: Message, value: String| {
                let mut args = FluentArgs::new(); args.set("value", value);
                cx.i18n().text_with_args(message, &args)
            };
            let (location, extra) = if let Some(original) = &file.original_path {
                let deleted = file.deleted_time.and_then(|date| u64::try_from(date).ok()).unwrap_or(0);
                (Some(line(Message::TooltipOriginalLocation, parent(original).unwrap_or_default())),
                    vec![line(Message::TooltipDeleted, format_date(deleted, crate::ui::format::date_format(cx)))])
            } else if search_result {
                (parent(&file.path).map(|parent| line(Message::TooltipLocation, parent)), Vec::new())
            } else {
                (None, Vec::new())
            };
            let notice = if provisional_size {
                cx.i18n().text(Message::FolderSizeProvisionalDescription)
                    .lines().map(|line| SharedString::from(line.to_owned())).collect()
            } else { Vec::new() };
            let info = FileInfo {
                type_label: crate::ui::components::file_list::file_type_display(&file, cx.i18n()),
                location, extra, notice, network: None,
                size: (file.size, file.is_dir, file.is_calculated), date: file.modified,
            };
            Self::new_for_file(file.name.clone(), file.path.clone(), info, previews, cx).into()
        }
    }
    /// Keep action labels and their optional keyboard shortcut on one line.
    pub fn with_shortcut(
        title: impl Into<SharedString>,
        shortcut: Option<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let title = title.into();
        move |_, cx| {
            cx.new(|_| Self {
                native_presentation: false, secondary_action: None, title: title.clone(), wrap_title: false, meta: None, shortcut: shortcut.clone(),
                file_info: None, fixed_bounds: None, preview_view: None,
            }).into()
        }
    }
    /// An alternate mouse action below the primary label, with its own shortcut.
    pub fn with_secondary_action(
        title: SharedString,
        label: SharedString,
        shortcut: SharedString,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |_, cx| cx.new(|_| Self {
            native_presentation: false, title: title.clone(), wrap_title: false,
            meta: None, shortcut: None, secondary_action: Some((label.clone(), shortcut.clone())),
            file_info: None, preview_view: None, fixed_bounds: None,
        }).into()
    }

    /// Crée un constructeur de tooltip simple affichant un texte.
    pub fn text(
        title: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        Self::plain_text(title.into(), false)
    }

    /// Full notices and paths wrap within the bubble without truncation.
    pub fn wrapped_text(
        title: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        Self::plain_text(title.into(), true)
    }

    fn plain_text(
        title: SharedString,
        wrap_title: bool,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |_, cx| {
            cx.new(|_| Self {
                native_presentation: false, secondary_action: None, title: title.clone(),
                wrap_title,
                meta: None,
                shortcut: None,
                file_info: None,
                fixed_bounds: None, preview_view: None,
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
                native_presentation: false, secondary_action: None, title: title.clone(),
                wrap_title: false,
                meta: Some(meta.clone()),
                shortcut: None,
                file_info: None,
                fixed_bounds: None, preview_view: None,
            })
            .into()
        }
    }

    pub fn duplicate_result(name: String, location: String, cx: &mut App) -> AnyView {
        cx.new(|_| DuplicateResultTooltip { name: name.into(), location }).into()
    }

    fn new_for_file(name: String, path: String, info: FileInfo, previews: bool, cx: &mut App) -> Entity<Self> {
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
                native_presentation: false, secondary_action: None, title: SharedString::from(name),
                wrap_title: false,
                meta: None,
                shortcut: None,
                file_info: Some(info),
                fixed_bounds: None,
                preview_view,
            }
        })
    }
}

struct DriveTooltip {
    name: SharedString,
    drive: oxyde_lib::models::DriveInfo,
}

struct DuplicateResultTooltip {
    name: SharedString,
    location: String,
}

#[derive(Clone)]
pub struct PreviewTask {
    viewer: bool,
    transition_image: Option<PreviewPixels>,
    maximum_size: Option<Size<Pixels>>,
    data: Option<PreviewData>,
    media_asset: Option<SharedString>,
    original_image: Option<std::path::PathBuf>,
    audio_source: Option<String>,
    audio_cached: bool,
    message: Option<Message>,
    loading_delay_elapsed: bool,
    _video_cancellation: Option<Arc<CancelVideoPreview>>,
}
