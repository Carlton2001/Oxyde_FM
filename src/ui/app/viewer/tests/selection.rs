use super::Source;
use oxyde_lib::models::FileEntry;
use std::sync::Arc;

fn entries(names: &[&str]) -> Arc<Vec<FileEntry>> {
    Arc::new(
        names
            .iter()
            .map(|name| FileEntry {
                name: (*name).into(),
                path: (*name).into(),
                ..Default::default()
            })
            .collect(),
    )
}

#[test]
fn multiple_selection_shows_first_file_in_explorer_order() {
    let source = Source {
        tab: "source".into(),
        selected: Arc::new(["c".into(), "a".into()].into()),
        files: entries(&["a", "b", "c"]),
        ..Default::default()
    };
    assert_eq!(source.resolve().file.map(|file| file.path).as_deref(), Some("a"));
}

#[test]
fn multiple_selection_prefers_the_focused_file() {
    let source = Source {
        selected: Arc::new(["c".into(), "a".into()].into()),
        focus: Some("c".into()),
        files: entries(&["a", "b", "c"]),
        ..Default::default()
    };
    assert_eq!(source.resolve().file.map(|file| file.path).as_deref(), Some("c"));
}

#[test]
fn neighbors_skip_other_media_kinds() {
    let source = Source {
        selected: Arc::new(["b.png".into()].into()),
        files: entries(&["a.jpg", "clip.mp4", "notes.txt", "b.png", "c.mp4", "d.gif"]),
        ..Default::default()
    };
    let resolved = source.resolve();
    assert_eq!(resolved.previous.as_deref(), Some("a.jpg"));
    assert_eq!(resolved.next.as_deref(), Some("d.gif"));
    let text = Source { selected: Arc::new(["notes.txt".into()].into()), ..source };
    let resolved = text.resolve();
    assert!(resolved.previous.is_none() && resolved.next.is_none());
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
    assert!(source.resolve().file.is_none());
}

#[test]
fn source_revision_distinguishes_replacements_with_the_same_file_list() {
    let source = Source {
        tab: "explorer".into(),
        selected: Arc::new(["first".into()].into()),
        focus: None,
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
