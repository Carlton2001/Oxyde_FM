use super::*;

#[test]
fn copies_paste_again_cuts_paste_once_and_discarding_empties() {
    let clipboard = LinuxFileClipboard;
    clipboard.set_files(vec!["/home/user/a".into()], false).unwrap();
    for _ in 0..2 {
        assert_eq!(clipboard.get_files().unwrap(), (vec!["/home/user/a".into()], false));
    }
    clipboard.set_files(vec!["/home/user/b".into()], true).unwrap();
    assert!(clipboard.has_file_format());
    assert_eq!(clipboard.get_files().unwrap(), (vec!["/home/user/b".into()], true));
    assert!(!clipboard.has_file_format());
    clipboard.set_files(vec!["/home/user/c".into()], false).unwrap();
    clipboard.discard();
    assert_eq!(clipboard.get_files().unwrap(), (Vec::new(), false));
}
