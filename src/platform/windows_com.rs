//! Thread-bound COM initialization and uniquely owned task-allocator resources.
use std::marker::PhantomData;
use std::rc::Rc;
use windows::core::{Error, HRESULT, PWSTR};
use windows::Win32::Foundation::{E_POINTER, RPC_E_CHANGED_MODE};
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PROPVARIANT};
use windows::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT, COINIT_APARTMENTTHREADED,
    COINIT_MULTITHREADED,
};

pub(super) struct ComApartment {
    release: Option<fn()>,
    // Initialization must be balanced on the creating thread.
    _thread: PhantomData<Rc<()>>,
}
fn release_com() {
    unsafe { CoUninitialize() };
}
impl ComApartment {
    fn from_result(result: HRESULT, reuse_existing: bool, release: fn()) -> Result<Self, Error> {
        let release = if result.is_ok() {
            Some(release)
        } else if reuse_existing && result == RPC_E_CHANGED_MODE {
            None
        } else {
            return Err(Error::from_hresult(result));
        };
        Ok(Self {
            release,
            _thread: PhantomData,
        })
    }
    fn enter(mode: COINIT, reuse_existing: bool) -> Result<Self, Error> {
        Self::from_result(
            unsafe { CoInitializeEx(None, mode) },
            reuse_existing,
            release_com,
        )
    }
    pub(super) fn sta() -> Result<Self, Error> {
        Self::enter(COINIT_APARTMENTTHREADED, false)
    }
    /// For operations already supported in either model. A changed model does
    /// not acquire a COM reference and must never trigger CoUninitialize.
    pub(super) fn sta_or_existing() -> Result<Self, Error> {
        Self::enter(COINIT_APARTMENTTHREADED, true)
    }
    pub(super) fn mta_or_existing() -> Result<Self, Error> {
        Self::enter(COINIT_MULTITHREADED, true)
    }
}
impl Drop for ComApartment {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            release();
        }
    }
}

pub(super) struct TaskMemory<T>(*mut T);
impl<T> TaskMemory<T> {
    /// # Safety
    /// Pointer must be a unique, live allocation releasable with CoTaskMemFree.
    pub(super) unsafe fn new(pointer: *mut T) -> Self {
        Self(pointer)
    }
}
impl<T> Drop for TaskMemory<T> {
    fn drop(&mut self) {
        unsafe { CoTaskMemFree(Some(self.0.cast())) };
    }
}

/// # Safety
/// Value must be a uniquely owned, null-terminated task-allocator string.
pub(super) unsafe fn take_string(value: PWSTR) -> Result<String, Error> {
    let _allocation = unsafe { TaskMemory::new(value.0) };
    if value.is_null() {
        return Err(Error::from_hresult(E_POINTER));
    }
    unsafe { Ok(value.to_string()?) }
}

pub(super) struct PropertyValue(PROPVARIANT);
impl PropertyValue {
    /// # Safety
    /// Value must own its contents, typically returned by IPropertyStore::GetValue.
    pub(super) unsafe fn new(value: PROPVARIANT) -> Self {
        Self(value)
    }
    pub(super) fn as_ref(&self) -> &PROPVARIANT {
        &self.0
    }
}
impl Drop for PropertyValue {
    fn drop(&mut self) {
        unsafe {
            let _ = PropVariantClear(&mut self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use windows::Win32::Foundation::{E_OUTOFMEMORY, S_FALSE, S_OK};
    use windows::Win32::System::Com::CoTaskMemAlloc;

    thread_local! {
        static RELEASES: Cell<usize> = const { Cell::new(0) };
    }
    fn count_release() {
        RELEASES.with(|count| count.set(count.get() + 1));
    }

    #[test]
    fn releases_only_successful_initializations_including_s_false() {
        RELEASES.with(|count| count.set(0));
        for result in [S_OK, S_FALSE] {
            drop(ComApartment::from_result(result, false, count_release).unwrap());
        }
        assert_eq!(RELEASES.with(Cell::get), 2);
        for reuse_existing in [false, true] {
            assert_eq!(
                ComApartment::from_result(E_OUTOFMEMORY, reuse_existing, count_release)
                    .err()
                    .unwrap()
                    .code(),
                E_OUTOFMEMORY
            );
        }
        assert!(ComApartment::from_result(RPC_E_CHANGED_MODE, false, count_release).is_err());
        drop(ComApartment::from_result(RPC_E_CHANGED_MODE, true, count_release).unwrap());
        assert_eq!(RELEASES.with(Cell::get), 2);
    }

    #[test]
    fn native_nested_initialization_keeps_outer_reference() {
        std::thread::spawn(|| {
            let outer = ComApartment::sta().unwrap();
            drop(ComApartment::sta().unwrap());
            // A failed probe acquires no reference. The outer STA must remain.
            assert_eq!(
                unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) },
                RPC_E_CHANGED_MODE
            );
            drop(outer);
            let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            assert_eq!(result, S_OK);
            drop(ComApartment::from_result(result, false, release_com).unwrap());
        })
        .join()
        .unwrap();
    }

    #[test]
    fn native_changed_mode_never_releases_worker_apartment() {
        std::thread::spawn(|| {
            let worker = ComApartment::mta_or_existing().unwrap();
            assert_eq!(
                ComApartment::sta().err().unwrap().code(),
                RPC_E_CHANGED_MODE
            );
            drop(ComApartment::sta_or_existing().unwrap());
            assert_eq!(
                unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) },
                RPC_E_CHANGED_MODE
            );
            drop(worker);
            let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            assert_eq!(result, S_OK);
            drop(ComApartment::from_result(result, false, release_com).unwrap());
        })
        .join()
        .unwrap();
    }

    #[test]
    fn native_error_and_unwind_release_initialization() {
        std::thread::spawn(|| {
            fn failed_operation() -> Result<(), Error> {
                let _com = ComApartment::sta()?;
                Err(Error::from_hresult(E_OUTOFMEMORY))
            }
            assert!(failed_operation().is_err());
            let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            assert_eq!(result, S_OK);
            drop(ComApartment::from_result(result, false, release_com).unwrap());

            assert!(std::panic::catch_unwind(|| {
                let _com = ComApartment::sta().unwrap();
                panic!("injected native operation panic");
            })
            .is_err());
            let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            assert_eq!(result, S_OK);
            drop(ComApartment::from_result(result, false, release_com).unwrap());
        })
        .join()
        .unwrap();
    }

    unsafe fn allocated_string(units: &[u16]) -> PWSTR {
        let pointer = unsafe { CoTaskMemAlloc(std::mem::size_of_val(units)) }.cast::<u16>();
        assert!(!pointer.is_null());
        unsafe { std::ptr::copy_nonoverlapping(units.as_ptr(), pointer, units.len()) };
        PWSTR(pointer)
    }

    #[test]
    fn native_owned_strings_support_unicode_and_reject_bad_utf16() {
        let name = "Données 日本語 🦀";
        let units: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        unsafe {
            assert_eq!(take_string(allocated_string(&units)).unwrap(), name);
            assert!(take_string(allocated_string(&[0xD800, 0])).is_err());
            assert_eq!(take_string(PWSTR::null()).unwrap_err().code(), E_POINTER);
        }
    }

    #[test]
    fn native_active_error_paths_preserve_existing_apartment() {
        std::thread::spawn(|| {
            let directory = tempfile::tempdir().unwrap();
            let missing = directory
                .path()
                .join("absent.txt")
                .to_string_lossy()
                .into_owned();
            let worker = ComApartment::mta_or_existing().unwrap();
            assert!(super::super::windows_shell_properties::friendly_name(&missing).is_err());
            assert!(super::super::thumbnails::get_pdf_thumbnail_raw(missing.clone()).is_err());
            assert!(
                super::super::windows_shell::get_native_context_menu_items(missing, false).is_err()
            );
            assert_eq!(
                unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) },
                RPC_E_CHANGED_MODE
            );
            drop(worker);
            let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            assert_eq!(result, S_OK);
            drop(ComApartment::from_result(result, false, release_com).unwrap());
        })
        .join()
        .unwrap();
    }
}
