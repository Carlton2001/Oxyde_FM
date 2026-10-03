use crate::ui::components::icons::{get_preview_type, PreviewType};
use crate::ui::format::{format_date, format_size};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::style::TNUM;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::sync::Arc;
use std::time::Duration;

/// Un composant de bulle d'aide (Tooltip) dans le style Oxyde.
pub struct Tooltip {
    title: SharedString,
    meta: Option<SharedString>,
    file_info: Option<FileInfo>,
    preview_view: Option<Entity<PreviewTask>>,
}

struct FileInfo {
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
    /// Crée un constructeur de tooltip simple affichant un texte.
    #[allow(dead_code)]
    pub fn text(
        title: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let title = title.into();
        move |_, cx| {
            cx.new(|_| Self {
                title: title.clone(),
                meta: None,
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
                file_info: Some(FileInfo {
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
        let preview_type = get_preview_type(&name);
        cx.new(|cx| {
            let preview_view = if preview_type != PreviewType::None {
                Some(cx.new(|cx| PreviewTask::new(path, preview_type, cx)))
            } else {
                None
            };

            Self {
                title: SharedString::from(name),
                meta: None,
                file_info: Some(FileInfo { ext, size, date }),
                preview_view,
            }
        })
    }
}

impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = OxydeTheme::global(cx);

        let mut box_div = div()
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
                    .max_w(rems(25.0))
                    .text_color(if self.file_info.is_some() {
                        theme.accent
                    } else {
                        theme.text
                    })
                    .text_size(rems(0.75))
                    .font_weight(FontWeight::MEDIUM)
                    .child(self.title.clone()),
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
                let i18n = cx.i18n();
                let mut args = FluentArgs::new();
                let type_label = match info.ext.as_str() {
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
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .text_size(rems(0.6875))
                        .text_color(theme.text_muted)
                        .child(file_type)
                        .child(file_size)
                        .child(modified),
                )
            });

        box_div.text_style().font_features = Some(TNUM.clone());

        // Décalage pour éviter que le tooltip n'apparaisse exactement sous le pointeur de la souris
        div().pl(rems(0.5)).pt(rems(0.625)).child(box_div)
    }
}

pub struct PreviewTask {
    data: Option<PreviewData>,
    media_asset: Option<SharedString>,
    original_image: Option<std::path::PathBuf>,
}

impl PreviewTask {
    fn new(path: String, preview_type: PreviewType, cx: &mut Context<Self>) -> Self {
        if preview_type == PreviewType::Image {
            return Self {
                data: None,
                media_asset: None,
                original_image: Some(path.into()),
            };
        }
        if preview_type == PreviewType::Pdf {
            return Self {
                data: None,
                media_asset: Some(path.into()),
                original_image: None,
            };
        }
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
                        let _ = ::oxyde_lib::commands::thumbnails::stream_video_frames(
                            path_clone,
                            move |rgba, pts| {
                                let frame =
                                    Arc::new(RenderImage::new(vec![image::Frame::new(rgba)]));
                                tx.blocking_send((frame, pts)).is_ok()
                            },
                        );
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
        }
    }
}

impl Render for PreviewTask {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = OxydeTheme::global(cx);
        if let Some(source) = self.media_asset.clone() {
            return div().child(
                img(move |window: &mut Window, cx: &mut App| {
                    window.use_asset::<TooltipPreviewAsset>(&source, cx)
                })
                .w(rems(25.0))
                .max_h(rems(23.75))
                .object_fit(ObjectFit::Contain)
                .with_fallback(|| div().size_full().into_any_element()),
            );
        }
        if let Some(source) = self.original_image.clone() {
            return div().child(
                img(source)
                    .w(rems(25.0))
                    .max_h(rems(23.75))
                    .object_fit(ObjectFit::Contain),
            );
        }
        if let Some(preview) = &self.data {
            div().child(match preview {
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
