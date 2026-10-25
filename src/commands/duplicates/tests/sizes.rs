use super::*;

#[test]
fn name_only_groups_keep_each_real_size_and_a_deterministic_minimum() {
    let root = tempfile::tempdir().unwrap();
    for (name, size) in [("a", 100), ("b", 1)] {
        let directory = root.path().join(name);
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("same.txt"), vec![0; size]).unwrap();
    }
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let groups = runtime.block_on(find_duplicates(
        &DuplicateSearchState::new(), vec![root.path().to_string_lossy().into_owned()],
        DuplicateSearchOptions { by_name: true, by_size: false, by_content: false },
    )).unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].size, 1);
    assert_eq!(groups[0].files.iter().map(|file| file.size).collect::<Vec<_>>(), [100, 1]);
}
