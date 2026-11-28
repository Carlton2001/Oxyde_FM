use super::*;

#[test]
fn a_mutable_access_takes_a_new_stamp_and_reads_keep_it() {
    let mut selection = SelectedPaths::from(["C:/a.txt".to_string()]);
    let first = selection.stamp();
    assert!(selection.contains("C:/a.txt") && selection.len() == 1);
    assert_eq!(selection.stamp(), first);
    assert_eq!(selection.clone().stamp(), first);
    selection.insert("C:/b.txt".into());
    let second = selection.stamp();
    assert_ne!(second, first);
    selection.clear();
    assert_ne!(selection.stamp(), second);
}

#[test]
fn a_set_put_in_place_of_another_never_reuses_its_stamp() {
    let mut selection = SelectedPaths::default();
    let replaced = selection.stamp();
    selection = SelectedPaths::default();
    assert_ne!(selection.stamp(), replaced);
    let collected: SelectedPaths = ["C:/a.txt".to_string()].into_iter().collect();
    assert_ne!(collected.stamp(), selection.stamp());
}
