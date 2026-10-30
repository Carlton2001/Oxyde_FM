use super::*;
use std::os::unix::ffi::OsStringExt;

fn given(arguments: &[&str], current: &Path) -> Vec<String> {
    locations(arguments.iter().map(OsString::from), current)
}

#[test]
fn folders_open_as_given_and_files_stand_for_their_folder() {
    let root = tempfile::tempdir().unwrap();
    let base = root.path().to_str().unwrap();
    std::fs::create_dir_all(root.path().join("Été 2026/sub")).unwrap();
    std::fs::write(root.path().join("Été 2026/note.txt"), b"").unwrap();
    let folder = format!("{base}/Été 2026");
    assert_eq!(given(&[&folder], Path::new("/")), [folder.clone()]);
    assert_eq!(given(&[&format!("{folder}/")], Path::new("/")), [folder.clone()]);
    assert_eq!(given(&[&format!("{folder}/note.txt")], Path::new("/")), [folder.clone()]);
    assert_eq!(given(&["."], Path::new(&folder)), [folder.clone()]);
    assert_eq!(given(&["sub/.."], Path::new(&folder)), [std::fs::canonicalize(&folder).unwrap().to_str().unwrap()]);
}

#[test]
fn file_uris_are_decoded_and_other_schemes_stay_uris() {
    let root = tempfile::tempdir().unwrap();
    let base = root.path().to_str().unwrap();
    std::fs::create_dir(root.path().join("a b")).unwrap();
    let expected = format!("{base}/a b");
    assert_eq!(given(&[&format!("file://{base}/a%20b")], Path::new("/")), [expected.clone()]);
    assert_eq!(given(&[&format!("file://localhost{base}/a%20b")], Path::new("/")), [expected]);
    assert_eq!(given(&["smb://server/share"], Path::new("/")), ["smb://server/share"]);
    assert!(given(&["file://server/share"], Path::new("/")).is_empty());
}

#[test]
fn what_cannot_be_opened_is_left_out() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing");
    assert!(given(&[missing.to_str().unwrap(), "--unknown"], root.path()).is_empty());
    let not_utf8 = OsString::from_vec(vec![b'/', 0xff]);
    assert!(locations([not_utf8].into_iter(), Path::new("/")).is_empty());
    assert!(given(&["file:///tmp/%FF"], Path::new("/")).is_empty());
}
