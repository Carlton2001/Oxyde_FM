use super::actionable_drop_paths;

#[test]
fn self_and_same_directory_drops_do_nothing() {
    let root = std::env::temp_dir().join("oxyde-drop");
    let item = root.join("folder");
    let path = item.to_string_lossy().into_owned();
    assert!(actionable_drop_paths(vec![path.clone()], &path).is_empty());
    assert!(actionable_drop_paths(vec![path], &root.to_string_lossy()).is_empty());
}

#[test]
fn mixed_drop_keeps_only_items_that_change_directory() {
    let root = std::env::temp_dir().join("oxyde-drop");
    let target = root.join("target");
    let foreign = root
        .join("other")
        .join("file")
        .to_string_lossy()
        .into_owned();
    let local = target.join("file").to_string_lossy().into_owned();
    assert_eq!(
        actionable_drop_paths(vec![local, foreign.clone()], &target.to_string_lossy()),
        vec![foreign]
    );
}

#[cfg(target_os = "windows")]
#[test]
fn windows_self_drop_ignores_case_and_separator_variants() {
    assert!(actionable_drop_paths(vec!["C:\\Folder\\Item".into()], "c:/folder/item/").is_empty());
}
