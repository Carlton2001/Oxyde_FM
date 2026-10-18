use super::*;
use crate::ui::date_groups::GroupMode;
use crate::ui::date_groups::{display_rows, visible_indices, DateGroup, DisplayRow, GroupKey};
use std::collections::HashSet;

fn file(name: &str) -> FileEntry {
    FileEntry {
        name: name.into(),
        ..Default::default()
    }
}

#[test]
fn families_cover_case_compound_extensions_and_unknown_files() {
    for (name, expected) in [
        ("BACKUP.ZIP", FileFamily::Archives),
        ("backup.tar.gz", FileFamily::Archives),
        ("backup.tar.xz", FileFamily::Archives),
        ("photo.HEIC", FileFamily::Images),
        ("installer.MSIX", FileFamily::Applications),
        ("report.docx", FileFamily::Documents),
        ("movie.mkv", FileFamily::Video),
        ("music.flac", FileFamily::Audio),
        ("main.rs", FileFamily::Code),
        ("main.ts", FileFamily::Code),
        ("README", FileFamily::Other),
        (".gitignore", FileFamily::Other),
        ("file.unknown", FileFamily::Other),
        ("file.", FileFamily::Other),
        // A document remains a document although its container format is ZIP.
        ("report.xlsx", FileFamily::Documents),
    ] {
        assert_eq!(FileFamily::for_file(&file(name)), expected, "{name}");
    }
    assert_eq!(
        FileFamily::for_file(&FileEntry {
            is_dir: true,
            ..file("folder.zip")
        }),
        FileFamily::Folders
    );
}

#[test]
fn family_rows_keep_sort_indices_pack_each_grid_group_and_restore_date_mode() {
    let files = [
        "b.zip",
        "photo.png",
        "a.7z",
        "unknown",
        "folder",
        "c.tar.gz",
    ]
    .map(file);
    let files: Vec<_> = files
        .into_iter()
        .enumerate()
        .map(|(index, mut file)| {
            file.is_dir = index == 4;
            file
        })
        .collect();
    let rows = display_rows(&files, GroupMode::Type, &HashSet::new(), 2);
    assert!(matches!(
        rows[0],
        DisplayRow::Header(GroupKey::Family(FileFamily::Folders), 1)
    ));
    assert!(matches!(
        rows[2],
        DisplayRow::Header(GroupKey::Family(FileFamily::Archives), 3)
    ));
    assert!(matches!(&rows[3], DisplayRow::Files(indices) if indices == &[0, 2]));
    assert!(matches!(&rows[4], DisplayRow::Files(indices) if indices == &[5]));
    assert_eq!(visible_indices(&rows), [4, 0, 2, 5, 1, 3]);
    let collapsed = HashSet::from([
        GroupKey::Family(FileFamily::Archives),
        GroupKey::Date(DateGroup::Older),
    ]);
    assert_eq!(
        visible_indices(&display_rows(&files, GroupMode::Type, &collapsed, 1)),
        [4, 1, 3]
    );
    assert!(visible_indices(&display_rows(&files, GroupMode::Date, &collapsed, 1)).is_empty());
    assert_eq!(
        visible_indices(&display_rows(&files, GroupMode::None, &collapsed, 2)),
        [0, 1, 2, 3, 4, 5]
    );
}

#[test]
fn native_categories_fill_unknown_extensions_without_overriding_catalog() {
    use oxyde_lib::models::file_entry::NativeFileKind;
    for (native_kind, expected) in [
        (NativeFileKind::Archives, FileFamily::Archives),
        (NativeFileKind::Documents, FileFamily::Documents),
        (NativeFileKind::Images, FileFamily::Images),
        (NativeFileKind::Audio, FileFamily::Audio),
        (NativeFileKind::Video, FileFamily::Video),
        (NativeFileKind::Applications, FileFamily::Applications),
    ] {
        let entry = FileEntry {
            native_kind: Some(native_kind),
            ..file("custom.oxyde_unknown")
        };
        assert_eq!(FileFamily::for_file(&entry), expected);
    }
    let source = FileEntry {
        native_kind: Some(NativeFileKind::Documents),
        ..file("main.rs")
    };
    assert_eq!(FileFamily::for_file(&source), FileFamily::Code);
    let folder = FileEntry {
        is_dir: true,
        native_kind: Some(NativeFileKind::Archives),
        ..file("folder.custom")
    };
    assert_eq!(FileFamily::for_file(&folder), FileFamily::Folders);
}

#[test]
fn disk_images_have_their_own_family_even_when_windows_reports_an_archive() {
    use oxyde_lib::models::file_entry::NativeFileKind;
    for name in [
        "system.ISO",
        "disk.img",
        "disk.vhd",
        "disk.vhdx",
        "disk.avhdx",
        "disk.vmdk",
        "disk.vdi",
        "disk.qcow",
        "disk.qcow2",
        "installer.dmg",
        "disk.nrg",
        "disk.sparseimage",
    ] {
        let entry = FileEntry {
            native_kind: Some(NativeFileKind::Archives),
            ..file(name)
        };
        assert_eq!(
            FileFamily::for_file(&entry),
            FileFamily::DiskImages,
            "{name}"
        );
    }
    // Ambiguous extensions stay in their established category.
    assert_eq!(FileFamily::for_file(&file("photo.raw")), FileFamily::Images);
    assert_eq!(FileFamily::for_file(&file("data.bin")), FileFamily::Other);
}
