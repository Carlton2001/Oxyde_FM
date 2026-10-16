use std::path::Path;

/// Formats handled by Windows' native disk-image driver, as in Tauri.
pub fn is_disk_image(path: &Path) -> bool {
    disk_image_format(path) && path.is_file()
}

pub(crate) fn disk_image_format(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("iso" | "img" | "vhd" | "vhdx")
    )
}

pub fn needs_mount_for_browsing(path: &Path) -> bool {
    is_disk_image(path) && mount_format(path)
}

pub(crate) fn mount_format(path: &Path) -> bool {
    matches!(
            path.extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("vhd" | "vhdx")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_formats_exclude_archive_entries_and_unhandled_images() {
        let root = tempfile::tempdir().unwrap();
        for name in ["disc.ISO", "disc.img", "disk.VHD", "disk.vhdx"] {
            let path = root.path().join(name);
            std::fs::write(&path, b"fixture").unwrap();
            assert!(is_disk_image(&path));
        }
        for name in [
            "disc.cue",
            "disk.vmdk",
            "folder",
            "outer.zip/disc.iso",
            "outer.iso/disk.vhd",
        ] {
            assert!(!is_disk_image(&root.path().join(name)));
        }
        assert!(needs_mount_for_browsing(&root.path().join("disk.vhdx")));
        assert!(!needs_mount_for_browsing(&root.path().join("disc.iso")));
    }
}
