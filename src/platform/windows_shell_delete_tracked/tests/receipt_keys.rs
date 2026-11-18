use super::*;

// The Shell reports `\\srv\share\…` for a root canonicalized as
// `\\?\UNC\srv\share\…`: both must give the same key.
#[test]
fn network_roots_and_shell_paths_share_one_key() {
    assert_eq!(key(Path::new(r"\\?\UNC\srv\share\folder")), key(Path::new(r"\\srv\share\folder")));
    assert_eq!(key(Path::new(r"\\?\C:\folder")), key(Path::new(r"C:\folder")));
    assert_eq!(key(Path::new("C:/folder/file")), r"C:\folder\file");
}
