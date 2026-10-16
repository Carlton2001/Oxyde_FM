use super::{IconType, PDF_EXTENSIONS};
use crate::ui::theme::{OxydeTheme, ThemeId};
use gpui::{rgb, Hsla};

/// Theme-specific file colors; other files remain neutral.
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
impl FileIconPalette {
    pub fn for_theme(id: ThemeId) -> Self {
        let colors = match id {
            ThemeId::Light => [0x3d5af1, 0x087f5b, 0x7542c8, 0xc92a50, 0xc92a2a, 0x087e8b],
            ThemeId::Dark => [0xd4a54a, 0x10b981, 0xa78bfa, 0xfb7185, 0xf87171, 0x22d3ee],
            ThemeId::AyuLight => [0xff9940, 0x668800, 0x8553ad, 0xf07178, 0xf07178, 0x247e99],
            ThemeId::AyuDark => [0xe6b450, 0x7edaf6, 0xdfbfff, 0xf07178, 0xf07178, 0x59c2ff],
            ThemeId::GithubLight => [0x54aeff, 0x1a7f37, 0x8250df, 0xcf222e, 0xcf222e, 0x0550ae],
            ThemeId::GithubDark => [0x79c0ff, 0x3fb950, 0xbc8cff, 0xff7b72, 0xff7b72, 0x58a6ff],
            ThemeId::OneLight => [0x4078f2, 0x50a14f, 0xa626a4, 0xe45649, 0xe45649, 0x0184bc],
            ThemeId::OneDark => [0x61afef, 0x98c379, 0xc678dd, 0xe06c75, 0xe06c75, 0x56b6c2],
            ThemeId::SolarizedLight => [0x268bd2, 0x859900, 0x6c71c4, 0xdc322f, 0xdc322f, 0x2aa198],
            ThemeId::SolarizedDark => [0xb58900, 0x859900, 0x6c71c4, 0xd33682, 0xdc322f, 0x2aa198],
            ThemeId::WindowsLight => [0x0672cb, 0x2e7d32, 0x673ab7, 0xc62828, 0xd32f2f, 0x009688],
            ThemeId::WindowsDark => [0x52a8ff, 0x81c784, 0xb39ddb, 0xef9a9a, 0xe57373, 0x4db6ac],
            ThemeId::Monokai => [0xe6db74, 0xa6e22e, 0xae81ff, 0xf92672, 0xf92672, 0x66d9ef],
        };
        Self {
            folder: rgb(colors[0]).into(),
            archive: rgb(match id {
                ThemeId::Light => 0x946b16,
                ThemeId::Dark => 0xd4a54a,
                ThemeId::AyuLight => 0x99601c,
                ThemeId::AyuDark => 0xe6b450,
                ThemeId::GithubLight => 0x946b16,
                ThemeId::GithubDark => 0xd4a54a,
                ThemeId::OneLight => 0x986801,
                ThemeId::OneDark => 0xe5c07b,
                ThemeId::SolarizedLight => 0x916d00,
                ThemeId::SolarizedDark => 0xb58900,
                ThemeId::WindowsLight => 0x946b16,
                ThemeId::WindowsDark => 0xe5b95c,
                ThemeId::Monokai => 0xe6db74,
            })
            .into(),
            image: rgb(colors[1]).into(),
            audio: rgb(colors[2]).into(),
            video: rgb(colors[3]).into(),
            pdf: rgb(colors[4]).into(),
            code: rgb(colors[5]).into(),
        }
    }
}

pub(super) fn file_icon_color(path: &str, icon: IconType, theme: &OxydeTheme) -> Hsla {
    let palette = theme.file_icons;
    match icon {
        IconType::Folder | IconType::FolderOpen => palette.folder,
        IconType::Archive => palette.archive,
        IconType::FileImage => palette.image,
        IconType::Music => palette.audio,
        IconType::FileVideo => palette.video,
        IconType::FileCode | IconType::Link2 => palette.code,
        IconType::FileText
            if path
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(path)
                .rsplit_once('.')
                .is_some_and(|(_, ext)| {
                    PDF_EXTENSIONS.contains(ext.to_ascii_lowercase().as_str())
                }) =>
        {
            palette.pdf
        }
        _ => theme.text_muted,
    }
}

#[cfg(test)]
#[path = "tests/colors.rs"]
mod tests;
