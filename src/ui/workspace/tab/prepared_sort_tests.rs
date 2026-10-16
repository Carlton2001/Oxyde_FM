use oxyde_lib::models::FileEntry;

use crate::ui::workspace::{SortColumn, SortDirection, TabState};
use std::cmp::Ordering;
fn old_sort(tab: &mut TabState) {
    let col = tab.sort_column;
    let dir = tab.sort_direction;
    std::sync::Arc::make_mut(&mut tab.files).sort_by(|a, b| {
        // Folders always first
        if a.is_dir && !b.is_dir {
            return Ordering::Less;
        }
        if !a.is_dir && b.is_dir {
            return Ordering::Greater;
        }

        let cmp = match col {
            SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortColumn::Type => {
                let ext_a = std::path::Path::new(&a.name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                let ext_b = std::path::Path::new(&b.name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                ext_a.to_lowercase().cmp(&ext_b.to_lowercase())
            }
            SortColumn::Size => a.size.cmp(&b.size),
            SortColumn::Modified => a.modified.cmp(&b.modified),
            SortColumn::OriginalLocation => {
                let location = |file: &FileEntry| {
                    file.original_path
                        .as_deref()
                        .and_then(|path| std::path::Path::new(path).parent())
                        .map(|path| path.to_string_lossy().to_lowercase())
                        .unwrap_or_default()
                };
                location(a).cmp(&location(b))
            }
            SortColumn::DeletedAt => a.deleted_time.cmp(&b.deleted_time),
            SortColumn::Location => std::path::Path::new(&a.path)
                .parent()
                .cmp(&std::path::Path::new(&b.path).parent()),
        };

        let cmp = if cmp == Ordering::Equal {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        } else {
            cmp
        };

        match dir {
            SortDirection::Ascending => cmp,
            SortDirection::Descending => cmp.reverse(),
        }
    });
}

fn fixture(count: usize) -> Vec<FileEntry> {
    (0..count)
        .rev()
        .map(|index| FileEntry {
            name: format!(
                "{} {}.{}",
                ["École", "école", "File", "file", "Résumé", "resume"][index % 6],
                index % 123,
                ["TXT", "txt", "pdf", ""][index % 4]
            ),
            path: std::path::Path::new(if index % 2 == 0 { "alpha" } else { "Zèbre" })
                .join(index.to_string())
                .to_string_lossy()
                .into_owned(),
            original_path: (index % 5 != 0).then(|| {
                std::path::Path::new(["École", "école", "archive"][index % 3])
                    .join(index.to_string())
                    .to_string_lossy()
                    .into_owned()
            }),
            size: (index % 7) as u64,
            modified: (index % 9) as u64,
            deleted_time: (index % 4 != 0).then_some((index % 8) as i64),
            is_dir: index % 3 == 0,
            ..Default::default()
        })
        .collect()
}
#[test]
fn prepared_sort_preserves_all_columns_directions_and_stable_ties() {
    for column in [
        SortColumn::Name,
        SortColumn::Type,
        SortColumn::Size,
        SortColumn::Modified,
        SortColumn::Location,
        SortColumn::OriginalLocation,
        SortColumn::DeletedAt,
    ] {
        for direction in [SortDirection::Ascending, SortDirection::Descending] {
            let mut old = TabState::new("old".into(), String::new());
            old.files = std::sync::Arc::new(fixture(300));
            // Equal names/keys with distinct paths exercise stable ordering.
            std::sync::Arc::make_mut(&mut old.files).extend([
                FileEntry {
                    name: "same".into(),
                    path: "first".into(),
                    ..Default::default()
                },
                FileEntry {
                    name: "same".into(),
                    path: "second".into(),
                    ..Default::default()
                },
            ]);
            old.sort_column = column;
            old.sort_direction = direction;
            let mut prepared = old.clone();
            old_sort(&mut old);
            prepared.sort_files();
            assert_eq!(
                old.files.iter().map(|file| &file.path).collect::<Vec<_>>(),
                prepared
                    .files
                    .iter()
                    .map(|file| &file.path)
                    .collect::<Vec<_>>(),
                "{column:?}, {direction:?}"
            );
        }
    }
}
#[test]
fn prepared_filters_preserve_unicode_case_and_accent_distinctions() {
    let mut tab = TabState::new("filter".into(), String::new());
    tab.files = std::sync::Arc::new(vec![
        FileEntry {
            name: "École.txt".into(),
            original_path: Some(
                std::path::Path::new("Résumé")
                    .join("École.txt")
                    .to_string_lossy()
                    .into_owned(),
            ),
            ..Default::default()
        },
        FileEntry {
            name: "ecole.txt".into(),
            ..Default::default()
        },
    ]);
    tab.filter_name = "ÉCOLE".into();
    tab.filter_original_location = "RÉSUMÉ".into();
    tab.update_cache();
    assert_eq!(tab.filtered_files().len(), 1);
    assert_eq!(tab.filtered_files()[0].name, "École.txt");
    tab.filter_name = "ecole".into();
    tab.filter_original_location.clear();
    tab.update_cache();
    assert_eq!(tab.filtered_files()[0].name, "ecole.txt");
}
#[test]
#[ignore = "one-off comparison of repeated comparator allocations against cached keys"]
fn profile_prepared_name_sort() {
    let files = fixture(20_000);
    let mut old = TabState::new("profile".into(), String::new());
    let mut prepared = old.clone();
    let mut old_time = std::time::Duration::ZERO;
    let mut prepared_time = std::time::Duration::ZERO;
    for _ in 0..5 {
        old.files = std::sync::Arc::new(files.clone());
        prepared.files = std::sync::Arc::new(files.clone());
        let start = std::time::Instant::now();
        old_sort(&mut old);
        old_time += start.elapsed();
        let start = std::time::Instant::now();
        prepared.sort_files();
        prepared_time += start.elapsed();
        assert_eq!(
            old.files.iter().map(|file| &file.path).collect::<Vec<_>>(),
            prepared
                .files
                .iter()
                .map(|file| &file.path)
                .collect::<Vec<_>>()
        );
    }
    eprintln!(
        "5 name sorts of 20,000 entries: comparator={old_time:?}, prepared={prepared_time:?}"
    );
}
