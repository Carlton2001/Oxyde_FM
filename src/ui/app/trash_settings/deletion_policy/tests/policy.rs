use super::DeletePolicy;
use oxyde_lib::commands::ops::TrashSystemConfig;

#[test]
fn policy_distinguishes_drives_and_accepts_windows_path_forms() {
    let mut policy = DeletePolicy::default();
    assert!(policy.replace(0, vec![("C:\\".into(), true), ("D:\\".into(), false)]));
    for path in [r"C:\folder\file.txt", "c:/folder/file.txt", r"\\?\C:\folder\file.txt"] {
        assert!(policy.for_path(path), "{path}");
    }
    assert!(!policy.for_path(r"D:\folder\file.txt"));
    assert!(!policy.for_path(r"\\server\share\file.txt"));
}

#[test]
fn saved_settings_win_over_a_stale_background_snapshot() {
    let mut policy = DeletePolicy::default();
    let revision = policy.revision();
    policy.saved(&[TrashSystemConfig {
        drive_path: "C:\\".into(), max_size_mb: 100, nuke_on_delete: true,
    }]);
    assert!(!policy.replace(revision, vec![("C:\\".into(), false)]));
    assert!(policy.for_path("C:/file.txt"));
    assert!(policy.replace(policy.revision(), vec![("C:\\".into(), false)]));
    assert!(!policy.for_path("C:/file.txt"));
}

#[test]
fn longest_mount_root_controls_the_label() {
    let mut policy = DeletePolicy::default();
    policy.replace(0, vec![("C:\\".into(), true), ("C:\\mounted\\".into(), false)]);
    assert!(!policy.for_path("C:/mounted/file.txt"));
    assert!(policy.for_path("C:/mounted-other/file.txt"));
}
