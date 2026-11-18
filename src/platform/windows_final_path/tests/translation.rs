use super::*;

#[test]
fn native_names_map_to_the_named_drive_or_the_network() {
    let ram = Some(('R', r"\Device\ImDisk0"));
    assert_eq!(dos_form(r"\Device\ImDisk0\work\a.txt", ram).as_deref(), Some(r"\\?\R:\work\a.txt"));
    assert_eq!(dos_form(r"\Device\ImDisk0", ram).as_deref(), Some(r"\\?\R:\"));
    assert_eq!(dos_form(r"\Device\Mup\vboxsvr\share\x", None).as_deref(), Some(r"\\?\UNC\vboxsvr\share\x"));
    // Another device, or a longer device name, is never taken for the drive.
    assert_eq!(dos_form(r"\Device\ImDisk01\work", ram), None);
    assert_eq!(dos_form(r"\Device\HarddiskVolume3\work", ram), None);
}

// The fallback itself, exercised on an ordinary volume: it must name the same
// object as the mount manager's form.
#[test]
fn the_translated_path_opens_the_same_object() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("file.txt");
    std::fs::write(&file, b"x").unwrap();
    let opened = std::fs::File::open(&file).unwrap();
    let expected = std::fs::canonicalize(&file).unwrap();
    assert_eq!(translated(&opened, &file).unwrap(), expected);
    assert_eq!(canonicalize(&file).unwrap(), expected);
    assert_eq!(canonicalize(root.path()).unwrap(), std::fs::canonicalize(root.path()).unwrap());
}
