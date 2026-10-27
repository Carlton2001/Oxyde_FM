use super::*;

#[test]
fn copies_paste_again_cuts_paste_once_and_text_copied_elsewhere_replaces_them() {
    hold(vec!["/home/user/a".into()], false, None);
    for _ in 0..2 {
        assert_eq!(current(Some("/home/user/a")), Some((vec!["/home/user/a".into()], false)));
    }
    hold(vec!["/home/user/b".into(), "/home/user/c".into()], true, None);
    assert!(LinuxFileClipboard.has_file_format());
    let both = Some("/home/user/b\n/home/user/c");
    assert_eq!(current(both), Some((vec!["/home/user/b".into(), "/home/user/c".into()], true)));
    assert!(!LinuxFileClipboard.has_file_format());
    hold(vec!["/home/user/d".into()], false, None);
    assert_eq!(current(Some("copied elsewhere")), None);
    assert!(!LinuxFileClipboard.has_file_format());
}

#[test]
fn files_use_the_gnome_clipboard_format() {
    let paths = vec!["/home/user/Été 2026/a#1.txt".to_string(), "/tmp/b".to_string()];
    let text = wayland::encode(&paths, true);
    assert_eq!(text, "cut\nfile:///home/user/%C3%89t%C3%A9%202026/a%231.txt\nfile:///tmp/b");
    assert_eq!(wayland::decode(&text), Some((paths, true)));
    assert_eq!(wayland::decode("copy\r\nfile:///tmp/b\r\n"), Some((vec!["/tmp/b".into()], false)));
    for text in ["", "copy", "move\nfile:///tmp/b", "copy\nhttps://example.com/b"] {
        assert_eq!(wayland::decode(text), None, "{text:?}");
    }
}
