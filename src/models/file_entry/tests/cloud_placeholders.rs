use super::*;

#[test]
fn local_file_is_not_a_cloud_placeholder() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("local.txt");
    std::fs::write(&path, b"payload").unwrap();
    assert!(!get_file_entry_from_path(&path).unwrap().is_cloud_placeholder);
}

#[cfg(target_os = "windows")]
#[test]
fn offline_file_is_a_cloud_placeholder() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("offline.txt");
    std::fs::write(&path, b"payload").unwrap();
    let status = std::process::Command::new("attrib").arg("+O").arg(&path).status().unwrap();
    assert!(status.success());
    assert!(get_file_entry_from_path(&path).unwrap().is_cloud_placeholder);
}
