use super::*;

const MARK: &[u8] = b"[ZoneTransfer]\r\nZoneId=3\r\n";

#[test]
fn local_volumes_keep_the_internet_mark() {
    let root = tempfile::tempdir().unwrap();
    assert!(has_named_streams(&hold_directory(root.path()).unwrap()).unwrap());
    let path = root.path().join("file");
    assert_eq!(read_provenance(&path).unwrap(), None);
    std::fs::write(&path, b"content").unwrap();
    assert_eq!(read_provenance(&path).unwrap(), None);
    write_provenance(&path, MARK).unwrap();
    assert_eq!(read_provenance(&path).unwrap().as_deref(), Some(MARK));
}

/// `OXYDE_TEST_NO_STREAMS` names a folder on a volume without alternate
/// streams: a FAT32 or exFAT drive, or `\wsl.localhost\<distribution>\tmp`.
#[test]
fn volumes_without_streams_carry_no_mark_and_refuse_nothing() {
    let Some(folder) = std::env::var_os("OXYDE_TEST_NO_STREAMS") else { return };
    let root = tempfile::tempdir_in(folder).unwrap();
    let held = hold_directory(root.path()).unwrap();
    assert!(!has_named_streams(&held).unwrap());
    let path = root.path().join("file");
    let output = create_file(&path).unwrap();
    assert_eq!(read_output_provenance(&held, &output, &path).unwrap(), None);
    write_output_provenance(&held, &output, &path, MARK).unwrap();
    drop(output);
    write_provenance(&path, MARK).unwrap();
    assert_eq!(read_provenance(&path).unwrap(), None);
    let names: Vec<_> = std::fs::read_dir(root.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, ["file"]);
}
