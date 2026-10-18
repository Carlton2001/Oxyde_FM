//! Resolve broad Shell categories without opening a file or loading a property handler.
use crate::models::file_entry::NativeFileKind;
use windows::core::PCWSTR;
use windows::Win32::UI::Shell::{AssocGetPerceivedType, Common::*};

pub(super) fn for_extension(extension: &str) -> Option<NativeFileKind> {
    if !extension.starts_with('.') || extension.len() < 2 || extension.contains(['\0', '/', '\\']) {
        return None;
    }
    let wide: Vec<u16> = extension.encode_utf16().chain(Some(0)).collect();
    let mut kind = PERCEIVED_TYPE_UNSPECIFIED;
    let mut flags = 0;
    unsafe {
        AssocGetPerceivedType(PCWSTR(wide.as_ptr()), &mut kind, &mut flags, None).ok()?;
    }
    match kind {
        PERCEIVED_TYPE_TEXT | PERCEIVED_TYPE_DOCUMENT => Some(NativeFileKind::Documents),
        PERCEIVED_TYPE_IMAGE => Some(NativeFileKind::Images),
        PERCEIVED_TYPE_AUDIO => Some(NativeFileKind::Audio),
        PERCEIVED_TYPE_VIDEO => Some(NativeFileKind::Video),
        PERCEIVED_TYPE_COMPRESSED => Some(NativeFileKind::Archives),
        PERCEIVED_TYPE_APPLICATION => Some(NativeFileKind::Applications),
        _ => None,
    }
}

#[cfg(test)]
#[path = "windows_file_kind/tests/classification.rs"]
mod tests;
