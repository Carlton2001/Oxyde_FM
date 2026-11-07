use super::*;

fn entry(name: &str, is_dir: bool, size: u64, modified: u64) -> FileEntry {
    FileEntry { name: name.into(), path: format!("/f/{name}"), is_dir, size, modified, ..Default::default() }
}

fn names(entries: &[FileEntry], shown: &[usize]) -> Vec<String> {
    shown.iter().map(|&index| entries[index].name.clone()).collect()
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
fn folders_come_first_whatever_the_order() {
    let entries = sample();
    assert_eq!(names(&entries, &arrange(&entries, "", SortKey::Name, false)), ["archive", "Photos", "a.TXT", "b2.png", "b10.txt"]);
    assert_eq!(names(&entries, &arrange(&entries, "", SortKey::Name, true)), ["Photos", "archive", "b10.txt", "b2.png", "a.TXT"]);
}

#[test]
fn each_column_sorts_with_names_breaking_ties() {
    let entries = sample();
    assert_eq!(names(&entries, &arrange(&entries, "", SortKey::Size, false)), ["archive", "Photos", "b2.png", "a.TXT", "b10.txt"]);
    assert_eq!(names(&entries, &arrange(&entries, "", SortKey::Modified, true)), ["archive", "Photos", "a.TXT", "b10.txt", "b2.png"]);
    assert_eq!(names(&entries, &arrange(&entries, "", SortKey::Type, false)), ["archive", "Photos", "b2.png", "a.TXT", "b10.txt"]);
}

#[test]
fn the_filter_ignores_case_and_surrounding_spaces() {
    let entries = sample();
    assert_eq!(names(&entries, &arrange(&entries, " TXT ", SortKey::Name, false)), ["a.TXT", "b10.txt"]);
    assert!(arrange(&entries, "zzz", SortKey::Name, false).is_empty());
}

#[test]
fn typing_finds_the_first_name_with_that_start() {
    let entries = sample();
    let shown = arrange(&entries, "", SortKey::Name, false);
    assert_eq!(find_prefix(&entries, &shown, "B"), Some(3));
    assert_eq!(find_prefix(&entries, &shown, "ph"), Some(1));
    assert_eq!(find_prefix(&entries, &shown, "x"), None);
}

#[test]
fn options_survive_the_preferences_and_default_when_unreadable() {
    let options = Options { grid: true, sort: SortKey::Modified, descending: true, folder: Some("/home/a".into()) };
    let value = options.to_value().unwrap();
    assert_eq!(value["sort"], "modified");
    assert_eq!(Options::from_value(Some(&value)), options);
    assert_eq!(Options::from_value(Some(&serde_json::json!({"sort": "nonsense"}))), Options::default());
    assert_eq!(Options::from_value(Some(&serde_json::json!({"grid": true}))).grid, true);
}
