use super::{path_within, paths_match};
use std::path::Path;

#[test]
fn paths_match_respects_platform_case_and_separators() {
    assert!(paths_match("/home/user/", "/home/user"));
    assert!(!paths_match("/home/user", "/home/username"));
    #[cfg(target_os = "windows")]
    {
        assert!(paths_match("D:/Music/", "d:\\music"));
        assert!(paths_match("D:/", "d:\\"));
        assert!(paths_match(r"\\?\C:\Long\Path", r"c:\long\path\"));
        assert!(paths_match(r"\\?\UNC\server\share\folder", r"\\SERVER\share\folder"));
    }
    #[cfg(not(target_os = "windows"))]
    assert!(!paths_match("/home/User", "/home/user"));
}

#[test]
fn path_within_counts_whole_components() {
    assert!(path_within(Path::new("/media/share/folder"), Path::new("/media/share")));
    assert!(path_within(Path::new("/media/share"), Path::new("/media/share/")));
    assert!(!path_within(Path::new("/media/share-other"), Path::new("/media/share")));
    #[cfg(target_os = "windows")]
    assert!(path_within(Path::new("z:/Docs/report.pdf"), Path::new(r"Z:\")));
    #[cfg(not(target_os = "windows"))]
    assert!(!path_within(Path::new("/media/Share/folder"), Path::new("/media/share")));
}
