use super::*;
use std::os::unix::fs::{symlink, PermissionsExt};

const APP_ID: &str = "io.github.Carlton2001.Oxyde";
const INSTALL_SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/packaging/linux/install.sh");

/// A home holding install.sh's layout, with `version` behind `current`.
fn user_home(version: &str) -> (tempfile::TempDir, PathBuf) {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join(".local/lib/oxyde");
    std::fs::create_dir_all(root.join(version).join("bin")).unwrap();
    std::fs::write(root.join(version).join("bin/oxyde"), b"").unwrap();
    symlink(version, root.join("current")).unwrap();
    let executable = std::fs::canonicalize(root.join(version).join("bin/oxyde")).unwrap();
    (home, executable)
}

#[test]
fn per_user_installation_updates_itself() {
    let (home, executable) = user_home("2.0.3");
    assert_eq!(detect(&executable, Some(home.path()), false), Installation::SelfUpdating { manifest_key: ARCHIVE_KEY });
    assert_eq!(user_root(&executable, Some(home.path())), Some(executable.parent().unwrap().parent().unwrap().parent().unwrap().to_path_buf()));
}

#[test]
fn home_reached_through_a_link_still_matches() {
    let (home, executable) = user_home("2.0.3");
    let links = tempfile::tempdir().unwrap();
    symlink(home.path(), links.path().join("home")).unwrap();
    assert!(user_root(&executable, Some(&links.path().join("home"))).is_some());
}

#[test]
fn copies_outside_install_layout_are_unmanaged() {
    let (home, executable) = user_home("2.0.3");
    std::fs::remove_file(home.path().join(".local/lib/oxyde/current")).unwrap();
    assert_eq!(detect(&executable, Some(home.path()), false), Installation::Unmanaged);
    let other = tempfile::tempdir().unwrap();
    assert_eq!(detect(&executable, Some(other.path()), false), Installation::Unmanaged);
    assert_eq!(detect(Path::new("/src/target/release/oxyde"), Some(home.path()), false), Installation::Unmanaged);
    assert_eq!(detect(&executable, None, false), Installation::Unmanaged);
}

#[test]
fn system_packages_announce_the_deb_only_when_dpkg_owns_oxyde() {
    let executable = Path::new(SYSTEM_EXECUTABLE);
    assert_eq!(detect(executable, None, true), Installation::Package { manifest_key: Some(DEB_KEY) });
    assert_eq!(detect(executable, None, false), Installation::Package { manifest_key: None });
}

#[test]
fn only_the_archive_top_script_is_taken() {
    assert!(is_script(Path::new("oxyde-2.0.3-x86_64-linux/install.sh")));
    assert!(!is_script(Path::new("oxyde-2.0.3-x86_64-linux/bin/install.sh")));
    assert!(!is_script(Path::new("install.sh")));
    assert!(!is_script(Path::new("other-2.0.3/install.sh")));
    assert!(!is_script(Path::new("../oxyde-2.0.3-x86_64-linux/install.sh")));
}

enum Item<'a> {
    File(&'a str, &'a [u8], u32),
    Link(&'a str, &'a str),
}

fn archive(folder: &Path, items: &[Item]) -> PathBuf {
    let path = folder.join("oxyde-9.9.9-x86_64-linux.tar.gz");
    let encoder = flate2::write::GzEncoder::new(File::create(&path).unwrap(), flate2::Compression::fast());
    let mut builder = tar::Builder::new(encoder);
    for item in items {
        let mut header = tar::Header::new_gnu();
        match item {
            Item::File(name, contents, mode) => {
                header.set_entry_type(tar::EntryType::Regular);
                header.set_size(contents.len() as u64);
                header.set_mode(*mode);
                builder.append_data(&mut header, name, *contents).unwrap();
            }
            Item::Link(name, target) => {
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_size(0);
                header.set_mode(0o777);
                builder.append_link(&mut header, name, target).unwrap();
            }
        }
    }
    builder.into_inner().unwrap().finish().unwrap();
    path
}

#[test]
fn script_is_copied_out_of_the_archive() {
    let folder = tempfile::tempdir().unwrap();
    let path = archive(folder.path(), &[
        Item::File("oxyde-9.9.9-x86_64-linux/bin/oxyde", b"binary", 0o755),
        Item::File("oxyde-9.9.9-x86_64-linux/install.sh", b"#!/bin/bash\n", 0o755),
    ]);
    let script = folder.path().join("install.sh");
    extract_script(&path, &script).unwrap();
    assert_eq!(std::fs::read(&script).unwrap(), b"#!/bin/bash\n");
    assert_eq!(std::fs::metadata(&script).unwrap().permissions().mode() & 0o777, 0o700);
}

#[test]
fn linked_or_missing_script_is_refused() {
    let folder = tempfile::tempdir().unwrap();
    let linked = archive(folder.path(), &[Item::Link("oxyde-9.9.9-x86_64-linux/install.sh", "/etc/passwd")]);
    assert_eq!(extract_script(&linked, &folder.path().join("a.sh")).unwrap_err().kind(), io::ErrorKind::InvalidData);
    let missing = archive(folder.path(), &[Item::File("oxyde-9.9.9-x86_64-linux/bin/oxyde", b"binary", 0o755)]);
    assert_eq!(extract_script(&missing, &folder.path().join("b.sh")).unwrap_err().kind(), io::ErrorKind::InvalidData);
}

#[test]
fn existing_script_is_never_overwritten() {
    let folder = tempfile::tempdir().unwrap();
    let path = archive(folder.path(), &[Item::File("oxyde-9.9.9-x86_64-linux/install.sh", b"new", 0o755)]);
    let script = folder.path().join("install.sh");
    std::fs::write(&script, b"planted").unwrap();
    assert_eq!(extract_script(&path, &script).unwrap_err().kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read(&script).unwrap(), b"planted");
}

/// A release archive whose install.sh is this checkout's, or `script` when given.
fn release(folder: &Path, version: &str, script: Option<&[u8]>) -> PathBuf {
    let packaging = Path::new(env!("CARGO_MANIFEST_DIR")).join("packaging/linux");
    let desktop = std::fs::read(packaging.join(format!("{APP_ID}.desktop"))).unwrap();
    let metainfo = std::fs::read(packaging.join(format!("{APP_ID}.metainfo.xml"))).unwrap();
    let ours = std::fs::read(INSTALL_SCRIPT).unwrap();
    let top = format!("oxyde-{version}-x86_64-linux");
    let files: Vec<(String, &[u8], u32)> = vec![
        (format!("{top}/bin/oxyde"), b"#!/bin/sh\n", 0o755),
        (format!("{top}/bin/oxyde-archive-io-worker"), b"#!/bin/sh\n", 0o755),
        (format!("{top}/share/applications/{APP_ID}.desktop"), &desktop, 0o644),
        (format!("{top}/share/metainfo/{APP_ID}.metainfo.xml"), &metainfo, 0o644),
        (format!("{top}/share/icons/hicolor/scalable/apps/{APP_ID}.svg"), b"<svg/>", 0o644),
        (format!("{top}/install.sh"), script.unwrap_or(&ours), 0o755),
    ];
    let items: Vec<Item> = files.iter().map(|(name, contents, mode)| Item::File(name, contents, *mode)).collect();
    let path = folder.join(format!("{top}.tar.gz"));
    std::fs::rename(archive(folder, &items), &path).unwrap();
    path
}

/// What Oxyde does with a downloaded release, on this checkout's install.sh: the
/// options it passes are the contract between releases.
#[test]
fn update_switches_versions_and_keeps_the_running_one() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join(".local/lib/oxyde");
    let data = home.path().join("data");
    for version in ["9.9.7", "9.9.8", "9.9.9"] {
        let folder = tempfile::tempdir().unwrap();
        install_into(&release(folder.path(), version, None), &root, Some(data.clone())).unwrap();
    }
    assert_eq!(std::fs::read_link(root.join("current")).unwrap(), Path::new("9.9.9"));
    assert!(!root.join("9.9.7").exists());
    assert!(root.join("9.9.8/bin/oxyde").exists(), "the replaced version may still be running");
    assert!(root.join("9.9.9/bin/oxyde-archive-io-worker").exists());
    assert_eq!(std::fs::read_link(home.path().join(".local/bin/oxyde")).unwrap(), root.join("current/bin/oxyde"));
    let entry = std::fs::read_to_string(data.join(format!("applications/{APP_ID}.desktop"))).unwrap();
    assert!(entry.contains(&format!("Exec=\"{}/current/bin/oxyde\" %U", root.display())), "{entry}");
    assert!(entry.lines().any(|line| line == "X-Oxyde-Install=user"));
    assert!(data.join(format!("icons/hicolor/scalable/apps/{APP_ID}.svg")).exists());
    let executable = std::fs::canonicalize(root.join("current/bin/oxyde")).unwrap();
    assert_eq!(detect(&executable, Some(home.path()), false), Installation::SelfUpdating { manifest_key: ARCHIVE_KEY });
}

#[test]
fn failed_installation_reports_the_script_message() {
    let home = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let script: &[u8] = b"#!/bin/bash\necho 'Error: no space left' >&2\nexit 1\n";
    let error = install_into(&release(folder.path(), "9.9.9", Some(script)), &home.path().join(".local/lib/oxyde"), None).unwrap_err();
    assert_eq!(error.to_string(), "no space left");
}

#[test]
fn only_the_download_folder_is_removed() {
    let parent = tempfile::tempdir().unwrap();
    let download = parent.path().join("oxyde-update-abc");
    let other = parent.path().join("Downloads");
    for folder in [&download, &other] {
        std::fs::create_dir(folder).unwrap();
        std::fs::write(folder.join("oxyde-9.9.9-x86_64-linux.tar.gz"), b"").unwrap();
        remove_download_folder(&folder.join("oxyde-9.9.9-x86_64-linux.tar.gz"));
    }
    assert!(!download.exists());
    assert!(other.join("oxyde-9.9.9-x86_64-linux.tar.gz").exists());
}

#[test]
fn new_version_starts_only_after_the_old_process_ends() {
    let folder = tempfile::tempdir().unwrap();
    let started = folder.path().join("started");
    let executable = folder.path().join("oxyde");
    std::fs::write(&executable, format!("#!/bin/sh\ntouch '{}'\n", started.display())).unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (reader, writer) = std::io::pipe().unwrap();
    // Stands for the old Oxyde: it ends when the pipe's writer closes.
    let mut old = Command::new("sh").arg("-c").arg("read line").stdin(reader).spawn().unwrap();
    relaunch_after(old.id(), &executable).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(600));
    assert!(!started.exists(), "started while the old process was alive");
    drop(writer);
    old.wait().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !started.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(started.exists(), "the new version never started");
}
