use super::shell_selection;
use crate::platform::windows_com::ComApartment;

#[test]
fn shell_array_preserves_files_from_different_directories() {
    let _com = ComApartment::sta().unwrap();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let paths: Vec<_> = [
        first.path().join("first.txt"),
        second.path().join("second.txt"),
    ]
    .into_iter()
    .map(|path| {
        std::fs::write(&path, b"selection").unwrap();
        path.to_string_lossy().into_owned()
    })
    .collect();
    let selection = shell_selection(&paths).unwrap();
    assert_eq!(unsafe { selection.GetCount().unwrap() }, 2);
}

#[test]
fn system_properties_data_contains_every_selected_shell_identity() {
    verify_properties_identities(false);
}

#[test]
fn sibling_properties_keep_their_real_parent_and_relative_children() {
    verify_properties_identities(true);
}

fn verify_properties_identities(same_parent: bool) {
    use windows::core::PCWSTR;
    use windows::Win32::System::Com::{DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL};
    use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use windows::Win32::System::Ole::ReleaseStgMedium;
    use windows::Win32::UI::Shell::CFSTR_SHELLIDLIST;
    use windows::Win32::UI::Shell::{ILCombine, SHGetPathFromIDListW};
    let _com = ComApartment::sta().unwrap();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let second_parent = if same_parent {
        first.path()
    } else {
        second.path()
    };
    let paths: Vec<_> = [
        first.path().join("first.zip"),
        second_parent.join("second.zip"),
    ]
    .into_iter()
    .map(|path| {
        std::fs::write(&path, b"properties").unwrap();
        path.to_string_lossy().into_owned()
    })
    .collect();
    let data = super::properties_data_object(&paths).unwrap();
    unsafe {
        let format = RegisterClipboardFormatW(PCWSTR(CFSTR_SHELLIDLIST.as_ptr()));
        assert_ne!(format, 0);
        let descriptor = FORMATETC {
            cfFormat: format as u16,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        data.QueryGetData(&descriptor).ok().unwrap();
        let mut medium = data.GetData(&descriptor).unwrap();
        let memory = medium.u.hGlobal;
        let length = GlobalSize(memory);
        let pointer = GlobalLock(memory);
        let count = if !pointer.is_null() && length >= 16 {
            Some(pointer.cast::<u32>().read_unaligned())
        } else {
            None
        };
        let mut identities = Vec::new();
        let mut parent_path = None;
        if count == Some(2) {
            let offsets = pointer.cast::<u32>();
            let parent_offset = offsets.add(1).read_unaligned() as usize;
            assert!(parent_offset < length);
            let parent = pointer.cast::<u8>().add(parent_offset).cast();
            let mut wide = [0u16; 260];
            if SHGetPathFromIDListW(parent, &mut wide).as_bool() {
                parent_path = Some(String::from_utf16_lossy(
                    &wide[..wide.iter().position(|c| *c == 0).unwrap()],
                ));
            }
            for index in 0..2 {
                let offset = offsets.add(index + 2).read_unaligned() as usize;
                assert!(offset < length);
                let child = pointer.cast::<u8>().add(offset).cast();
                let full = ILCombine(Some(parent), Some(child));
                let _full = super::super::windows_com::TaskMemory::new(full);
                let mut wide = [0u16; 260];
                if !full.is_null() && SHGetPathFromIDListW(full, &mut wide).as_bool() {
                    identities.push(String::from_utf16_lossy(
                        &wide[..wide.iter().position(|c| *c == 0).unwrap()],
                    ));
                }
            }
        }
        if !pointer.is_null() {
            let _ = GlobalUnlock(memory);
        }
        ReleaseStgMedium(&mut medium);
        assert_eq!(count, Some(2), "The properties API must receive both PIDLs");
        let normalize = |path: &str| path.replace('/', "\\").to_lowercase();
        assert_eq!(
            identities.iter().map(|p| normalize(p)).collect::<Vec<_>>(),
            paths.iter().map(|p| normalize(p)).collect::<Vec<_>>()
        );
        if same_parent {
            assert_eq!(
                parent_path.as_deref().map(normalize),
                Some(normalize(first.path().to_str().unwrap()))
            );
        }
    }
}
