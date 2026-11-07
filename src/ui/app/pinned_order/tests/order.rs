use super::{apply_order, moved};
use oxyde_lib::models::QuickAccessItem;

fn items(paths: &[&str]) -> Vec<QuickAccessItem> {
    paths.iter().map(|path| QuickAccessItem { name: path.to_string(), path: path.to_string() }).collect()
}

fn paths(items: &[QuickAccessItem]) -> Vec<&str> {
    items.iter().map(|item| item.path.as_str()).collect()
}

#[test]
fn saved_order_leads_and_new_pins_keep_the_system_order_after_it() {
    let order = vec!["/c".to_string(), "/gone".to_string(), "/a".to_string()];
    let ordered = apply_order(items(&["/a", "/b", "/c", "/d"]), &order);
    assert_eq!(paths(&ordered), ["/c", "/a", "/b", "/d"]);
}

#[test]
fn moves_before_or_after_the_target() {
    let list = items(&["/a", "/b", "/c"]);
    assert_eq!(paths(&moved(&list, "/c", "/a", false).unwrap()), ["/c", "/a", "/b"]);
    assert_eq!(paths(&moved(&list, "/a", "/c", true).unwrap()), ["/b", "/c", "/a"]);
    assert_eq!(paths(&moved(&list, "/a", "/b", true).unwrap()), ["/b", "/a", "/c"]);
}

#[test]
fn a_drop_in_place_changes_nothing() {
    let list = items(&["/a", "/b", "/c"]);
    assert!(moved(&list, "/b", "/b", false).is_none());
    assert!(moved(&list, "/b", "/a", true).is_none());
    assert!(moved(&list, "/b", "/c", false).is_none());
}
