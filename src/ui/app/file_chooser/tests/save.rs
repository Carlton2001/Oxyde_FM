use super::{stem_end, target, Target};

#[test]
fn the_stem_ends_before_the_last_extension() {
    assert_eq!(stem_end("report.odt"), 6);
    assert_eq!(stem_end("archive.tar.gz"), 11);
    assert_eq!(stem_end("Untitled"), 8);
    assert_eq!(stem_end(".bashrc"), 7);
    assert_eq!(stem_end(""), 0);
}

#[cfg(unix)]
#[test]
fn targets_tell_folders_files_free_names_and_missing_folders_apart() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("a.txt");
    std::fs::write(&file, b"a").unwrap();
    let link = folder.path().join("dangling");
    std::os::unix::fs::symlink(folder.path().join("nowhere"), &link).unwrap();
    assert_eq!(target(folder.path()), Target::Folder);
    assert_eq!(target(&file), Target::File);
    assert_eq!(target(&link), Target::File);
    assert_eq!(target(&folder.path().join("b.txt")), Target::Free);
    assert_eq!(target(&folder.path().join("missing/b.txt")), Target::Unreachable);
    assert_eq!(target(&file.join("b.txt")), Target::Unreachable);
}
