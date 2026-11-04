use super::{IconType, PDF_EXTENSIONS};
use crate::ui::theme::OxydeTheme;
use gpui::Hsla;

/// File colors computed from each theme's seeds (`theme/file_colors.rs`); other files remain neutral.
#[derive(Debug, Clone, Copy)]
pub struct FileIconPalette {
    pub folder: Hsla,
    pub archive: Hsla,
    pub image: Hsla,
    pub audio: Hsla,
    pub video: Hsla,
    pub pdf: Hsla,
    pub code: Hsla,
}
pub(super) fn file_icon_color(path: &str, icon: IconType, theme: &OxydeTheme) -> Hsla {
    let palette = theme.file_icons;
    match icon {
        IconType::Folder | IconType::FolderOpen | IconType::FolderSymlink => palette.folder,
        IconType::Archive => palette.archive,
        IconType::FileImage => palette.image,
        IconType::Music => palette.audio,
        IconType::FileVideo => palette.video,
        IconType::FileCode | IconType::Link2 | IconType::FileSymlink => palette.code,
        IconType::BrokenLink => theme.text_muted,
        IconType::FileText
            if path
                .rsplit(std::path::is_separator)
                .next()
                .unwrap_or(path)
                .rsplit_once('.')
                .is_some_and(|(_, ext)| {
                    PDF_EXTENSIONS.contains(ext.to_ascii_lowercase().as_str())
                }) =>
        {
            palette.pdf
        }
        _ => theme.text,
    }
}

#[cfg(test)]
#[path = "tests/colors.rs"]
mod tests;
