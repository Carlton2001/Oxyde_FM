use super::*;
use crate::ui::date_groups::visible_indices;

fn file(name: &str, size: u64) -> FileEntry {
    FileEntry {
        name: name.into(),
        path: format!("C:/fixture/{name}"),
        size,
        ..Default::default()
    }
}
fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

#[test]
fn size_boundaries_and_uncalculated_folders_are_distinct() {
    for (size, expected) in [
        (0, SizeGroup::Empty),
        (1, SizeGroup::Tiny),
        (10239, SizeGroup::Tiny),
        (10240, SizeGroup::Small),
        (1048575, SizeGroup::Small),
        (1048576, SizeGroup::Medium),
        (104857599, SizeGroup::Medium),
        (104857600, SizeGroup::Large),
        (1073741823, SizeGroup::Large),
        (1073741824, SizeGroup::Huge),
        (u64::MAX, SizeGroup::Huge),
    ] {
        assert_eq!(
            group_for_file(&file("file", size), GroupMode::Size, today()),
            GroupKey::Size(expected)
        );
    }
    let mut folder = FileEntry {
        is_dir: true,
        ..file("folder", 2000000)
    };
    assert_eq!(
        group_for_file(&folder, GroupMode::Size, today()),
        GroupKey::Family(FileFamily::Folders)
    );
    folder.is_calculated = true;
    assert_eq!(
        group_for_file(&folder, GroupMode::Size, today()),
        GroupKey::Size(SizeGroup::Medium)
    );
    folder.size = 0;
    assert_eq!(
        group_for_file(&folder, GroupMode::Size, today()),
        GroupKey::Size(SizeGroup::Empty)
    );
}

#[test]
fn extensions_are_case_insensitive_keep_order_and_pack_separate_grid_rows() {
    let files = [
        "b.ZIP",
        "a.png",
        "a.zip",
        "README",
        ".gitignore",
        "folder.zip",
    ]
    .map(|name| file(name, 0));
    let mut files = files.to_vec();
    files[5].is_dir = true;
    let rows = display_rows(&files, GroupMode::Extension, &HashSet::new(), 2);
    assert_eq!(visible_indices(&rows), [5, 1, 0, 2, 3, 4]);
    let collapsed = HashSet::from([GroupKey::Extension("zip".into())]);
    assert_eq!(
        visible_indices(&display_rows(&files, GroupMode::Extension, &collapsed, 2)),
        [5, 1, 3, 4]
    );
    assert!(matches!(&rows[5], DisplayRow::Files(indices) if indices == &[0, 2]));
}

#[test]
fn origin_uses_original_paths_in_trash_and_full_parent_paths_in_search() {
    let entries = [
        FileEntry {
            path: "C:/a/project/file.txt".into(),
            ..file("file.txt", 0)
        },
        FileEntry {
            path: "C:/b/project/file.txt".into(),
            ..file("file.txt", 0)
        },
        FileEntry {
            path: "C:/$Recycle.Bin/payload".into(),
            original_path: Some("C:/a/project/old.txt".into()),
            ..file("old.txt", 0)
        },
    ];
    let rows = display_rows(&entries, GroupMode::Origin, &HashSet::new(), 1);
    assert_eq!(visible_indices(&rows), [0, 2, 1]);
    let keys: Vec<_> = rows
        .iter()
        .filter_map(|row| {
            if let DisplayRow::Header(group, _) = row {
                Some(group)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(keys.len(), 2);
    assert_ne!(keys[0], keys[1]);
}

#[test]
fn creation_and_deletion_use_their_own_dates_and_missing_data_stays_unknown() {
    let now = chrono::Local::now().timestamp_millis() as u64;
    let mut entry = FileEntry {
        modified: 0,
        created: Some(now),
        deleted_time: Some(now as i64),
        ..file("file", 0)
    };
    assert_eq!(
        group_for_file(&entry, GroupMode::Date, today()),
        GroupKey::Date(DateGroup::Older)
    );
    for mode in [GroupMode::Created, GroupMode::Deleted] {
        assert_eq!(
            group_for_file(&entry, mode, today()),
            GroupKey::Date(DateGroup::Today)
        );
    }
    entry.created = None;
    entry.deleted_time = None;
    for mode in [GroupMode::Created, GroupMode::Deleted] {
        assert_eq!(group_for_file(&entry, mode, today()), GroupKey::MissingDate);
    }
    entry.created = Some(u64::MAX);
    entry.deleted_time = Some(-1);
    assert_eq!(
        group_for_file(&entry, GroupMode::Created, today()),
        GroupKey::MissingDate
    );
    assert_eq!(
        group_for_file(&entry, GroupMode::Deleted, today()),
        GroupKey::MissingDate
    );
}
