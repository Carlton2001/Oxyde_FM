use std::path::Path;

/// Formats handled by Windows' native disk-image driver, as in Tauri.
pub fn is_disk_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("iso" | "img" | "vhd" | "vhdx")
    ) && crate::utils::archive::split_virtual_path(&path.to_string_lossy())
        .is_none_or(|(_, internal)| internal.is_empty())
}

pub fn needs_mount_for_browsing(path: &Path) -> bool {
    is_disk_image(path)
        && matches!(
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
        for name in ["disc.ISO", "disc.img", "disk.VHD", "disk.vhdx"] {
            assert!(is_disk_image(Path::new(name)));
        }
        for name in [
            "disc.cue",
            "disk.vmdk",
            "folder",
            "outer.zip/disc.iso",
            "outer.iso/disk.vhd",
        ] {
            assert!(!is_disk_image(Path::new(name)));
        }
        assert!(needs_mount_for_browsing(Path::new("disk.vhdx")));
        assert!(!needs_mount_for_browsing(Path::new("disc.iso")));
    }
}
