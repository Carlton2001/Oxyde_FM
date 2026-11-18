use super::*;

#[test]
fn copies_paste_again_cuts_paste_once_and_text_copied_elsewhere_replaces_them() {
    hold(vec!["/home/user/a".into()], false);
    for _ in 0..2 {
        assert_eq!(current(Some("/home/user/a")), Some((vec!["/home/user/a".into()], false)));
    }
    hold(vec!["/home/user/b".into(), "/home/user/c".into()], true);
    assert!(LinuxFileClipboard.has_file_format());
    let both = Some("/home/user/b\n/home/user/c");
    assert_eq!(current(both), Some((vec!["/home/user/b".into(), "/home/user/c".into()], true)));
    assert!(!LinuxFileClipboard.has_file_format());
    hold(vec!["/home/user/d".into()], false);
    assert_eq!(current(Some("copied elsewhere")), None);
    assert!(!LinuxFileClipboard.has_file_format());
}

#[test]
fn files_other_managers_offer_are_read_in_the_gnome_format() {
    let paths = vec!["/home/user/Été 2026/a#1.txt".to_string(), "/tmp/b".to_string()];
    let text = "cut\nfile:///home/user/%C3%89t%C3%A9%202026/a%231.txt\nfile:///tmp/b";
    assert_eq!(wayland::decode(text), Some((paths, true)));
    assert_eq!(wayland::decode("copy\r\nfile:///tmp/b\r\n"), Some((vec!["/tmp/b".into()], false)));
    for text in ["", "copy", "move\nfile:///tmp/b", "copy\nhttps://example.com/b"] {
        assert_eq!(wayland::decode(text), None, "{text:?}");
    }
}

#[test]
fn trashed_items_are_offered_as_text_alone() {
    let trash = dirs::data_dir().unwrap().join("Trash/files/doc.txt").to_string_lossy().into_owned();
    std::fs::create_dir_all(dirs::data_dir().unwrap().join("Trash/info")).unwrap();
    assert!(!LinuxFileClipboard.interface_offers_files(&[trash]));
    assert_eq!(LinuxFileClipboard.interface_offers_files(&["/tmp/a".into()]), wayland::session());
}

#[test]
fn outside_wayland_wl_paste_is_not_asked() {
    if wayland::session() {
        return;
    }
    assert_eq!(wayland::files(), None);
}
