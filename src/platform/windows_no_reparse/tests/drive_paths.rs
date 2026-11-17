use super::*;

// Windows 10 refuses the `\??\C:` object manager link under OBJ_DONT_REPARSE
// (error 4395); absolute paths must still open below the volume root.
#[test]
fn absolute_drive_paths_open_below_the_volume_root() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("file.txt"), b"x").unwrap();
    open(&folder, true, 0x80, 7, 1).unwrap();
    open(&folder.join("file.txt"), false, 0x80, 7, 1).unwrap();
    let volume = std::path::Path::new(&folder.components().next().unwrap().as_os_str().to_owned()).join("\\");
    open(&volume, true, 0x80, 7, 1).unwrap();
}

#[test]
fn reparse_points_below_the_volume_root_are_still_refused() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("file.txt"), b"x").unwrap();
    let link = root.path().join("link");
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&target)
        .output()
        .unwrap();
    assert!(status.status.success());
    assert!(open(&link.join("file.txt"), false, 0x80, 7, 1).is_err());
    assert!(open(&link, true, 0x80, 7, 1).is_err());
}

// The walk Windows 10 falls back to, exercised on any kernel.
#[test]
fn the_component_walk_opens_plain_paths_and_refuses_junctions() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("file.txt"), b"x").unwrap();
    let absolute = std::path::absolute(&target).unwrap();
    open_by_components(&absolute, true, 0x80, 7, 1).unwrap();
    open_by_components(&absolute.join("file.txt"), false, 0x80, 7, 1).unwrap();
    let link = root.path().join("link");
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&target)
        .output()
        .unwrap();
    assert!(status.status.success());
    let link = std::path::absolute(&link).unwrap();
    assert!(open_by_components(&link.join("file.txt"), false, 0x80, 7, 1).is_err());
    assert!(open_by_components(&link, true, 0x80, 7, 1).is_err());
}

#[test]
fn the_kernel_probe_reaches_a_verdict_on_the_system_drive() {
    assert!(drive_links_refused().is_some());
}
