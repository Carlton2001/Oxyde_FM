//! Publish the Recycle Bin's own Shell identities, not filesystem `$R` items.
use std::path::{Path, PathBuf};
use windows::Win32::System::Com::{
    CoTaskMemFree, IDataObject, DVASPECT_CONTENT, FORMATETC, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::{
    OleFlushClipboard, OleInitialize, OleSetClipboard, OleUninitialize, ReleaseStgMedium,
};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    BHID_EnumItems, BHID_SFObject, FOLDERID_RecycleBinFolder, IEnumShellItems, ILFindLastID,
    IShellFolder, IShellItem, SHGetIDListFromObject, SHGetKnownFolderItem, KF_FLAG_DEFAULT,
    SIGDN_DESKTOPABSOLUTEPARSING,
};

struct OleApartment;
impl OleApartment {
    fn new() -> Result<Self, String> {
        unsafe { OleInitialize(None) }.map_err(|error| error.to_string())?;
        Ok(Self)
    }
}
impl Drop for OleApartment {
    fn drop(&mut self) {
        unsafe { OleUninitialize() };
    }
}
struct Pidl(*mut ITEMIDLIST);
impl Drop for Pidl {
    fn drop(&mut self) {
        unsafe { CoTaskMemFree(Some(self.0.cast())) };
    }
}
fn normalize(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_start_matches("\\\\?\\")
        .to_lowercase()
}

// Caller holds an OLE apartment until the returned interfaces have been dropped.
fn create_data_object(paths: &[PathBuf], cut: bool) -> Result<IDataObject, String> {
    unsafe {
        let folder: IShellItem =
            SHGetKnownFolderItem(&FOLDERID_RecycleBinFolder, KF_FLAG_DEFAULT, None)
                .map_err(|error| error.to_string())?;
        let enumerator: IEnumShellItems = folder
            .BindToHandler(None, &BHID_EnumItems)
            .map_err(|error| error.to_string())?;
        let mut selected = Vec::new();
        loop {
            let mut items = [None];
            let mut fetched = 0;
            enumerator
                .Next(&mut items, Some(&mut fetched))
                .map_err(|error| error.to_string())?;
            if fetched == 0 {
                break;
            }
            let item = items[0].take().ok_or("Missing Shell item")?;
            let text = item
                .GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING)
                .map_err(|error| error.to_string())?;
            let name = text.to_string().map_err(|error| error.to_string());
            CoTaskMemFree(Some(text.0.cast()));
            let name = name?;
            if paths
                .iter()
                .any(|path| normalize(path) == normalize(Path::new(&name)))
            {
                selected.push(Pidl(
                    SHGetIDListFromObject(&item).map_err(|error| error.to_string())?,
                ));
            }
        }
        if selected.len() != paths.len() {
            return Err("One or more selected Recycle Bin items are no longer available".into());
        }
        let pointers: Vec<_> = selected
            .iter()
            .map(|pidl| pidl.0 as *const ITEMIDLIST)
            .collect();
        let native_folder: IShellFolder = folder
            .BindToHandler(None, &BHID_SFObject)
            .map_err(|error| error.to_string())?;
        let children: Vec<_> = pointers
            .iter()
            .map(|pidl| ILFindLastID(*pidl) as *const ITEMIDLIST)
            .collect();
        let object: IDataObject = native_folder
            .GetUIObjectOf(windows::Win32::Foundation::HWND::default(), &children, None)
            .map_err(|error| error.to_string())?;
        set_effect(&object, c"Preferred DropEffect", if cut { 2 } else { 1 })?;
        Ok(object)
    }
}

fn set_effect(object: &IDataObject, name: &std::ffi::CStr, effect: u32) -> Result<(), String> {
    use windows::core::PCSTR;
    use windows::Win32::System::DataExchange::RegisterClipboardFormatA;
    unsafe {
        let format = RegisterClipboardFormatA(PCSTR(name.as_ptr().cast()));
        if format == 0 {
            return Err("Cannot register the clipboard drop effect".into());
        }
        let memory = GlobalAlloc(GMEM_MOVEABLE, 4).map_err(|error| error.to_string())?;
        let mut medium = STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: memory },
            pUnkForRelease: std::mem::ManuallyDrop::new(None),
        };
        let pointer = GlobalLock(memory);
        if pointer.is_null() {
            ReleaseStgMedium(&mut medium);
            return Err("Cannot lock the clipboard drop effect".into());
        }
        pointer.cast::<u32>().write(effect);
        let _ = GlobalUnlock(memory);
        let descriptor = FORMATETC {
            cfFormat: format as u16,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        if let Err(error) = object.SetData(&descriptor, &medium, true) {
            ReleaseStgMedium(&mut medium);
            return Err(error.to_string());
        }
        Ok(())
    }
}

pub(super) fn set_files(paths: Vec<PathBuf>, cut: bool) -> Result<(), String> {
    // Tokio workers may already use MTA COM. This short-lived STA renders all
    // clipboard formats before exiting, so no worker must pump OLE messages.
    std::thread::spawn(move || {
        let _apartment = OleApartment::new()?;
        let object = create_data_object(&paths, cut)?;
        unsafe {
            OleSetClipboard(&object).map_err(|error| error.to_string())?;
            OleFlushClipboard().map_err(|error| error.to_string())?;
        }
        Ok(())
    })
    .join()
    .map_err(|_| "Shell clipboard worker panicked".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::SHCreateItemFromParsingName;

    #[test]
    #[ignore = "checks the Explorer Shell transfer consumer using generated fixtures, without replacing the clipboard"]
    fn native_shell_data_object_preserves_trash_names_and_metadata() {
        let _apartment = OleApartment::new().unwrap();
        let root = tempfile::tempdir().unwrap();
        let originals: Vec<_> = [
            "Rapport école 🦀.txt",
            "Raccourci école.lnk",
            "Lien école.url",
            "Dossier école",
        ]
        .iter()
        .map(|name| root.path().join(name))
        .collect();
        for source in &originals[..3] {
            std::fs::write(source, b"Shell transfer fixture").unwrap();
        }
        std::fs::create_dir(&originals[3]).unwrap();
        std::fs::write(originals[3].join("contenu.txt"), b"nested Shell fixture").unwrap();
        super::super::platform()
            .shell_delete(&originals, true)
            .unwrap();
        struct Cleanup(Vec<super::super::TrashTransferEntry>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let entries = self.0.clone();
                let _ = std::thread::spawn(move || {
                    if let Ok(items) = crate::commands::ops::list_trash() {
                        let ids = items
                            .into_iter()
                            .filter(|item| {
                                entries.iter().any(|entry| {
                                    entry.original_path == PathBuf::from(&item.original_path)
                                })
                            })
                            .map(|item| item.path)
                            .collect();
                        let _ = tokio::runtime::Runtime::new()
                            .unwrap()
                            .block_on(crate::commands::ops::restore_items(ids));
                        // Only sidecars captured for this generated fixture.
                        for entry in entries {
                            if !entry.path.exists()
                                && super::super::windows_trash::original_path(&entry.path).as_ref()
                                    == Some(&entry.original_path)
                            {
                                let _ = std::fs::remove_file(entry.metadata_path);
                            }
                        }
                    }
                })
                .join();
            }
        }
        let items = crate::commands::ops::list_trash().unwrap();
        let physical: Vec<_> = originals
            .iter()
            .map(|source| {
                PathBuf::from(
                    &items
                        .iter()
                        .find(|item| PathBuf::from(&item.original_path) == *source)
                        .unwrap()
                        .path,
                )
            })
            .collect();
        let entries = super::super::platform()
            .trash_transfer_entries(&physical)
            .unwrap();
        let _cleanup = Cleanup(entries.clone());
        for cut in [false, true] {
            let destination = root.path().join(if cut { "move" } else { "copy" });
            std::fs::create_dir(&destination).unwrap();
            let source_object = create_data_object(&physical, cut).unwrap();
            let object: IDataObject = unsafe {
                windows::Win32::UI::Shell::SHCreateDataObject(None, Some(&[]), None::<&IDataObject>)
                    .unwrap()
            };
            for name in [c"Shell IDList Array", c"Preferred DropEffect"] {
                unsafe {
                    use windows::core::PCSTR;
                    let format = windows::Win32::System::DataExchange::RegisterClipboardFormatA(
                        PCSTR(name.as_ptr().cast()),
                    );
                    let descriptor = FORMATETC {
                        cfFormat: format as u16,
                        ptd: std::ptr::null_mut(),
                        dwAspect: DVASPECT_CONTENT.0,
                        lindex: -1,
                        tymed: TYMED_HGLOBAL.0 as u32,
                    };
                    let mut medium = source_object.GetData(&descriptor).unwrap();
                    if let Err(error) = object.SetData(&descriptor, &medium, true) {
                        ReleaseStgMedium(&mut medium);
                        panic!("Cannot persist Shell format: {error}");
                    }
                }
            }
            drop(source_object);
            // CF_HDROP remains available for Oxyde; Shell IDList carries the
            // original namespace names to native consumers such as Explorer.
            unsafe {
                use windows::Win32::System::Memory::GlobalSize;
                use windows::Win32::System::Ole::CF_HDROP;
                let format = FORMATETC {
                    cfFormat: CF_HDROP.0,
                    ptd: std::ptr::null_mut(),
                    dwAspect: DVASPECT_CONTENT.0,
                    lindex: -1,
                    tymed: TYMED_HGLOBAL.0 as u32,
                };
                let hdrop_object = create_data_object(&physical, cut).unwrap();
                let mut medium = hdrop_object.GetData(&format).unwrap();
                assert!(
                    GlobalSize(medium.u.hGlobal)
                        > std::mem::size_of::<windows::Win32::UI::Shell::DROPFILES>()
                );
                let drop = windows::Win32::UI::Shell::HDROP(medium.u.hGlobal.0);
                let count = windows::Win32::UI::Shell::DragQueryFileW(drop, u32::MAX, None);
                let mut received = Vec::new();
                for index in 0..count {
                    let length = windows::Win32::UI::Shell::DragQueryFileW(drop, index, None);
                    let mut wide = vec![0u16; length as usize + 1];
                    windows::Win32::UI::Shell::DragQueryFileW(drop, index, Some(&mut wide));
                    received.push(normalize(Path::new(
                        &String::from_utf16(&wide[..length as usize]).unwrap(),
                    )));
                }
                let mut expected: Vec<_> = physical.iter().map(|path| normalize(path)).collect();
                received.sort();
                expected.sort();
                assert_eq!(received, expected);
                ReleaseStgMedium(&mut medium);
                let wide: Vec<_> = destination.as_os_str().encode_wide().chain([0]).collect();
                let target: IShellItem =
                    SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).unwrap();
                use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
                use windows::Win32::UI::Shell::{FileOperation, IFileOperation, FOF_NO_UI};
                let operation: IFileOperation =
                    CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER).unwrap();
                operation.SetOperationFlags(FOF_NO_UI).unwrap();
                if cut {
                    operation.MoveItems(&object, &target).unwrap();
                } else {
                    operation.CopyItems(&object, &target).unwrap();
                }
                operation.PerformOperations().unwrap();
                assert!(!operation.GetAnyOperationsAborted().unwrap().as_bool());
                // Explorer reports the actual outcome to the source data object.
                set_effect(&object, c"Performed DropEffect", if cut { 2 } else { 1 }).unwrap();
                set_effect(&object, c"Paste Succeeded", if cut { 2 } else { 1 }).unwrap();
            }
            for source in &originals[..3] {
                assert_eq!(
                    std::fs::read(destination.join(source.file_name().unwrap())).unwrap(),
                    b"Shell transfer fixture"
                );
            }
            assert_eq!(
                std::fs::read(
                    destination
                        .join(originals[3].file_name().unwrap())
                        .join("contenu.txt")
                )
                .unwrap(),
                b"nested Shell fixture"
            );
            assert_eq!(
                std::fs::read_dir(&destination).unwrap().count(),
                originals.len()
            );
            for entry in &entries {
                assert_eq!(entry.path.exists(), !cut);
                if !cut {
                    assert!(entry.metadata_path.exists());
                }
            }
        }
        let remaining = crate::commands::ops::list_trash().unwrap();
        assert!(remaining
            .iter()
            .all(|item| !physical.contains(&PathBuf::from(&item.path))));
    }
}
