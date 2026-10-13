use super::*;

#[test]
fn hardlink_alias_has_same_identity_and_cannot_bypass_retained_source_sharing() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.zip");
    let alias = root.path().join("alias.zip");
    std::fs::write(&source, b"held bytes").unwrap();
    std::fs::hard_link(&source, &alias).unwrap();
    let held = open(&source, false).unwrap();
    let second = open(&alias, false).unwrap();
    assert_eq!(identity(&held).unwrap(), identity(&second).unwrap());
    drop(second);
    assert!(OpenOptions::new().write(true).share_mode(7).open(&alias).is_err());
    assert!(std::fs::rename(&source, root.path().join("renamed-source.zip")).is_err());
    // NTFS can rename another hardlink. The held source still binds the same
    // object, and writes through the renamed alias must remain forbidden.
    let renamed_alias = root.path().join("renamed.zip");
    std::fs::rename(&alias, &renamed_alias).unwrap();
    assert_eq!(identity(&open(&renamed_alias, false).unwrap()).unwrap(), identity(&held).unwrap());
    assert!(OpenOptions::new().write(true).share_mode(7).open(&renamed_alias).is_err());
    assert_eq!(std::fs::read(&source).unwrap(), b"held bytes");
    drop(held);
    assert!(OpenOptions::new().write(true).share_mode(7).open(&renamed_alias).is_ok());
}

#[test]
fn source_identity_survives_metadata_only_changes_and_replacement_is_distinct() {
    use windows::Win32::Storage::FileSystem::FILE_WRITE_ATTRIBUTES;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.zip");
    std::fs::write(&source, b"original").unwrap();
    let held = open(&source, true).unwrap();
    let before = identity(&held).unwrap();
    let attributes = OpenOptions::new().access_mode(FILE_WRITE_ATTRIBUTES.0).share_mode(7).open(&source).unwrap();
    attributes.set_times(std::fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1234567890))).unwrap();
    assert_eq!(identity(&held).unwrap(), before);
    drop(attributes);
    std::fs::rename(&source, root.path().join("old.zip")).unwrap();
    std::fs::write(&source, b"original").unwrap();
    assert_ne!(identity(&open(&source, false).unwrap()).unwrap(), before);
    assert_eq!(identity(&held).unwrap(), before);
}
