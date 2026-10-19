use super::Source;
use oxyde_lib::models::FileEntry;
use std::sync::Arc;

#[test]
fn selection_follows_explorer_order_instead_of_hashset_order() {
    let source = Source {
        tab: "source".into(),
        selected: Arc::new(["c".into(), "a".into()].into()),
        files: Arc::new(
            ["a", "b", "c"]
                .map(|path| FileEntry {
                    path: path.into(),
                    ..Default::default()
                })
                .into(),
        ),
    };
    let selected = source.selected_files();
    assert_eq!(
        selected
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>(),
        ["a", "c"]
    );
}

#[test]
fn empty_selection_does_not_retain_a_removed_file() {
    let source = Source {
        files: Arc::new(vec![FileEntry {
            path: "removed".into(),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(source.selected_files().is_empty());
}

#[test]
fn source_revision_distinguishes_replacements_with_the_same_file_list() {
    let source = Source {
        tab: "explorer".into(),
        selected: Arc::new(["first".into()].into()),
        files: Arc::new(vec![FileEntry {
            path: "first".into(),
            ..Default::default()
        }]),
    };
    let captured = source.clone();
    let latest = Source {
        selected: Arc::new(["second".into()].into()),
        ..source
    };
    assert!(!latest.same_revision(&captured));
    assert!(latest.same_revision(&latest.clone()));
}
