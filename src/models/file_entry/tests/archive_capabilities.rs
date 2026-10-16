use super::*;

#[test]
fn entry_capabilities_use_known_origin_without_querying_the_filesystem() {
    for parent in ["ordinary.zip", "ordinary.7z", "ordinary.tar.gz"] {
        let mut file = FileEntry {
            path: format!("{parent}/inner.zip"),
            ..Default::default()
        };
        assert!(file.is_navigable_archive() && file.is_writable_archive());
        file.archive_path = Some(std::sync::Arc::new("real.zip".into()));
        assert!(!file.is_navigable_archive() && !file.is_writable_archive());
        file.path = format!("{parent}/disk.vhdx");
        assert!(!file.is_disk_image() && !file.needs_mount_for_browsing());
        file.archive_path = None;
        assert!(file.is_disk_image() && file.needs_mount_for_browsing());
        file.is_dir = true;
        assert!(!file.is_disk_image() && !file.needs_mount_for_browsing());
        file.path = format!("{parent}/folder.zip");
        assert!(!file.is_navigable_archive() && !file.is_writable_archive());
    }
}
