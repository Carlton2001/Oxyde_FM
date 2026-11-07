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
