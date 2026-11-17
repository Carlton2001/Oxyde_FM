use super::*;
use std::sync::Arc;

fn entry(path: &str) -> FileEntry {
    FileEntry { path: path.into(), name: path.rsplit('/').next().unwrap().into(), ..Default::default() }
}

fn tab() -> TabState {
    let mut tab = TabState::new("test".into(), "C:/folder".into());
    tab.files = Arc::new(["C:/folder/a.txt", "C:/folder/b.txt", "C:/folder/c.log"].map(entry).to_vec());
    tab.update_cache();
    tab
}

#[test]
fn a_filter_hides_selected_entries_from_actions() {
    let mut tab = tab();
    tab.selected_paths = tab.files.iter().map(|file| file.path.clone()).collect();
    tab.filter_name = "a.txt".into();
    tab.update_cache();
    // Ctrl+A, then a name filter: only the visible entry is deleted, copied or cut.
    assert_eq!(tab.actionable_selection(), ["C:/folder/a.txt"]);
    assert_eq!(tab.selected_paths.len(), 3);
}

#[test]
fn a_vanished_entry_is_never_acted_on_and_order_follows_the_list() {
    let mut tab = tab();
    tab.selected_paths = ["C:/folder/c.log", "C:/folder/a.txt", "C:/folder/gone.txt"].map(String::from).into();
    assert_eq!(tab.actionable_selection(), ["C:/folder/a.txt", "C:/folder/c.log"]);
}
