use super::*;

fn paths(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

const VOLUME: &str = r"\\?\Volume{00000000-0000-0000-0000-000000000001}\";

#[test]
fn paths_below_a_registered_mount_folder_move_to_the_volume_root() {
    let rebased = rebased(
        Path::new(r"C:\Data\Photos\a.jpg"),
        Path::new(r"C:\Data\"),
        Path::new(VOLUME),
        &paths(&[r"E:\", r"c:\data\"]),
    );
    assert_eq!(rebased, Some(Path::new(VOLUME).join(r"Photos\a.jpg")));
    let folder = rebased_folder();
    assert_eq!(folder, Some(PathBuf::from(VOLUME)));
}

fn rebased_folder() -> Option<PathBuf> {
    rebased(Path::new(r"C:\Data"), Path::new(r"C:\Data\"), Path::new(VOLUME), &paths(&[r"C:\Data\"]))
}

#[test]
fn unregistered_folders_and_drive_roots_are_not_rebased() {
    // A junction aimed at a volume root is not in the mount manager's list.
    let junction = rebased(
        Path::new(r"C:\Shared\link\Windows"),
        Path::new(r"C:\Shared\link\"),
        Path::new(VOLUME),
        &paths(&[r"C:\"]),
    );
    assert_eq!(junction, None);
    let root = rebased(Path::new(r"C:\Windows"), Path::new(r"C:\"), Path::new(VOLUME), &paths(&[r"C:\"]));
    assert_eq!(root, None);
    let elsewhere = rebased(
        Path::new(r"C:\Other\x"),
        Path::new(r"C:\Data\"),
        Path::new(VOLUME),
        &paths(&[r"C:\Data\"]),
    );
    assert_eq!(elsewhere, None);
}

#[test]
fn only_drive_paths_are_considered() {
    assert_eq!(drive_path(Path::new(r"\\?\C:\Data\x")), Some(PathBuf::from(r"C:\Data\x")));
    assert_eq!(drive_path(Path::new(r"C:\Data\x")), Some(PathBuf::from(r"C:\Data\x")));
    assert_eq!(drive_path(Path::new(r"\\server\share\x")), None);
    assert_eq!(drive_path(Path::new(VOLUME)), None);
}

// Anyone may aim a junction at a volume root; only mount folders the mount
// manager registered are crossed.
#[test]
fn junctions_to_a_volume_root_are_still_refused() {
    let drive = std::env::var_os("SystemDrive").unwrap_or_else(|| "C:".into());
    let mut root = std::ffi::OsString::from(&drive);
    root.push("\\");
    let Some(volume) = volume_name(Path::new(&root)) else {
        return;
    };
    let folder = tempfile::tempdir().unwrap();
    let link = folder.path().join("link");
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&volume)
        .output()
        .unwrap();
    assert!(status.status.success());
    assert!(super::super::open(&link.join("Windows"), true, 0x80, 7, 1).is_err());
    assert!(super::super::open(&link, true, 0x80, 7, 1).is_err());
}
