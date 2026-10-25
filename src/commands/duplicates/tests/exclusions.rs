use super::*;

fn search(paths: Vec<PathBuf>, criteria: u8) -> (Vec<DuplicateGroup>, usize) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let state = DuplicateSearchState::new();
    let groups = runtime
        .block_on(find_duplicates(
            &state,
            paths
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            DuplicateSearchOptions {
                by_name: criteria & 1 != 0,
                by_size: criteria & 2 != 0,
                by_content: criteria & 4 != 0,
            },
        ))
        .unwrap();
    let scanned = state.1.lock().unwrap().current;
    (groups, scanned)
}

#[test]
fn recycle_bins_are_pruned_for_every_combination_of_criteria() {
    let root = tempfile::tempdir().unwrap();
    let visible = root.path().join("documents/same.txt");
    std::fs::create_dir_all(visible.parent().unwrap()).unwrap();
    std::fs::write(&visible, b"identical").unwrap();
    for bin in ["$Recycle.Bin", "nested/$rEcYcLe.bIn"] {
        let directory = root.path().join(bin).join("S-1-5-21/deleted");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("same.txt"), b"identical").unwrap();
        std::fs::write(directory.join("metadata"), b"identical").unwrap();
    }

    for criteria in 1..8 {
        let (groups, scanned) = search(vec![root.path().to_path_buf()], criteria);
        assert!(groups.is_empty(), "criteria: {criteria}");
        assert_eq!(scanned, 1, "recycle bin contents must not be scanned");
    }
}

#[test]
fn explicit_recycle_bin_roots_and_descendants_are_ignored() {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("$RECYCLE.BIN");
    let deleted = bin.join("S-1-5-21/deleted");
    std::fs::create_dir_all(&deleted).unwrap();
    std::fs::write(deleted.join("a.txt"), b"identical").unwrap();
    std::fs::write(deleted.join("b.txt"), b"identical").unwrap();

    for selected in [bin, deleted] {
        let (groups, scanned) = search(vec![selected], 6);
        assert!(groups.is_empty());
        assert_eq!(scanned, 0);
    }
}

#[test]
fn similarly_named_directories_still_participate_in_duplicate_search() {
    let root = tempfile::tempdir().unwrap();
    for directory in ["$Recycle.Bin-backup", "prefix-$Recycle.Bin", "documents"] {
        let path = root.path().join(directory);
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("same.txt"), b"identical").unwrap();
    }

    for criteria in 1..8 {
        let (groups, _) = search(vec![root.path().to_path_buf()], criteria);
        assert_eq!(groups.len(), 1, "criteria: {criteria}");
        assert_eq!(groups[0].files.len(), 3);
    }
}
