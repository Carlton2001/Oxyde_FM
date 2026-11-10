use super::typed_path;
use std::path::Path;

#[test]
fn typed_paths_resolve_home_relative_names_and_dots() {
    let (folder, home) = (Path::new("/data/projects"), Some(Path::new("/home/a")));
    assert_eq!(typed_path("~", folder, home), Path::new("/home/a"));
    assert_eq!(typed_path(" ~/Music/ ", folder, home), Path::new("/home/a/Music"));
    assert_eq!(typed_path("~other", folder, home), Path::new("/data/projects/~other"));
    assert_eq!(typed_path("notes.txt", folder, home), Path::new("/data/projects/notes.txt"));
    assert_eq!(typed_path("../old/./a", folder, home), Path::new("/data/old/a"));
    assert_eq!(typed_path("/etc/../../usr", folder, home), Path::new("/usr"));
    assert_eq!(typed_path("~", folder, None), Path::new("/data/projects/~"));
}

#[cfg(target_os = "windows")]
#[test]
fn typed_windows_paths_keep_their_drive_or_share() {
    let (folder, home) = (Path::new(r"D:\Data\Projects"), Some(Path::new(r"C:\Users\Me")));
    assert_eq!(typed_path("notes.txt", folder, home), Path::new(r"D:\Data\Projects\notes.txt"));
    assert_eq!(typed_path(r"..\..\..\Old", folder, home), Path::new(r"D:\Old"));
    assert_eq!(typed_path(r"E:\Media\.\a.png", folder, home), Path::new(r"E:\Media\a.png"));
    assert_eq!(typed_path(r"~\Music", folder, home), Path::new(r"C:\Users\Me\Music"));
    assert_eq!(typed_path(r"\\server\share\a\..\b", folder, home), Path::new(r"\\server\share\b"));
}
