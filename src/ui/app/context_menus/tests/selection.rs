use super::action_paths;
use std::collections::HashSet;

#[test]
fn clicking_a_selected_item_targets_the_whole_selection_in_stable_order() {
    let selection = HashSet::from(["C:/b.txt".into(), "C:/a.txt".into(), "D:/c.txt".into()]);
    assert_eq!(action_paths("C:/b.txt", Some(&selection)),
        vec!["C:/a.txt", "C:/b.txt", "D:/c.txt"]);
}

#[test]
fn clicking_outside_the_selection_targets_only_the_clicked_item() {
    let selection = HashSet::from(["C:/a.txt".into(), "C:/b.txt".into()]);
    assert_eq!(action_paths("C:/other.txt", Some(&selection)), vec!["C:/other.txt"]);
}

#[test]
fn empty_or_missing_selection_keeps_the_clicked_item() {
    assert_eq!(action_paths("C:/a.txt", Some(&HashSet::new())), vec!["C:/a.txt"]);
    assert_eq!(action_paths("C:/a.txt", None), vec!["C:/a.txt"]);
}
