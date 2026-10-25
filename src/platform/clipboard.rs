#![cfg(target_os = "windows")]

use crate::models::CommandError;
use crate::utils::path_security::validate_path;
use log::info;
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::CF_HDROP;
use windows::Win32::UI::Shell::DROPFILES;

// Custom format for Preferred DropEffect
fn get_drop_effect_format() -> Result<u32, CommandError> {
    use windows::core::PCSTR;
    use windows::Win32::System::DataExchange::RegisterClipboardFormatA;
    let format =
        unsafe { RegisterClipboardFormatA(PCSTR(c"Preferred DropEffect".as_ptr().cast())) };
    if format == 0 {
        return Err(CommandError::SystemError(format!(
            "Cannot register clipboard drop effect: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(format)
}

fn malformed(format: &str, detail: &str) -> CommandError {
    CommandError::SystemError(format!("Invalid {format} clipboard data: {detail}"))
}

struct ClipboardReadSession;
impl Drop for ClipboardReadSession {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

/// A borrowed system allocation: unlock it on every exit, never free it.
struct LockedClipboardData {
    handle: HGLOBAL,
    pointer: *const u8,
    length: usize,
}
impl LockedClipboardData {
    /// # Safety
    /// The caller must keep the allocation alive and unchanged until Drop, either
    /// by holding the clipboard open or by owning the allocation.
    unsafe fn new(handle: HANDLE) -> Result<Self, CommandError> {
        let handle = HGLOBAL(handle.0);
        let length = unsafe { GlobalSize(handle) };
        if length == 0 || length > isize::MAX as usize {
            return Err(malformed("global memory", "invalid allocation size"));
        }
        let pointer = unsafe { GlobalLock(handle) }.cast::<u8>();
        if pointer.is_null() {
            return Err(CommandError::SystemError(format!(
                "Cannot lock clipboard data: {}",
                std::io::Error::last_os_error()
            )));
        }
        Ok(Self {
            handle,
            pointer,
            length,
        })
    }
    fn bytes(&self) -> &[u8] {
        // GlobalSize bounds the allocation; the caller keeps it alive and unchanged.
        unsafe { std::slice::from_raw_parts(self.pointer, self.length) }
    }
}
impl Drop for LockedClipboardData {
    fn drop(&mut self) {
        unsafe {
            let _ = GlobalUnlock(self.handle);
        }
    }
}

pub(super) fn parse_dropfiles(bytes: &[u8]) -> Result<Vec<String>, CommandError> {
    let header_size = std::mem::size_of::<DROPFILES>();
    if bytes.len() < header_size {
        return Err(malformed("CF_HDROP", "truncated header"));
    }
    // Decode bytes rather than dereferencing potentially unaligned external data.
    let offset = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    if offset < header_size || offset > bytes.len() {
        return Err(malformed(
            "CF_HDROP",
            "file list offset outside the payload",
        ));
    }
    let wide_offset = std::mem::offset_of!(DROPFILES, fWide);
    let wide = u32::from_le_bytes(bytes[wide_offset..wide_offset + 4].try_into().unwrap());
    if wide == 0 {
        // Preserve the existing behavior for unsupported ANSI file lists.
        return Ok(Vec::new());
    }
    let mut characters = bytes[offset..]
        .as_chunks::<2>().0.iter()
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]));
    let mut files = Vec::new();
    let mut name = Vec::new();
    while let Some(character) = characters.next() {
        if character != 0 {
            name.push(character);
        } else if name.is_empty() {
            if files.is_empty() && characters.next() != Some(0) {
                return Err(malformed("CF_HDROP", "missing empty-list terminator"));
            }
            // GlobalSize may include unused padding after the logical payload.
            return Ok(files);
        } else {
            let path = String::from_utf16(&name)
                .map_err(|_| malformed("CF_HDROP", "invalid UTF-16 path"))?;
            files.push(path);
            name.clear();
        }
    }
    Err(malformed("CF_HDROP", "missing double-null terminator"))
}

fn parse_drop_effect(bytes: &[u8]) -> Result<bool, CommandError> {
    let effect = bytes
        .get(..4)
        .ok_or_else(|| malformed("Preferred DropEffect", "truncated effect"))?;
    Ok(u32::from_le_bytes(effect.try_into().unwrap()) == 2)
}

pub(super) fn has_file_format() -> bool {
    unsafe { IsClipboardFormatAvailable(CF_HDROP.0 as u32) }.is_ok()
}

pub fn get_clipboard_files() -> Result<(Vec<String>, bool), CommandError> {
    if unsafe { OpenClipboard(None) }.is_err() {
        return Ok((Vec::new(), false));
    }
    let _session = ClipboardReadSession;
    let files = match unsafe { GetClipboardData(CF_HDROP.0 as u32) } {
        Ok(handle) if !handle.is_invalid() => {
            let data = unsafe { LockedClipboardData::new(handle) }?;
            parse_dropfiles(data.bytes())?
        }
        _ => return Ok((Vec::new(), false)),
    };
    let mut is_cut = false;
    if !files.is_empty() {
        let format = get_drop_effect_format()?;
        if unsafe { IsClipboardFormatAvailable(format) }.is_ok() {
            let handle = unsafe { GetClipboardData(format) }.map_err(|error| {
                CommandError::SystemError(format!("Cannot read clipboard drop effect: {error}"))
            })?;
            let data = unsafe { LockedClipboardData::new(handle) }?;
            // A present but truncated effect is an error, not an implicit Copy.
            is_cut = parse_drop_effect(data.bytes())?;
        }
    }
    Ok((files, is_cut))
}

pub fn set_clipboard_files(paths: Vec<String>, is_cut: bool) -> Result<(), CommandError> {
    info!("Setting clipboard (cut={}): {:?}", is_cut, paths);
    let validated_paths: Vec<String> = paths
        .iter()
        .map(|p| validate_path(p).map(|pb: std::path::PathBuf| pb.to_string_lossy().to_string()))
        .collect::<Result<Vec<String>, CommandError>>()?;

    let transfer_paths: Vec<_> = validated_paths
        .iter()
        .map(std::path::PathBuf::from)
        .collect();
    let trash_entries = super::platform()
        .trash_transfer_entries(&transfer_paths)
        .map_err(CommandError::TrashError)?;
    if !trash_entries.is_empty() {
        return super::windows_trash_clipboard::set_files(transfer_paths, is_cut)
            .map_err(CommandError::SystemError);
    }

    super::windows_clipboard_write::set_files(&validated_paths, is_cut)
}

pub fn set_clipboard_text(text: String) -> Result<(), CommandError> {
    super::windows_clipboard_write::set_text(&text)
}

#[cfg(test)]
mod read_tests {
    use super::*;

    fn fixture(names: &[&str], offset: usize) -> Vec<u8> {
        let mut bytes = vec![0; offset];
        bytes[..4].copy_from_slice(&(offset as u32).to_le_bytes());
        let wide = std::mem::offset_of!(DROPFILES, fWide);
        bytes[wide..wide + 4].copy_from_slice(&1_u32.to_le_bytes());
        for name in names {
            for unit in name.encode_utf16().chain(Some(0)) {
                bytes.extend_from_slice(&unit.to_le_bytes());
            }
        }
        if names.is_empty() {
            bytes.extend_from_slice(&0_u16.to_le_bytes());
        }
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes
    }

    #[test]
    fn valid_unicode_and_unc_lists_allow_unaligned_offsets_and_unused_padding() {
        let names = [
            r"C:\école\Fichier 🦀.txt",
            r"\\serveur\partage\Deuxième.txt",
        ];
        for offset in [
            std::mem::size_of::<DROPFILES>(),
            std::mem::size_of::<DROPFILES>() + 1,
        ] {
            let bytes = fixture(&names, offset);
            assert_eq!(parse_dropfiles(&bytes).unwrap(), names);
            let mut padded = bytes.clone();
            padded.extend_from_slice(&[0xff, 0xff, 0x7f]);
            assert_eq!(parse_dropfiles(&padded).unwrap(), names);
            let mut unaligned = vec![0x7f];
            unaligned.extend_from_slice(&bytes);
            assert_eq!(parse_dropfiles(&unaligned[1..]).unwrap(), names);
        }
    }

    #[test]
    fn every_truncated_prefix_of_a_valid_list_is_rejected() {
        let bytes = fixture(
            &[r"C:\First.txt", r"C:\Second école.txt"],
            std::mem::size_of::<DROPFILES>(),
        );
        for length in 0..bytes.len() {
            assert!(
                parse_dropfiles(&bytes[..length]).is_err(),
                "prefix length {length}"
            );
        }
        assert_eq!(parse_dropfiles(&bytes).unwrap().len(), 2);
    }

    #[test]
    fn offsets_into_header_or_beyond_allocation_are_rejected() {
        let original = fixture(&[r"C:\File.txt"], std::mem::size_of::<DROPFILES>());
        for offset in (0..std::mem::size_of::<DROPFILES>() as u32).chain([
            original.len() as u32,
            original.len() as u32 + 1,
            u32::MAX,
        ]) {
            let mut bytes = original.clone();
            bytes[..4].copy_from_slice(&offset.to_le_bytes());
            assert!(parse_dropfiles(&bytes).is_err(), "offset {offset}");
        }
    }

    #[test]
    fn invalid_utf16_after_a_valid_path_rejects_the_entire_list() {
        let mut bytes = fixture(&[r"C:\First.txt"], std::mem::size_of::<DROPFILES>());
        bytes.truncate(bytes.len() - 2);
        for unit in [0xd800_u16, 0, 0] {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        assert!(parse_dropfiles(&bytes).is_err());
    }

    #[test]
    fn empty_lists_require_two_nulls_and_ansi_lists_remain_unsupported() {
        let empty = fixture(&[], std::mem::size_of::<DROPFILES>());
        assert!(parse_dropfiles(&empty).unwrap().is_empty());
        assert!(parse_dropfiles(&empty[..empty.len() - 2]).is_err());
        let mut ansi = fixture(&[r"C:\File.txt"], std::mem::size_of::<DROPFILES>());
        let wide = std::mem::offset_of!(DROPFILES, fWide);
        ansi[wide..wide + 4].fill(0);
        ansi.truncate(std::mem::size_of::<DROPFILES>());
        ansi.extend_from_slice(b"C:\\File.txt\0\0");
        assert!(parse_dropfiles(&ansi).unwrap().is_empty());
    }

    #[test]
    fn truncated_drop_effect_is_rejected_without_implicitly_copying() {
        let effect = 2_u32.to_le_bytes();
        for length in 0..4 {
            assert!(parse_drop_effect(&effect[..length]).is_err());
        }
        assert!(parse_drop_effect(&effect).unwrap());
        for value in [0_u32, 1, 3, u32::MAX] {
            assert!(!parse_drop_effect(&value.to_le_bytes()).unwrap());
        }
        assert!(parse_drop_effect(&[2, 0, 0, 0, 0xff]).unwrap());
    }

    struct OwnedFixture(HGLOBAL);
    impl OwnedFixture {
        fn new(bytes: &[u8]) -> Self {
            use windows::Win32::System::Memory::{GlobalAlloc, GMEM_MOVEABLE};
            let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }.unwrap();
            let fixture = Self(handle);
            let pointer = unsafe { GlobalLock(handle) }.cast::<u8>();
            assert!(!pointer.is_null());
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len());
                let _ = GlobalUnlock(handle);
            }
            fixture
        }
    }
    impl Drop for OwnedFixture {
        fn drop(&mut self) {
            // GlobalFree succeeds with NULL; this binding maps it to Err(S_OK).
            let result = unsafe {
                windows::Win32::Foundation::SetLastError(windows::Win32::Foundation::ERROR_SUCCESS);
                windows::Win32::Foundation::GlobalFree(Some(self.0))
            };
            assert!(
                matches!(&result, Err(error) if error.code().is_ok()),
                "fixture free failed: {result:?}"
            );
        }
    }

    #[test]
    fn native_borrowed_buffers_are_unlocked_but_never_freed_by_the_reader() {
        use windows::Win32::System::Memory::GlobalFlags;
        let names = [r"C:\Fichier école 🦀.txt", r"\\serveur\partage\Fichier.txt"];
        let valid = fixture(&names, std::mem::size_of::<DROPFILES>());
        for bytes in [valid, vec![0; std::mem::size_of::<DROPFILES>()]] {
            let owned = OwnedFixture::new(&bytes);
            let result = {
                let borrowed = unsafe { LockedClipboardData::new(HANDLE(owned.0 .0)) }.unwrap();
                assert_eq!(unsafe { GlobalFlags(owned.0) } & 0xff, 1);
                parse_dropfiles(borrowed.bytes())
            };
            assert_eq!(unsafe { GlobalFlags(owned.0) } & 0xff, 0);
            assert!(unsafe { GlobalSize(owned.0) } >= bytes.len());
            if bytes[0] == 0 {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap(), names);
            }
        }
    }
}
