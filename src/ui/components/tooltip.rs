use gpui::*;
use gpui::prelude::FluentBuilder;
use crate::ui::theme::OxydeTheme;
use crate::ui::style::TNUM;
use crate::ui::components::icons::{get_preview_type, PreviewType};
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
    size: String,
    date: String,
}

#[derive(Clone)]
enum PreviewData {
    Image(Arc<RenderImage>),
    Text(String),
}

impl Tooltip {
    /// Crée un constructeur de tooltip simple affichant un texte.
    #[allow(dead_code)]
    pub fn text(title: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let title = title.into();
        move |_, cx| {
            cx.new(|_| Self {
                title: title.clone(),
                meta: None,
                file_info: None,
                preview_view: None,
            }).into()
        }
    }

    /// Crée un constructeur de tooltip avec un texte principal et un sous-texte (méta/raccourci).
    pub fn with_meta(title: impl Into<SharedString>, meta: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let title = title.into();
        let meta = meta.into();
        move |_, cx| {
            cx.new(|_| Self {
                title: title.clone(),
                meta: Some(meta.clone()),
                file_info: None,
                preview_view: None,
            }).into()
        }
    }

    /// Crée un constructeur de tooltip complet pour un fichier avec preview possible.
    pub fn for_file_full(name: String, path: String, ext: String, size: String, date: String) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let path_clone = path.clone();
        move |_, cx| {
            Self::new_for_file_full(name.clone(), path_clone.clone(), ext.clone(), size.clone(), date.clone(), cx).into()
        }
    }

    pub fn new_for_file_full(name: String, path: String, ext: String, size: String, date: String, cx: &mut App) -> Entity<Self> {
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
                file_info: Some(FileInfo {
                    ext,
                    size,
                    date,
                }),
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
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .shadow_lg()
            .font_family("Inter")
            .font_weight(FontWeight::LIGHT)
            .py_2()
            .px_3()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .max_w(px(400.0))
                    .text_color(if self.file_info.is_some() { theme.accent } else { theme.text })
                    .text_size(px(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .child(self.title.clone())
            )
            .when_some(self.meta.clone(), |this, meta| {
                this.child(
                    div()
                        .max_w(px(350.0))
                        .text_color(theme.text_muted)
                        .text_size(px(11.0))
                        .child(meta)
                )
            })
            .when_some(self.file_info.as_ref(), |this, info| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .text_size(px(11.0))
                        .text_color(theme.text_muted)
                        .child(format!("Type : {}", info.ext))
                        .child(format!("Taille : {}", info.size))
                        .child(format!("Modifié le : {}", info.date))
                )
            })
            .when_some(self.preview_view.clone(), |this, view| {
                this.child(
                    div()
                        .mt_2()
                        .border_t_1()
                        .border_color(theme.border)
                        .pt_2()
                        .child(view)
                )
            });

        box_div.text_style().get_or_insert_with(Default::default).font_features = Some(TNUM.clone());

        // Décalage pour éviter que le tooltip n'apparaisse exactement sous le pointeur de la souris
        div()
            .pl(px(8.0))
            .pt(px(10.0))
            .child(box_div)
    }
}

pub struct PreviewTask {
    data: Option<PreviewData>,
}

impl PreviewTask {
    fn new(path: String, preview_type: PreviewType, cx: &mut Context<Self>) -> Self {
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                if preview_type == PreviewType::Video {
                    // Channel size 4: gives the decode thread a small buffer so it is never
                    // starved while the async side is sleeping, without letting it run
                    // arbitrarily far ahead.
                    let (tx, mut rx) = tokio::sync::mpsc::channel::<(Arc<RenderImage>, Duration)>(4);

                    let path_clone = path.clone();
                    std::thread::spawn(move || {
                        let _ = ::oxyde_lib::commands::thumbnails::stream_video_frames(path_clone, move |rgba, pts| {
                            let frame = Arc::new(RenderImage::new(vec![image::Frame::new(rgba)]));
                            tx.blocking_send((frame, pts)).is_ok()
                        });
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
                        if res.is_err() { break; }

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
                            PreviewType::Image => ::oxyde_lib::commands::thumbnails::get_image_thumbnail(path.clone()).await.ok(),
                            PreviewType::Office => ::oxyde_lib::commands::thumbnails::get_office_thumbnail(path.clone()).await.ok(),
                            PreviewType::Pdf => ::oxyde_lib::commands::thumbnails::get_pdf_thumbnail(path.clone()).await.ok(),
                            _ => None,
                        };

                        if let Some(rgba) = rgba_img {
                            // Backend now consistently returns BGRA for all types
                            let frame = image::Frame::new(rgba);
                            Some(PreviewData::Image(Arc::new(RenderImage::new(vec![frame]))))
                        } else if preview_type == PreviewType::Office {
                            // Fallback to text for Office if no image
                            if let Ok(content) = ::oxyde_lib::commands::thumbnails::get_office_text_preview(path.clone()).await {
                                let snippet = content.lines().take(15).collect::<Vec<_>>().join("\n");
                                Some(PreviewData::Text(snippet))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    PreviewType::Text => {
                        if let Ok(content) = tokio::fs::read_to_string(path).await {
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
        }).detach();

        Self { data: None }
    }
}

impl Render for PreviewTask {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = OxydeTheme::global(cx);
        if let Some(preview) = &self.data {
            div()
                .child(match preview {
                    PreviewData::Image(render_img) => {
                        div()
                            .w(px(250.0)) // Largeur fixe raisonnable
                            .child(img(render_img.clone()).size_full())
                    }
                    PreviewData::Text(t) => {
                        div()
                            .max_w(px(350.0))
                            .max_h(px(200.0))
                            .overflow_hidden()
                            .text_size(px(10.0))
                            .text_color(theme.text)
                            .child(t.clone())
                    }
                })
        } else {
            div()
                .text_size(px(10.0))
                .text_color(theme.text_muted)
                .child("Chargement de l'aperçu...")
        }
    }
}
