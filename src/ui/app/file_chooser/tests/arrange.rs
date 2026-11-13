use super::{arrange, sidebar_width, Admission, Options};
use oxyde_lib::platform::{FileTypeFilter, FileTypePattern};
use std::collections::HashMap;
use std::sync::Arc;
use crate::ui::workspace::{SortColumn, SortDirection};
use oxyde_lib::models::FileEntry;

fn entry(name: &str, is_dir: bool, size: u64, modified: u64) -> FileEntry {
    FileEntry { name: name.into(), path: format!("/f/{name}"), is_dir, size, modified, ..Default::default() }
}

fn names(entries: &[FileEntry]) -> Vec<String> {
    entries.iter().map(|entry| entry.name.clone()).collect()
}

fn no_mime(_: &str, _: &str) -> bool {
    false
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
    let ascending = arrange(&entries, "", &Admission::default(), &no_mime, SortColumn::Name, SortDirection::Ascending);
    assert_eq!(names(&ascending.shown), ["archive", "Photos", "a.TXT", "b2.png", "b10.txt"]);
    assert_eq!(ascending.folders, 2);
    let descending = arrange(&entries, "", &Admission::default(), &no_mime, SortColumn::Name, SortDirection::Descending);
    assert_eq!(names(&descending.shown), ["Photos", "archive", "b10.txt", "b2.png", "a.TXT"]);
    let size = arrange(&entries, "", &Admission::default(), &no_mime, SortColumn::Size, SortDirection::Ascending);
    assert_eq!(names(&size.shown), ["archive", "Photos", "b2.png", "a.TXT", "b10.txt"]);
}

#[test]
fn the_filter_matches_names_without_regard_to_case_and_columns_measure_everything() {
    let entries = sample();
    let filtered = arrange(&entries, "TXT", &Admission::default(), &no_mime, SortColumn::Name, SortDirection::Ascending);
    assert_eq!(names(&filtered.shown), ["a.TXT", "b10.txt"]);
    assert_eq!(filtered.folders, 0);
    assert_eq!(filtered.sizes.len(), 4);
    assert!(arrange(&entries, "zzz", &Admission::default(), &no_mime, SortColumn::Name, SortDirection::Ascending).shown.is_empty());
}

#[test]
fn options_survive_the_preferences_and_default_when_unreadable() {
    let options = Options { grid: true, sort: SortColumn::Modified, direction: SortDirection::Descending, folder: Some("/home/a".into()), sidebar: Some(212.5), clients: vec![("app".into(), "/home/a/b".into())] };
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

#[test]
fn the_request_admits_folders_always_and_files_of_the_type_shown() {
    let entries = sample();
    let folders = Admission { folders_only: true, ..Default::default() };
    let shown = arrange(&entries, "", &folders, &no_mime, SortColumn::Name, SortDirection::Ascending);
    assert_eq!(names(&shown.shown), ["archive", "Photos"]);
    // Shell patterns keep case, as GTK's do, except on Windows, where names have
    // none; MIME types come from the files read.
    let filter = FileTypeFilter { name: "Text".into(), patterns: vec![
        FileTypePattern::Glob("*.txt".into()),
        FileTypePattern::MimeType("image/*".into()),
    ] };
    let content_types = Arc::new(HashMap::from([("/f/b2.png".to_owned(), "image/png"), ("/f/a.TXT".to_owned(), "text/plain")]));
    let typed = Admission { types: Some(filter), content_types, ..Default::default() };
    let media = |mime: &str, pattern: &str| pattern.strip_suffix("/*").is_some_and(|media| mime.starts_with(media));
    let shown = arrange(&entries, "", &typed, &media, SortColumn::Name, SortDirection::Ascending);
    if cfg!(target_os = "windows") {
        assert_eq!(names(&shown.shown), ["archive", "Photos", "a.TXT", "b2.png", "b10.txt"]);
    } else {
        assert_eq!(names(&shown.shown), ["archive", "Photos", "b2.png", "b10.txt"]);
    }
    // A filter without rules lets no file through.
    let empty = Admission { types: Some(FileTypeFilter { name: "None".into(), patterns: Vec::new() }), ..Default::default() };
    assert_eq!(arrange(&entries, "", &empty, &no_mime, SortColumn::Name, SortDirection::Ascending).folders, 2);
    assert_eq!(arrange(&entries, "", &empty, &no_mime, SortColumn::Name, SortDirection::Ascending).shown.len(), 2);
}

#[test]
fn each_application_finds_its_own_last_folder_the_latest_first() {
    let mut options = Options::default();
    options.remember_client("a", "/one");
    options.remember_client("b", "/two");
    options.remember_client("a", "/three");
    assert_eq!(options.client_folder("a").as_deref(), Some("/three"));
    assert_eq!(options.client_folder("b").as_deref(), Some("/two"));
    assert_eq!(options.client_folder("c"), None);
    assert_eq!(options.clients.first().map(|(client, _)| client.as_str()), Some("a"));
    for client in 0..40 {
        options.remember_client(&client.to_string(), "/x");
    }
    assert_eq!(options.clients.len(), 32);
    assert_eq!(options.client_folder("a"), None);
}
