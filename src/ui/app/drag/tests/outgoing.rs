use super::*;

fn offered(paths: &[&str]) -> Option<Vec<PathBuf>> {
    offered_paths(&paths.iter().map(|path| path.to_string()).collect::<Vec<_>>())
}

#[test]
fn real_paths_are_offered_in_order() {
    assert_eq!(offered(&["/a/b.txt", "/a/Été 2026", "/a/c.zip"]),
        Some(vec!["/a/b.txt".into(), "/a/Été 2026".into(), "/a/c.zip".into()]));
}

#[test]
fn one_path_that_cannot_be_offered_refuses_the_whole_drag() {
    assert_eq!(offered(&["/a/b.txt", "/a/c.zip/member.txt"]), None);
    assert_eq!(offered(&["/a/b.txt", "smb://server/share"]), None);
    assert_eq!(offered(&["/a/b.txt", "relative.txt"]), None);
    assert_eq!(offered(&["/a/b.txt", ""]), None);
    use std::os::unix::ffi::OsStrExt;
    let encoded = oxyde_lib::utils::path_text::encode(Path::new(std::ffi::OsStr::from_bytes(b"/a/\xe9")));
    assert_eq!(offered(&[&encoded]), None);
}
