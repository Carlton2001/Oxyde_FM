//! Moving changes a folder entry, traversing needs `x`: as `mv` and `cp`, none of
//! these needs to read the item or its ancestors.
use super::*;
use std::os::unix::fs::PermissionsExt;

fn set_mode(path: &Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

fn transfer(source: &Path, target: &Path, is_move: bool) -> Result<(), String> {
    let kind = if is_move { FileOpType::Move } else { FileOpType::Copy };
    let op = Arc::new(Mutex::new(FileOperation::new(kind, vec![source.to_path_buf()], Some(target.to_path_buf()), None)));
    FileOperationManager::perform_copy(&op, vec![source.to_path_buf()], Some(target.to_path_buf()), is_move, None)
}

/// Gives every folder back its permissions so the temporary tree can be removed.
struct Restore(PathBuf);
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = std::process::Command::new("chmod").arg("-R").arg("u+rwx").arg(&self.0).status();
    }
}

fn tree() -> (tempfile::TempDir, PathBuf, PathBuf, Restore) {
    let root = tempfile::tempdir().unwrap();
    let (a, b) = (root.path().join("a"), root.path().join("b"));
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let restore = Restore(root.path().to_path_buf());
    (root, a, b, restore)
}

#[test]
fn an_unreadable_file_moves_and_renames_on_its_disk() {
    if rustix::process::geteuid().is_root() { return }
    let (_root, a, b, _restore) = tree();
    let secret = a.join("secret");
    std::fs::write(&secret, b"kept").unwrap();
    set_mode(&secret, 0o000);
    transfer(&secret, &b, true).unwrap();
    crate::systems::item_rename::rename(&b.join("secret"), &b.join("renamed")).unwrap();
    set_mode(&b.join("renamed"), 0o600);
    assert_eq!(std::fs::read(b.join("renamed")).unwrap(), b"kept");
}

#[test]
fn a_folder_that_cannot_be_listed_moves_but_does_not_copy() {
    if rustix::process::geteuid().is_root() { return }
    let (_root, a, b, _restore) = tree();
    let folder = a.join("drop-box");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("inside"), b"x").unwrap();
    set_mode(&folder, 0o300);
    // Copying must list it, as `cp -r` does.
    assert!(transfer(&folder, &b, false).is_err());
    let _ = std::fs::remove_dir(b.join("drop-box"));
    transfer(&folder, &b, true).unwrap();
    assert!(!folder.exists());
    set_mode(&b.join("drop-box"), 0o700);
    assert_eq!(std::fs::read(b.join("drop-box/inside")).unwrap(), b"x");
}

#[test]
fn traversable_ancestors_and_write_only_targets_are_enough() {
    if rustix::process::geteuid().is_root() { return }
    let (root, a, b, _restore) = tree();
    let shared = root.path().join("shared");
    std::fs::create_dir_all(shared.join("public")).unwrap();
    std::fs::write(shared.join("public/file"), b"through").unwrap();
    set_mode(&shared, 0o311);
    transfer(&shared.join("public/file"), &b, true).unwrap();
    assert_eq!(std::fs::read(b.join("file")).unwrap(), b"through");
    let drop_box = root.path().join("drop-box");
    std::fs::create_dir(&drop_box).unwrap();
    set_mode(&drop_box, 0o300);
    std::fs::write(a.join("copied"), b"c").unwrap();
    std::fs::write(a.join("moved"), b"m").unwrap();
    transfer(&a.join("copied"), &drop_box, false).unwrap();
    transfer(&a.join("moved"), &drop_box, true).unwrap();
    assert_eq!(std::fs::read(drop_box.join("copied")).unwrap(), b"c");
    assert_eq!(std::fs::read(drop_box.join("moved")).unwrap(), b"m");
}

/// Rights given by ACL entries rather than mode bits, for another user than the owner:
/// `OXYDE_TEST_ACL=<fixture>`, laid out by its owner as
/// - `shared` `0700` with `u:<tester>:--x`, holding `public` `0700` with `u:<tester>:rwx`
///   and `public/file` `0600`, unreadable by the tester;
/// - `drop` `0700` with `u:<tester>:-wx`;
/// - `mine`, owned by the tester, holding `copied`.
#[test]
fn rights_given_by_acl_entries_are_enough() {
    let Some(fixture) = std::env::var_os("OXYDE_TEST_ACL").map(PathBuf::from) else { return };
    let (file, drop_box) = (fixture.join("shared/public/file"), fixture.join("drop"));
    assert!(std::fs::read(&file).is_err() && std::fs::read_dir(&drop_box).is_err(), "fixture grants too much");
    transfer(&file, &drop_box, true).unwrap();
    transfer(&fixture.join("mine/copied"), &drop_box, false).unwrap();
    assert!(std::fs::symlink_metadata(drop_box.join("file")).unwrap().is_file());
    assert_eq!(std::fs::symlink_metadata(drop_box.join("copied")).unwrap().len(), 7);
    assert!(std::fs::symlink_metadata(&file).is_err());
}
