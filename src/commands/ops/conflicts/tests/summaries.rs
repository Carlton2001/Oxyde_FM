use super::*;

#[cfg(windows)]
#[test]
fn junction_roots_and_nested_links_keep_walkdir_totals() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("source");
    let witness = fixture.path().join("witness");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&witness).unwrap();
    std::fs::write(witness.join("payload"), [42; 73]).unwrap();
    let link = root.join("link");
    crate::platform::windows_reparse_tests::junction(&link, &witness);
    for paths in [[&root, &link], [&link, &root]] {
        let mut cache = TreeSummaries::default();
        for path in paths {
            let actual = cache.get(path);
            let mut expected = Totals::default();
            for entry in walkdir::WalkDir::new(path)
                .into_iter()
                .filter_map(Result::ok)
            {
                if entry.file_type().is_dir() {
                    expected.directories += 1;
                } else {
                    expected.files += 1;
                    expected.bytes += entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
                }
            }
            assert_eq!(
                (actual.bytes, actual.directories, actual.files),
                (expected.bytes, expected.directories, expected.files)
            );
        }
    }
    std::fs::remove_dir(&link).unwrap();
    assert_eq!(std::fs::read(witness.join("payload")).unwrap(), [42; 73]);
}

#[test]
fn nested_conflicts_visit_each_entry_once_and_keep_legacy_totals() {
    let root = tempfile::tempdir().unwrap();
    let mut path = root.path().to_path_buf();
    let mut directories = vec![path.clone()];
    for index in 0..24 {
        path = path.join(format!("d{index}"));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("payload"), [42; 7]).unwrap();
        directories.push(path.clone());
    }
    let mut cache = TreeSummaries::default();
    let mut legacy_visits = 0;
    for directory in &directories {
        let actual = cache.get(directory);
        let mut expected = Totals::default();
        for entry in walkdir::WalkDir::new(directory)
            .into_iter()
            .filter_map(Result::ok)
        {
            legacy_visits += 1;
            expected.add(if entry.file_type().is_dir() {
                Totals {
                    directories: 1,
                    ..Default::default()
                }
            } else {
                Totals {
                    files: 1,
                    bytes: entry.metadata().unwrap().len(),
                    directories: 0,
                }
            });
        }
        assert_eq!(
            (actual.bytes, actual.directories, actual.files),
            (expected.bytes, expected.directories, expected.files)
        );
    }
    assert_eq!(cache.visits, 49);
    eprintln!(
        "conflict totals: legacy_visits={legacy_visits}; aggregate_visits={}",
        cache.visits
    );
    let mut reverse = TreeSummaries::default();
    for directory in directories.iter().rev() {
        let actual = reverse.get(directory);
        let expected = cache.get(directory);
        assert_eq!(
            (actual.bytes, actual.directories, actual.files),
            (expected.bytes, expected.directories, expected.files)
        );
    }
    assert!(reverse.visits <= 2 * cache.visits);
    std::fs::write(path.join("payload"), [42; 14]).unwrap();
    let mut revalidation = TreeSummaries::default();
    assert_eq!(revalidation.get(root.path()).bytes, 24 * 7 + 7);
}
