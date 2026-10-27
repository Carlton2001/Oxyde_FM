use super::*;
use std::fs;

fn held(path: &Path) -> File {
    File::open(path).unwrap()
}

fn folder() -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    fs::create_dir(&folder).unwrap();
    (root, folder)
}

#[test]
fn effects_land_in_the_held_folder_after_it_was_renamed_and_replaced() {
    let (root, folder) = folder();
    let parent = held(&folder);
    let moved = root.path().join("moved");
    fs::rename(&folder, &moved).unwrap();
    fs::create_dir(&folder).unwrap();
    create_file(&parent, &folder.join("output")).unwrap();
    ensure_directory(&parent, &folder.join("sub")).unwrap();
    assert!(moved.join("output").is_file() && moved.join("sub").is_dir());
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 0);
}

#[test]
fn a_final_link_is_never_followed() {
    let (root, folder) = folder();
    let outside = root.path().join("outside");
    fs::write(&outside, b"kept").unwrap();
    std::os::unix::fs::symlink(&outside, folder.join("link")).unwrap();
    let parent = held(&folder);
    assert!(open_file(&parent, &folder.join("link"), true).is_err());
    assert!(open_file(&parent, &folder.join("link"), false).is_err());
    assert!(create_file(&parent, &folder.join("link")).is_err());
    assert!(open_directory(&parent, OsStr::new("link")).is_err());
    assert_eq!(fs::read(&outside).unwrap(), b"kept");
}

#[test]
fn removal_takes_the_held_file_and_keeps_its_replacement() {
    let (root, folder) = folder();
    let path = folder.join("checked");
    fs::write(&path, b"checked").unwrap();
    let file = held(&path);
    // Replaced after the check: the held object went elsewhere, a new one took its name.
    let elsewhere = root.path().join("elsewhere");
    fs::rename(&path, &elsewhere).unwrap();
    fs::write(&path, b"replacement").unwrap();
    remove(&file, &path).unwrap();
    assert!(!elsewhere.exists());
    assert_eq!(fs::read(&path).unwrap(), b"replacement");
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 1);
}

#[test]
fn a_wrong_entry_is_put_back_untouched() {
    let (_root, folder) = folder();
    let (one, other) = (folder.join("one"), folder.join("other"));
    fs::write(&one, b"one").unwrap();
    fs::write(&other, b"other").unwrap();
    let parent = held(&folder);
    let expected = identity(&held(&other)).unwrap();
    assert!(remove_entry(&parent, OsStr::new("one"), expected, false).is_err());
    assert_eq!(fs::read(&one).unwrap(), b"one");
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 2);
}

#[test]
fn renaming_moves_the_held_file_from_its_current_folder() {
    let (root, folder) = folder();
    let source = folder.join("source");
    fs::write(&source, b"held").unwrap();
    let file = held(&source);
    let target = root.path().join("target");
    fs::create_dir(&target).unwrap();
    let relocated = root.path().join("relocated");
    fs::rename(&folder, &relocated).unwrap();
    fs::create_dir(&folder).unwrap();
    fs::write(&source, b"replacement").unwrap();
    rename_file(&file, &source, &held(&target), &target.join("arrived"), false).unwrap();
    assert_eq!(fs::read(target.join("arrived")).unwrap(), b"held");
    assert_eq!(fs::read(&source).unwrap(), b"replacement");
    assert!(!relocated.join("source").exists());
}

#[test]
fn exclusive_renaming_never_replaces_and_replacing_does() {
    let (_root, folder) = folder();
    let (source, taken) = (folder.join("source"), folder.join("taken"));
    fs::write(&source, b"source").unwrap();
    fs::write(&taken, b"taken").unwrap();
    let (file, parent) = (held(&source), held(&folder));
    let error = rename_file(&file, &source, &parent, &taken, false).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read(&taken).unwrap(), b"taken");
    rename_file(&file, &source, &parent, &taken, true).unwrap();
    assert_eq!(fs::read(&taken).unwrap(), b"source");
    assert!(!source.exists());
}

#[test]
fn an_item_is_renamed_only_while_its_name_is_still_it() {
    let (_root, folder) = folder();
    let source = folder.join("item");
    fs::create_dir(&source).unwrap();
    let parent = held(&folder);
    let item = open_item(&parent, &source).unwrap();
    fs::rename(&source, folder.join("away")).unwrap();
    fs::create_dir(&source).unwrap();
    assert!(rename_item(&item, &parent, &source, &parent, &folder.join("renamed")).is_err());
    assert!(source.is_dir() && folder.join("away").is_dir() && !folder.join("renamed").exists());
}

#[test]
fn an_empty_folder_goes_only_if_it_is_the_expected_one() {
    let (_root, folder) = folder();
    let sub = folder.join("sub");
    fs::create_dir(&sub).unwrap();
    let parent = held(&folder);
    let expected = open_directory(&parent, OsStr::new("sub")).unwrap();
    fs::rename(&sub, folder.join("first")).unwrap();
    fs::create_dir(&sub).unwrap();
    assert!(remove_directory(&parent, &sub, &expected).is_err());
    assert!(sub.is_dir());
    remove(&expected, &sub).unwrap();
    assert!(!folder.join("first").exists() && sub.is_dir());
}

/// Exclusive publication on each listed mount, for instance vfat, exFAT and ntfs-3g
/// images: `OXYDE_TEST_PUBLISH=<writable folders>`.
#[test]
fn publication_is_exclusive_on_every_listed_file_system() {
    let Some(folders) = std::env::var_os("OXYDE_TEST_PUBLISH") else { return };
    for base in std::env::split_paths(&folders) {
        let root = tempfile::tempdir_in(&base).unwrap();
        let (source, taken, free) = (root.path().join("source"), root.path().join("taken"), root.path().join("free"));
        fs::write(&source, b"source").unwrap();
        fs::write(&taken, b"taken").unwrap();
        let parent = held(root.path());
        // Held as operations hold it, without reading: FUSE then forgets it once unnamed.
        let file = open_file(&parent, &source, false).unwrap();
        let error = rename_file(&file, &source, &parent, &taken, false).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists, "{}", base.display());
        rename_file(&file, &source, &parent, &free, false).unwrap();
        assert_eq!(fs::read(&free).unwrap(), b"source", "{}", base.display());
        assert!(!source.exists(), "{}", base.display());
        remove(&file, &free).unwrap();
        assert!(!free.exists(), "{}", base.display());
        let (sub_parent, sub) = (held(root.path()), root.path().join("sub"));
        let (directory, _) = ensure_directory(&sub_parent, &sub).unwrap();
        remove_directory(&sub_parent, &sub, &directory).unwrap();
        assert!(!sub.exists(), "{}", base.display());
    }
}
