use super::*;

#[test]
fn natural_names_cover_numbers_unicode_and_stable_case_ties() {
    let expected = [
        "fichier2",
        "FICHIER2",
        "fichier02",
        "fichier002",
        "fichier10",
        "fichier99999999999999999999",
        "fichier100000000000000000000",
        "version2-part2",
        "version2-part10",
        "École2",
        "école10",
    ];
    for listed in [false, true] {
        for direction in [SortDirection::Ascending, SortDirection::Descending] {
            for column in [SortColumn::Name, SortColumn::Size, SortColumn::Type] {
                let mut files: Vec<_> = expected
                    .iter()
                    .rev()
                    .map(|name| FileEntry {
                        name: (*name).into(),
                        ..Default::default()
                    })
                    .collect();
                files.push(FileEntry {
                    name: "folder10".into(),
                    is_dir: true,
                    ..Default::default()
                });
                files.push(FileEntry {
                    name: "folder2".into(),
                    is_dir: true,
                    ..Default::default()
                });
                sort(&mut files, column, direction, listed);
                let mut names = expected.to_vec();
                // Input case ties retain their arrival order.
                names.swap(0, 1);
                let folders = match direction {
                    SortDirection::Ascending => ["folder2", "folder10"],
                    SortDirection::Descending => {
                        names.reverse();
                        let len = names.len();
                        names.swap(len - 1, len - 2);
                        ["folder10", "folder2"]
                    }
                };
                let wanted: Vec<_> = folders.into_iter().chain(names).collect();
                assert_eq!(
                    files
                        .iter()
                        .map(|file| file.name.as_str())
                        .collect::<Vec<_>>(),
                    wanted,
                    "{column:?}, {direction:?}, listed={listed}"
                );
            }
        }
    }
}
