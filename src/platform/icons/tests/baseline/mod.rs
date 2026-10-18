//! Original implementation, frozen from fcf724874a05fc4c58ab4f6f3093b9bafe13a645.
use crate::platform::{platform, windows_icon_bitmap, windows_icon_pixels};
use image::RgbaImage;
use std::path::Path;

mod icons;
#[path = "cache.rs"]
mod windows_icon_cache;

pub(super) fn clear() {
    icons::purge_icon_cache();
}

pub(super) fn image(path: &str, size: &str, directory: Option<&Path>, key: &str) -> RgbaImage {
    let bytes = icons::extract_icon_png(
        path,
        size,
        false,
        directory.map(Path::to_path_buf),
        key.into(),
    )
    .unwrap();
    // The original worker decodes the cached/generated PNG before raw-pixel IPC.
    windows_icon_cache::decode_icon_png(&bytes).unwrap()
}

pub(super) fn disk(path: &Path) -> RgbaImage {
    // The original parent validates the PNG on read, then decodes it again.
    let bytes = windows_icon_cache::read_cached_png(path).unwrap();
    windows_icon_cache::decode_icon_png(&bytes).unwrap()
}
