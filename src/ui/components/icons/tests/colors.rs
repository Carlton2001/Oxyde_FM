use super::*;

#[test]
fn pdf_color_depends_on_filename_and_preserves_other_text_documents() {
    let theme = OxydeTheme::light();
    for path in [r"C:\Documents\REPORT.PDF", "/documents/report.pdf"] {
        assert_eq!(
            file_icon_color(path, IconType::FileText, &theme),
            theme.file_icons.pdf
        );
    }
    for path in [
        r"C:\folder.pdf\notes.txt",
        "/folder.pdf/notes.docx",
        "notes",
    ] {
        assert_eq!(
            file_icon_color(path, IconType::FileText, &theme),
            theme.text_muted
        );
    }
    assert_eq!(
        file_icon_color("folder.pdf", IconType::Folder, &theme),
        theme.file_icons.folder
    );
}

#[test]
fn categories_use_the_active_palette_for_every_theme() {
    for id in ThemeId::ALL {
        let theme = OxydeTheme::for_id(id);
        for (icon, expected) in [
            (IconType::Folder, theme.file_icons.folder),
            (IconType::FolderOpen, theme.file_icons.folder),
            (IconType::Archive, theme.file_icons.archive),
            (IconType::FileImage, theme.file_icons.image),
            (IconType::Music, theme.file_icons.audio),
            (IconType::FileVideo, theme.file_icons.video),
            (IconType::FileCode, theme.file_icons.code),
            (IconType::Link2, theme.file_icons.code),
            (IconType::Package, theme.text_muted),
            (IconType::Disc, theme.text_muted),
            (IconType::File, theme.text_muted),
        ] {
            assert_eq!(
                file_icon_color("entry", icon, &theme),
                expected,
                "{id:?}: {icon:?}"
            );
        }
    }
}
