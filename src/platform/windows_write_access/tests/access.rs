use super::*;

fn icacls(path: &Path, arguments: &[&str]) {
    let status = std::process::Command::new("icacls").arg(path).args(arguments).output().unwrap().status;
    assert!(status.success(), "icacls {arguments:?}");
}

#[test]
fn only_subfolders_that_refuse_adding_files_and_folders_are_listed() {
    let root = tempfile::tempdir().unwrap();
    let (open, folders_only, locked) =
        (root.path().join("open"), root.path().join("folders-only"), root.path().join("locked"));
    for folder in [&open, &folders_only, &locked] {
        std::fs::create_dir(folder).unwrap();
    }
    // A deny entry for Everyone binds administrators too, elevated or not.
    icacls(&folders_only, &["/deny", "*S-1-1-0:(WD)"]);
    icacls(&locked, &["/deny", "*S-1-1-0:(WD,AD)"]);
    let paths = [&open, &folders_only, &locked, &root.path().join("missing")]
        .map(|path| path.to_string_lossy().into_owned());
    let denied = denied_subfolders(root.path(), paths.to_vec());
    for folder in [&folders_only, &locked] {
        icacls(folder, &["/remove:d", "*S-1-1-0"]);
    }
    assert_eq!(denied, [paths[2].clone()]);
    assert!(!denies_writes(root.path()));
    assert!(!denies_writes(Path::new("::{20D04FE0-3AEA-1069-A2D8-08002B30309D}")));
}
