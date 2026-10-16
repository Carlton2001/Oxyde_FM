use super::*;

#[test]
fn indexed_overlap_matches_the_original_scan_in_different_orders() {
    let root = PathBuf::from("root");
    let candidates: Vec<_> = [
        "a", "a/b", "a/b/c", "a!", "a-", "a.", "a0", "ab", "ab/c", "b", "b/a", "b/A", "B", "b",
        "c/./d", "c/d", "c/d/e", "c",
    ]
    .into_iter()
    .map(|name| root.join(name))
    .chain([root.clone()])
    .collect();
    for seed in 0..64_u64 {
        let mut order = candidates.clone();
        let mut state = seed + 1;
        for index in (1..order.len()).rev() {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            order.swap(index, (state as usize) % (index + 1));
        }
        let mut expected: Vec<PathBuf> = Vec::new();
        let mut index = SourcePathSet::default();
        for path in order {
            let accepted = !expected
                .iter()
                .any(|existing| path.starts_with(existing) || existing.starts_with(&path));
            assert_eq!(
                index.insert(path.clone()),
                accepted,
                "seed={seed}, path={path:?}"
            );
            if accepted {
                expected.push(path);
            }
            assert_eq!(index.paths.len(), expected.len());
        }
    }
}

#[test]
fn flat_selection_accepts_siblings_but_rejects_their_parent_and_duplicates() {
    let mut paths = SourcePathSet::default();
    let root = PathBuf::from("selected");
    for number in 0..20_000 {
        assert!(paths.insert(root.join(format!("file-{number:05}"))));
    }
    assert!(!paths.insert(root.clone()));
    assert!(!paths.insert(root.join("file-19999")));
    assert!(!paths.insert(root.join("file-00000/child")));
    assert!(paths.insert(root.join("file-20000")));
}

#[cfg(target_os = "windows")]
#[test]
fn drive_and_unc_paths_keep_the_original_case_and_component_rules() {
    let candidates = [
        r"\\?\D:\folder\file",
        r"\\?\D:\folder",
        r"\\?\D:\Folder",
        r"\\?\E:\folder",
        r"\\server\share\folder",
        r"\\server\share\folder\child",
        r"\\server\share\folder-other",
    ];
    for reverse in [false, true] {
        let mut order: Vec<PathBuf> = candidates.iter().map(PathBuf::from).collect();
        if reverse {
            order.reverse();
        }
        let mut expected: Vec<PathBuf> = Vec::new();
        let mut paths = SourcePathSet::default();
        for path in order {
            let accepted = !expected
                .iter()
                .any(|existing| path.starts_with(existing) || existing.starts_with(&path));
            assert_eq!(paths.insert(path.clone()), accepted, "{path:?}");
            if accepted {
                expected.push(path);
            }
        }
    }
}
