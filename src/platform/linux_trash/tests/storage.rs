use super::*;
use std::ffi::OsStr;

fn item(trash: &Path, name: &str, original_parent: &Path) -> trash::TrashItem {
    trash::TrashItem {
        id: trash.join("info").join(format!("{name}.trashinfo")).into_os_string(),
        name: name.into(),
        original_parent: original_parent.to_path_buf(),
        time_deleted: 0,
    }
}

fn trash_with(name: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let trash = root.path().join("Trash");
    std::fs::create_dir_all(trash.join("files")).unwrap();
    std::fs::create_dir_all(trash.join("info")).unwrap();
    std::fs::write(trash.join("files").join(name), b"content").unwrap();
    std::fs::write(trash.join("info").join(format!("{name}.trashinfo")), b"[Trash Info]\n").unwrap();
    let original = root.path().join("original");
    std::fs::create_dir(&original).unwrap();
    (root, trash, original)
}

#[test]
fn content_is_beside_the_record_and_names_keep_their_case() {
    let trash = Path::new("/home/user/.local/share/Trash");
    let upper = item(trash, "A.txt", Path::new("/tmp"));
    let lower = item(trash, "a.txt", Path::new("/tmp"));
    assert_eq!(content_path(&upper).unwrap(), trash.join("files/A.txt"));
    assert_eq!(content_path(&lower).unwrap(), trash.join("files/a.txt"));
}

#[test]
fn trash_content_is_recognized_only_under_a_trash_root() {
    let (_root, trash, original) = trash_with("x");
    assert!(inside_trash(&trash.join("files").join("x")));
    assert!(inside_trash(&trash.join("files").join("x").join("child")));
    assert!(!inside_trash(&trash.join("info").join("x.trashinfo")));
    assert!(!inside_trash(&original.join("files").join("x")));
}

#[test]
fn restore_refuses_conflicts_and_missing_folders_without_touching_the_trash() {
    let (_root, trash, original) = trash_with("report.txt");
    std::fs::write(original.join("report.txt"), b"kept").unwrap();
    let error = restore(&item(&trash, "report.txt", &original)).unwrap_err();
    assert_eq!(TrashRestoreFailure::from_message(&error), Some(TrashRestoreIssue::Conflict));
    assert_eq!(std::fs::read(original.join("report.txt")).unwrap(), b"kept");
    assert!(trash.join("files/report.txt").exists());

    let missing = original.join("gone");
    let error = restore(&item(&trash, "report.txt", &missing)).unwrap_err();
    assert_eq!(TrashRestoreFailure::from_message(&error), Some(TrashRestoreIssue::Unavailable));
    assert!(!missing.exists());
    assert!(trash.join("info/report.txt.trashinfo").exists());
}

#[test]
fn restore_moves_content_back_and_drops_its_record() {
    let (_root, trash, original) = trash_with("report.txt");
    restore(&item(&trash, "report.txt", &original)).unwrap();
    assert_eq!(std::fs::read(original.join("report.txt")).unwrap(), b"content");
    assert!(!trash.join("files/report.txt").exists());
    assert!(!trash.join("info/report.txt.trashinfo").exists());
}

#[test]
fn purge_removes_links_not_their_targets_and_updates_the_size_cache() {
    let (root, trash, original) = trash_with("plain");
    let target = root.path().join("target");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("keep"), b"keep").unwrap();
    std::os::unix::fs::symlink(&target, trash.join("files/link")).unwrap();
    std::fs::write(trash.join("info/link.trashinfo"), b"[Trash Info]\n").unwrap();
    std::fs::create_dir(trash.join("files/folder")).unwrap();
    std::fs::write(trash.join("info/folder.trashinfo"), b"[Trash Info]\n").unwrap();
    std::fs::write(trash.join("directorysizes"), b"10 1 folder\n20 2 other%20name\n").unwrap();

    purge(&[item(&trash, "link", &original), item(&trash, "folder", &original)]).unwrap();
    assert!(target.join("keep").exists());
    assert!(!trash.join("files/link").exists() && !trash.join("files/folder").exists());
    assert_eq!(std::fs::read(trash.join("directorysizes")).unwrap(), b"20 2 other%20name\n");
}

#[test]
fn size_cache_names_are_percent_decoded() {
    let cache = b"5 1 a%20b\n6 2 c\n";
    assert_eq!(without_cached_size(cache, OsStr::new("a b")).unwrap(), b"6 2 c\n");
    assert!(without_cached_size(cache, OsStr::new("a%20b")).is_none());
}

#[test]
fn kde_limits_default_when_absent_and_read_their_group() {
    let text = "[Status]\nEmpty=true\n\n[/home/user/.local/share/Trash]\nDays=30\nLimitReachedAction=1\nPercent=5.5\nUseSizeLimit=false\nUseTimeLimit=true\n";
    let home = kde_limits(text, PathBuf::from("/home/user/.local/share/Trash"));
    assert_eq!(home, KdeTrashLimits {
        trash: PathBuf::from("/home/user/.local/share/Trash"), use_time_limit: true, days: 30,
        use_size_limit: false, percent: 5.5, action: KdeLimitAction::DeleteOldest,
    });
    let other = kde_limits(text, PathBuf::from("/mnt/disk/.Trash-1000"));
    assert_eq!((other.use_time_limit, other.days, other.use_size_limit, other.percent, other.action),
        (false, 7, true, 10.0, KdeLimitAction::Warn));
}

#[test]
fn kde_settings_update_one_group_and_keep_the_rest() {
    let text = "# comment\n[Status]\nEmpty=true\n\n[/t]\nDays=7\nOther=x\n\n[Next]\nKey=1\n";
    let updated = set_kconfig_values(text, "/t", &[("Days", "30".into()), ("UseTimeLimit", "true".into())]);
    assert_eq!(updated, "# comment\n[Status]\nEmpty=true\n\n[/t]\nDays=30\nOther=x\nUseTimeLimit=true\n\n[Next]\nKey=1\n");
    let added = set_kconfig_values("[Status]\nEmpty=true\n", "/new", &[("Days", "3".into())]);
    assert_eq!(added, "[Status]\nEmpty=true\n\n[/new]\nDays=3\n");
    assert_eq!(set_kconfig_values("", "/t", &[("Days", "1".into())]), "[/t]\nDays=1\n");
}
