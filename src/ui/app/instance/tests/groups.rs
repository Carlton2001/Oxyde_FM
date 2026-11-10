use super::{by_folder, shows};

#[test]
fn items_open_one_tab_per_folder_in_their_order() {
    let items = ["/b/one", "/a/two", "/b/three"].map(String::from).to_vec();
    assert_eq!(by_folder(items), [
        ("/b".to_string(), vec!["/b/one".to_string(), "/b/three".to_string()]),
        ("/a".to_string(), vec!["/a/two".to_string()]),
    ]);
}

#[test]
fn a_tab_shows_a_folder_whatever_its_trailing_separator() {
    assert!(shows("/home/me/", "/home/me"));
    assert!(shows("/", "/"));
    assert!(!shows("/home/me", "/home/me2"));
    assert_eq!(shows(r"C:\Users\Me", r"c:\users\me\"), cfg!(target_os = "windows"));
}
