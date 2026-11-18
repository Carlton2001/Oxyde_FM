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
    let mut volume = folder.components().next().unwrap().as_os_str().to_owned();
    volume.push("\\");
    let volume = std::path::PathBuf::from(volume);
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
    assert_ne!(kernel(), Kernel::Unknown);
}

// Kernels without OBJ_DONT_REPARSE look up single components with
// OPEN_REPARSE_POINT only: the final check must still refuse a junction.
#[test]
fn single_components_refuse_junctions_without_dont_reparse() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    std::fs::create_dir(&target).unwrap();
    let link = root.path().join("link");
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&target)
        .output()
        .unwrap();
    assert!(status.status.success());
    let parent = open(root.path(), true, 0x20 | 0x80, 7, 1).unwrap();
    let leaf = leaf_name(std::ffi::OsStr::new("link")).unwrap();
    let (opened, _) = create_file(leaf.clone(), parent.as_raw_handle(), Some(true), 0x80, 7, 1, false, 0x80).unwrap();
    assert_ne!(opened.metadata().unwrap().file_attributes() & 0x400, 0);
    assert!(open_name_with_attributes(leaf, parent.as_raw_handle(), Some(true), 0x80, 7, 1, true, 0x80).is_err());
}

/// An existing local path reached through the loopback administrative
/// share, or None where that share is not reachable. The SMB client caches
/// missing names, so the path must predate the test.
fn loopback_share(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let text = path.to_str()?;
    let (drive, rest) = text.split_once(":\\")?;
    let shared = std::path::PathBuf::from(format!(r"\\localhost\{drive}$\{rest}"));
    std::fs::metadata(&shared).ok().map(|_| shared)
}

// Windows 11 accepts `\??\C:` but refuses the `\??\UNC` link under
// OBJ_DONT_REPARSE (error 4395): network paths must fall back to the walk.
#[test]
fn network_paths_open_whatever_the_kernel_verdict() {
    let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(shared) = loopback_share(crate_root) else {
        return;
    };
    open(&shared, true, 0x80, 7, 1).unwrap();
    open(&shared.join("Cargo.toml"), false, 0x80, 7, 1).unwrap();
    let mut verbatim = std::ffi::OsString::from(r"\\?\UNC\");
    verbatim.push(&shared.to_str().unwrap()[2..]);
    let verbatim = std::path::PathBuf::from(verbatim);
    open(&verbatim.join("Cargo.toml"), false, 0x80, 7, 1).unwrap();
}

// `Default User` is a junction every Windows profile folder carries.
#[test]
fn junctions_on_network_paths_are_still_refused() {
    let Some(profiles) = std::env::var_os("PUBLIC")
        .and_then(|public| std::path::Path::new(&public).parent().map(|path| path.to_path_buf()))
    else {
        return;
    };
    let junction = profiles.join("Default User");
    if !std::fs::symlink_metadata(&junction).is_ok_and(|metadata| metadata.file_attributes() & 0x400 != 0) {
        return;
    }
    let Some(shared) = loopback_share(&junction) else {
        return;
    };
    assert!(open(&shared, true, 0x80, 7, 1).is_err());
    assert!(open(&shared.join("NTUSER.DAT"), false, 0x80, 7, 1).is_err());
}
