use super::Output;
use crate::platform::platform;
use crate::systems::path_parents::HeldParents;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;

/// Root ignores folder modes: the refusal below needs an ordinary user.
fn refused_for_this_user(folder: &std::path::Path) -> bool {
    let probe = folder.join("probe");
    std::fs::write(&probe, b"").unwrap();
    std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o555)).unwrap();
    let refused = std::fs::remove_file(&probe).is_err();
    std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _ = std::fs::remove_file(&probe);
    refused
}

#[test]
fn a_partial_output_the_folder_refuses_to_remove_is_emptied() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("drop");
    std::fs::create_dir(&folder).unwrap();
    if !refused_for_this_user(&folder) {
        return;
    }
    let destination = folder.join("copy.bin");
    let parents = HeldParents::new(&destination).unwrap();
    let output = Output::new(&parents, &destination, false).unwrap();
    let mut file = platform().create_operation_output(parents.directory(), output.path()).unwrap();
    file.write_all(b"partial contents").unwrap();
    output.capture(&file).unwrap();
    drop(file);
    // Creating stays possible, removing does not: as a drop folder.
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o555)).unwrap();
    drop(output);
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(std::fs::read(&destination).unwrap(), b"");
}

#[test]
fn a_partial_output_is_removed() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("copy.bin");
    let parents = HeldParents::new(&destination).unwrap();
    let output = Output::new(&parents, &destination, false).unwrap();
    let mut file = platform().create_operation_output(parents.directory(), output.path()).unwrap();
    file.write_all(b"partial contents").unwrap();
    output.capture(&file).unwrap();
    drop(file);
    std::fs::write(root.path().join("witness"), b"unowned").unwrap();
    drop(output);
    assert!(!destination.exists());
    assert_eq!(std::fs::read(root.path().join("witness")).unwrap(), b"unowned");
}
