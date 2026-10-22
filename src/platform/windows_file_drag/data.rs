//! A file data object owns bytes, never the files. COM gets a fresh allocation per read.
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        System::{Com::*, Memory::*, Ole::*},
        UI::Shell::SHCreateStdEnumFmtEtc,
    },
};

const MAX_PAYLOAD: usize = 32 * 1024 * 1024;

pub(super) fn format(id: u16) -> FORMATETC {
    FORMATETC {
        cfFormat: id,
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
        ..Default::default()
    }
}

pub(super) fn read_global(medium: &STGMEDIUM) -> Result<Vec<u8>> {
    if medium.tymed != TYMED_HGLOBAL.0 as u32 {
        return Err(DV_E_TYMED.into());
    }
    let handle = unsafe { medium.u.hGlobal };
    let length = unsafe { GlobalSize(handle) };
    if length == 0 || length > MAX_PAYLOAD {
        return Err(DV_E_FORMATETC.into());
    }
    let pointer = unsafe { GlobalLock(handle) }.cast::<u8>();
    if pointer.is_null() {
        return Err(Error::from_thread());
    }
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length) }.to_vec();
    let _ = unsafe { GlobalUnlock(handle) };
    Ok(bytes)
}

pub(super) fn medium(bytes: &[u8]) -> Result<STGMEDIUM> {
    let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }?;
    let pointer = unsafe { GlobalLock(handle) }.cast::<u8>();
    if pointer.is_null() {
        let _ = unsafe { GlobalFree(Some(handle)) };
        return Err(Error::from_thread());
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len());
    }
    let _ = unsafe { GlobalUnlock(handle) };
    Ok(STGMEDIUM {
        tymed: TYMED_HGLOBAL.0 as u32,
        u: STGMEDIUM_0 { hGlobal: handle },
        ..Default::default()
    })
}

pub(super) fn read_paths(object: &IDataObject) -> Result<Vec<String>> {
    let mut storage = unsafe { object.GetData(&format(CF_HDROP.0)) }?;
    let result = read_global(&storage).and_then(|bytes| {
        super::super::clipboard::parse_dropfiles(&bytes).map_err(|_| Error::from(DV_E_FORMATETC))
    });
    unsafe {
        ReleaseStgMedium(&mut storage);
    }
    result.and_then(|paths| {
        if paths.is_empty()
            || paths
                .iter()
                .any(|path| !std::path::Path::new(path).is_absolute() || path.contains('\0'))
        {
            Err(DV_E_FORMATETC.into())
        } else {
            Ok(paths)
        }
    })
}

pub(super) fn registered(name: PCWSTR) -> u16 {
    unsafe { windows::Win32::System::DataExchange::RegisterClipboardFormatW(name) as u16 }
}

pub(super) fn set_effect(object: &IDataObject, id: u16, effect: DROPEFFECT) {
    if id == 0 {
        return;
    }
    if let Ok(mut storage) = medium(&effect.0.to_le_bytes()) {
        if unsafe { object.SetData(&format(id), &storage, true) }.is_err() {
            unsafe {
                ReleaseStgMedium(&mut storage);
            }
        }
    }
}

pub(super) type Formats = Rc<RefCell<BTreeMap<u16, Vec<u8>>>>;

#[implement(IDataObject)]
pub(super) struct FileData {
    pub formats: Formats,
}

#[allow(non_snake_case)]
impl IDataObject_Impl for FileData_Impl {
    fn GetData(&self, requested: *const FORMATETC) -> Result<STGMEDIUM> {
        self.QueryGetData(requested).ok()?;
        let id = unsafe { (*requested).cfFormat };
        medium(&self.formats.borrow()[&id])
    }
    fn GetDataHere(&self, _: *const FORMATETC, _: *mut STGMEDIUM) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn QueryGetData(&self, requested: *const FORMATETC) -> HRESULT {
        let Some(requested) = (unsafe { requested.as_ref() }) else {
            return E_POINTER;
        };
        if requested.dwAspect != DVASPECT_CONTENT.0 || requested.lindex != -1 {
            return DV_E_FORMATETC;
        }
        if requested.tymed & TYMED_HGLOBAL.0 as u32 == 0 {
            return DV_E_TYMED;
        }
        if self.formats.borrow().contains_key(&requested.cfFormat) {
            S_OK
        } else {
            DV_E_FORMATETC
        }
    }
    fn GetCanonicalFormatEtc(&self, _: *const FORMATETC, output: *mut FORMATETC) -> HRESULT {
        if let Some(output) = unsafe { output.as_mut() } {
            output.ptd = std::ptr::null_mut();
        }
        DATA_S_SAMEFORMATETC
    }
    fn SetData(
        &self,
        requested: *const FORMATETC,
        storage: *const STGMEDIUM,
        release: BOOL,
    ) -> Result<()> {
        let requested = unsafe { requested.as_ref() }.ok_or_else(|| Error::from(E_POINTER))?;
        let storage = unsafe { storage.as_ref() }.ok_or_else(|| Error::from(E_POINTER))?;
        if requested.dwAspect != DVASPECT_CONTENT.0 || requested.lindex != -1 {
            return Err(DV_E_FORMATETC.into());
        }
        let bytes = read_global(storage)?;
        if self.formats.borrow().values().map(Vec::len).sum::<usize>() + bytes.len() > MAX_PAYLOAD {
            return Err(E_OUTOFMEMORY.into());
        }
        self.formats.borrow_mut().insert(requested.cfFormat, bytes);
        if release.as_bool() {
            // Release a shallow local copy only after accepting ownership of the medium.
            let mut owned = unsafe { std::ptr::read(storage) };
            unsafe {
                ReleaseStgMedium(&mut owned);
            }
        }
        Ok(())
    }
    fn EnumFormatEtc(&self, direction: u32) -> Result<IEnumFORMATETC> {
        if direction != DATADIR_GET.0 as u32 {
            return Err(E_NOTIMPL.into());
        }
        unsafe {
            SHCreateStdEnumFmtEtc(
                &self
                    .formats
                    .borrow()
                    .keys()
                    .map(|id| format(*id))
                    .collect::<Vec<_>>(),
            )
        }
    }
    fn DAdvise(&self, _: *const FORMATETC, _: u32, _: Ref<IAdviseSink>) -> Result<u32> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }
    fn DUnadvise(&self, _: u32) -> Result<()> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }
    fn EnumDAdvise(&self) -> Result<IEnumSTATDATA> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }
}
