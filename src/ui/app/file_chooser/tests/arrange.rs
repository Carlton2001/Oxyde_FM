use super::{arrange, arrow_target, find_match, Options};
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
fn type_to_select_cycles_a_repeated_letter_and_keeps_a_matching_item() {
    let names = ["alpha", "beta", "bravo", "charlie"];
    assert_eq!(find_match(&names, None, "b"), Some(1));
    assert_eq!(find_match(&names, Some(1), "b"), Some(2));
    assert_eq!(find_match(&names, Some(2), "bb"), Some(1));
    assert_eq!(find_match(&names, Some(1), "be"), Some(1));
    assert_eq!(find_match(&names, None, "x"), None);
}

#[test]
fn arrows_follow_rows_and_start_at_the_first_entry() {
    let rows: Vec<&[usize]> = vec![&[0, 1, 2], &[3, 4, 5], &[6]];
    assert_eq!(arrow_target(&rows, None, "up"), Some(0));
    assert_eq!(arrow_target(&rows, Some(1), "down"), Some(4));
    assert_eq!(arrow_target(&rows, Some(5), "down"), Some(6));
    assert_eq!(arrow_target(&rows, Some(2), "right"), Some(2));
    assert_eq!(arrow_target(&[], None, "down"), None);
}

#[test]
fn options_survive_the_preferences_and_default_when_unreadable() {
    let options = Options { grid: true, sort: SortColumn::Modified, direction: SortDirection::Descending, folder: Some("/home/a".into()) };
    let value = options.to_value().unwrap();
    assert_eq!(value["sort"], "modified");
    assert_eq!(Options::from_value(Some(&value)), options);
    assert_eq!(Options::from_value(Some(&serde_json::json!({"sort": "nonsense"}))), Options::default());
    assert!(Options::from_value(Some(&serde_json::json!({"grid": true}))).grid);
}
