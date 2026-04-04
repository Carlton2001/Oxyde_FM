use gpui::*;
use gpui::prelude::FluentBuilder;
use crate::ui::theme::OxydeTheme;
use crate::ui::style::TNUM;

/// Un composant de bulle d'aide (Tooltip) dans le style Oxyde.
/// Il peut afficher un titre principal et des métadonnées optionnelles (comme un raccourci clavier).
pub struct Tooltip {
    title: SharedString,
    meta: Option<SharedString>,
    file_info: Option<FileInfo>,
}

struct FileInfo {
    ext: String,
    size: String,
    date: String,
}

impl Tooltip {
    /// Crée un constructeur de tooltip simple affichant un texte.
    /// Idéal pour être passé directement à la méthode `.tooltip()` d'un élément.
    #[allow(dead_code)]
    pub fn text(title: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let title = title.into();
        move |_, cx| {
            cx.new(|_| Self {
                title: title.clone(),
                meta: None,
                file_info: None,
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
            }).into()
        }
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
            .font_family("Inter")
            .font_weight(FontWeight::LIGHT)
            .py_1()
            .px_2()
            .flex()
            .flex_col()
            .gap(px(2.0))
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
                        .max_w(px(300.0))
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
            });

        box_div.text_style().get_or_insert_with(Default::default).font_features = Some(TNUM.clone());

        // Décalage pour éviter que le tooltip n'apparaisse exactement sous le pointeur de la souris
        div()
            .pl(px(8.0))
            .pt(px(10.0))
            .child(box_div)
    }
}
