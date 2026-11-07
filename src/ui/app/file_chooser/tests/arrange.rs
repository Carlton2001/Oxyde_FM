use super::{arrange, sidebar_width, Options};
use crate::ui::workspace::{SortColumn, SortDirection};
use oxyde_lib::models::FileEntry;

fn entry(name: &str, is_dir: bool, size: u64, modified: u64) -> FileEntry {
    FileEntry { name: name.into(), path: format!("/f/{name}"), is_dir, size, modified, ..Default::default() }
}

fn names(entries: &[FileEntry]) -> Vec<String> {
    entries.iter().map(|entry| entry.name.clone()).collect()
}

fn sample() -> Vec<FileEntry> {
    vec![
        entry("b10.txt", false, 30, 3),
        entry("Photos", true, 0, 1),
        entry("b2.png", false, 10, 2),
        entry("archive", true, 0, 5),
        entry("a.TXT", false, 20, 9),
    ]
}

#[test]
fn folders_stay_first_in_the_main_windows_order() {
    let entries = sample();
    let ascending = arrange(&entries, "", SortColumn::Name, SortDirection::Ascending);
    assert_eq!(names(&ascending.shown), ["archive", "Photos", "a.TXT", "b2.png", "b10.txt"]);
    assert_eq!(ascending.folders, 2);
    let descending = arrange(&entries, "", SortColumn::Name, SortDirection::Descending);
    assert_eq!(names(&descending.shown), ["Photos", "archive", "b10.txt", "b2.png", "a.TXT"]);
    let size = arrange(&entries, "", SortColumn::Size, SortDirection::Ascending);
    assert_eq!(names(&size.shown), ["archive", "Photos", "b2.png", "a.TXT", "b10.txt"]);
}

#[test]
fn the_filter_matches_names_without_regard_to_case_and_columns_measure_everything() {
    let entries = sample();
    let filtered = arrange(&entries, "TXT", SortColumn::Name, SortDirection::Ascending);
    assert_eq!(names(&filtered.shown), ["a.TXT", "b10.txt"]);
    assert_eq!(filtered.folders, 0);
    assert_eq!(filtered.sizes.len(), 4);
    assert!(arrange(&entries, "zzz", SortColumn::Name, SortDirection::Ascending).shown.is_empty());
}

#[test]
fn options_survive_the_preferences_and_default_when_unreadable() {
    let options = Options { grid: true, sort: SortColumn::Modified, direction: SortDirection::Descending, folder: Some("/home/a".into()), sidebar: Some(212.5) };
    let value = options.to_value().unwrap();
    assert_eq!(value["sort"], "modified");
    assert_eq!(Options::from_value(Some(&value)), options);
    assert_eq!(Options::from_value(Some(&serde_json::json!({"sort": "nonsense"}))), Options::default());
    assert!(Options::from_value(Some(&serde_json::json!({"grid": true}))).grid);
}

#[test]
fn the_sidebar_resizes_within_the_main_bounds_and_leaves_the_pane_its_minimum() {
    assert_eq!(sidebar_width(300.0, 1200.0), 300.0);
    assert_eq!(sidebar_width(40.0, 1200.0), 150.0);
    assert_eq!(sidebar_width(900.0, 2000.0), 600.0);
    assert_eq!(sidebar_width(450.0, 1000.0), 300.0);
    // A window at its minimum keeps the smallest sidebar.
    assert_eq!(sidebar_width(400.0, 800.0), 150.0);
}
