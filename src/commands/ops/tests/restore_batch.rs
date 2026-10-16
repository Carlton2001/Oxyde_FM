use super::*;

fn item(name: &str) -> trash::TrashItem {
    trash::TrashItem { id: format!("/bin/{name}").into(), name: name.into(), original_parent: "/original".into(), time_deleted: 0 }
}

#[test]
fn failures_before_and_during_restore_do_not_block_independent_items() {
    let items = vec![item("bad"), item("good"), item("last")];
    let paths = vec!["/bin/missing", "/bin/bad", "/bin/good", "/bin/last"];
    let mut visited = Vec::new();
    let report = restore_selected(items, paths.into_iter().map(String::from).collect(), |item| {
        visited.push(item.name.clone());
        if item.name == "bad" { Err("native failure".into()) } else { Ok(()) }
    });
    assert_eq!(visited.len(), 3);
    assert_eq!(report.restored.len(), 2);
    assert_eq!(report.failures.len(), 2);
}

#[test]
fn ambiguous_original_does_not_block_exact_ids_and_aliases_are_deduplicated() {
    let first = item("same");
    let mut second = first.clone();
    second.id = "/bin/other".into();
    let mut calls = 0;
    let report = restore_selected(vec![first, second], vec!["/original/same".into(), "/bin/same".into(), "/bin/same".into(), "/bin/other".into()], |_| { calls += 1; Ok(()) });
    assert_eq!(calls, 2);
    assert_eq!(report.failures.len(), 1);
}

#[test]
fn original_path_and_id_restore_only_once_in_either_order() {
    for paths in [vec!["/original/file", "/bin/file"], vec!["/bin/file", "/original/file"]] {
        let mut calls = 0;
        let report = restore_selected(vec![item("file")], paths.into_iter().map(String::from).collect(), |_| { calls += 1; Ok(()) });
        assert_eq!(calls, 1);
        assert_eq!(report.restored.len(), 1);
        assert!(report.failures.is_empty());
    }
}
