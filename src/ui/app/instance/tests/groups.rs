use super::by_folder;

#[test]
fn items_open_one_tab_per_folder_in_their_order() {
    let items = ["/b/one", "/a/two", "/b/three"].map(String::from).to_vec();
    assert_eq!(by_folder(items), [
        ("/b".to_string(), vec!["/b/one".to_string(), "/b/three".to_string()]),
        ("/a".to_string(), vec!["/a/two".to_string()]),
    ]);
}
