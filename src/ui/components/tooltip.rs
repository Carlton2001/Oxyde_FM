mod assets;
mod presentation;
mod preview;
mod video;
mod placement;
pub(crate) mod hover;
mod pdf_cache;
mod position;

use video::CancelVideoPreview;
use crate::ui::components::icons::{get_preview_type, PreviewType};
use crate::ui::format::format_date;
use crate::ui::i18n::{I18nContext, Message};
use gpui::*;
use std::sync::Arc;

pub(crate) fn invalidate_pdf_preview(path: &str) {
    pdf_cache::invalidate(path);
}

/// Un composant de bulle d'aide (Tooltip) dans le style Oxyde.
pub struct Tooltip {
    title: SharedString,
    wrap_title: bool,
    meta: Option<SharedString>,
    shortcut: Option<SharedString>,
    file_info: Option<FileInfo>,
    preview_view: Option<Entity<PreviewTask>>,
    fixed_bounds: Option<std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>>,
}

struct FileInfo {
    location: Option<String>,
    network: Option<oxyde_lib::models::FileEntry>,
    ext: String,
    size: (u64, bool, bool),
    date: u64,
}

#[derive(Clone)]
enum PreviewData {
    Image(Arc<RenderImage>),
    Text(String),
}

impl Tooltip {
    pub(super) fn ready_for_handover(&self, cx: &mut App) -> bool {
        self.preview_view.as_ref().map(|preview| preview.read(cx).clone())
            .is_none_or(|preview| preview.ready_for_handover(cx))
    }
    pub(crate) fn fixed_path(title: SharedString, bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |_, cx| cx.new(|_| Self {
            title: title.clone(), wrap_title: true, meta: None, shortcut: None,
            file_info: None, preview_view: None, fixed_bounds: Some(bounds.clone()),
        }).into()
    }
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
                Self { title, wrap_title: false, meta: None, shortcut: Some(if window.modifiers().shift { "Shift+Del" } else { "Del" }.into()), file_info: None, fixed_bounds: None, preview_view: None }
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
            title: file.name.clone().into(), wrap_title: false, meta: None, shortcut: None,
            file_info: Some(FileInfo { location: None, network: Some(file.clone()), ext: "Réseau".into(), size: (0, false, false), date: 0 }),
            fixed_bounds: None, preview_view: None,
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
                title: title.clone(), wrap_title: false, meta: None, shortcut: shortcut.clone(),
                file_info: None, fixed_bounds: None, preview_view: None,
            }).into()
        }
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
                title: title.clone(),
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
                title: title.clone(),
                wrap_title: false,
                meta: Some(meta.clone()),
                shortcut: None,
                file_info: None,
                fixed_bounds: None, preview_view: None,
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
                wrap_title: false,
                meta: None,
                shortcut: None,
                file_info: Some(FileInfo {
                    location: None,
                    network: None,
                    ext: ext.clone(),
                    size,
                    date,
                }),
                fixed_bounds: None, preview_view: None,
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
                size,
                date,
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
                wrap_title: false,
                meta: None,
                shortcut: None,
                file_info: Some(FileInfo { location, network: None, ext, size, date }),
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
    maximum_size: Option<Size<Pixels>>,
    data: Option<PreviewData>,
    media_asset: Option<SharedString>,
    original_image: Option<std::path::PathBuf>,
    message: Option<Message>,
    loading_delay_elapsed: bool,
    _video_cancellation: Option<Arc<CancelVideoPreview>>,
}
