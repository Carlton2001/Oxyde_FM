use gpui::*;
use gpui::prelude::FluentBuilder;
use crate::ui::theme::OxydeTheme;
use crate::ui::style::TNUM;
use crate::ui::components::icons::{get_preview_type, PreviewType};
use std::sync::Arc;

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

    /// Crée un constructeur de tooltip détaillé pour un fichier.
    pub fn for_file(name: String, ext: String, size: String, date: String) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
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

    /// Crée un constructeur de tooltip affichant un nom et un chemin complet (pour la sidebar).
    pub fn for_path(name: String, path: String) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let name = SharedString::from(name);
        let path = SharedString::from(path);
        move |_, cx| {
            cx.new(|_| Self {
                title: name.clone(),
                meta: Some(path.clone()),
                file_info: None,
                preview_view: None,
            }).into()
        }
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
                let preview = match preview_type {
                    PreviewType::Image | PreviewType::Office | PreviewType::Pdf => {
                        let rgba_img = match preview_type {
                            PreviewType::Image => ::oxyde_lib::commands::thumbnails::get_image_thumbnail(path.clone()).await.ok(),
                            PreviewType::Office => ::oxyde_lib::commands::thumbnails::get_office_thumbnail(path.clone()).await.ok(),
                            PreviewType::Pdf => ::oxyde_lib::commands::thumbnails::get_pdf_thumbnail(path.clone()).await.ok(),
                            _ => None,
                        };

                        if let Some(mut rgba) = rgba_img {
                            // GPUI expects BGRA
                            for pixel in rgba.chunks_exact_mut(4) {
                                pixel.swap(0, 2);
                            }
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
                    let _ = this.update(&mut cx, |this, cx: &mut Context<Self>| {
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
