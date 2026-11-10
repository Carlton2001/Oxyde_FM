//! Oxyde in place of the Open and Save dialogs of Windows applications (experimental).
//!
//! Oxyde registers this library for the user as the server of the two classes of the
//! common item dialog, which `comdlg32.dll` serves for the machine. An application
//! that creates one loads the library instead, and gets the system's own dialog,
//! created straight from `comdlg32.dll`, inside a wrapper (`dialog`). The wrapper asks
//! Oxyde through its launcher (`ask`) when the dialog is a plain one, and shows the
//! system's dialog in every other case, any failure included.
//!
//! The library runs inside other applications: it starts no thread of its own but the
//! one that reads the launcher's answer, keeps no state between dialogs, and never
//! unloads. It writes what it does to the file `OXYDE_FILE_DIALOG_LOG` names, when
//! the application's environment has that variable.
#![cfg(windows)]
#![allow(non_snake_case)]

mod ask;
mod dialog;
mod items;
mod request;

use core::ffi::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::OnceLock;
use windows::core::{IUnknown, IUnknown_Vtbl, Interface, BOOL, GUID, HRESULT, HSTRING, PCSTR};
use windows::Win32::Foundation::{CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_NOINTERFACE, E_POINTER, HMODULE, MAX_PATH, S_FALSE, S_OK};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Vtbl};
use windows::Win32::System::LibraryLoader::{DisableThreadLibraryCalls, GetModuleFileNameW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows::Win32::UI::Shell::IFileDialog;

/// `CLSID_FileOpenDialog` and `CLSID_FileSaveDialog`.
const OPEN: GUID = GUID::from_u128(0xDC1C5A9C_E88A_4DDE_A5A1_60F82A20AEF7);
const SAVE: GUID = GUID::from_u128(0xC0B4E2F3_BA21_4773_8DBA_335EC946EB8B);

static MODULE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

/// Where this library is, which is where the launcher is.
pub(crate) fn module_path() -> Option<std::path::PathBuf> {
    let module = MODULE.load(Ordering::Relaxed);
    if module.is_null() {
        return None;
    }
    let mut path = vec![0u16; 32 * 1024];
    let length = unsafe { GetModuleFileNameW(Some(HMODULE(module)), &mut path) } as usize;
    (length > 0 && length < path.len()).then(|| std::path::PathBuf::from(String::from_utf16_lossy(&path[..length])))
}

/// Whether the application asking is a program of Oxyde's own folder. Oxyde keeps the
/// system's dialogs: answering itself, it would wait for its own interface.
pub(crate) fn within_oxyde() -> bool {
    static WITHIN: OnceLock<bool> = OnceLock::new();
    *WITHIN.get_or_init(|| {
        let folder = |path: std::path::PathBuf| path.parent().map(|folder| folder.to_string_lossy().trim_start_matches(r"\\?\").to_lowercase());
        let library = module_path().and_then(folder);
        library.is_some() && library == std::env::current_exe().ok().and_then(folder)
    })
}

/// The user's registration of the Open dialog, which Oxyde writes and removes along
/// with the Save one.
const REGISTRATION: windows::core::PCWSTR = windows::core::w!(r"Software\Classes\CLSID\{DC1C5A9C-E88A-4DDE-A5A1-60F82A20AEF7}\InprocServer32");

/// Whether the user's registration names this library.
fn registered_here() -> bool {
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
    let mut text = vec![0u16; 32 * 1024];
    let mut size = (text.len() * 2) as u32;
    let read = unsafe { RegGetValueW(HKEY_CURRENT_USER, REGISTRATION, None, RRF_RT_REG_SZ, None, Some(text.as_mut_ptr().cast()), Some(&mut size)) };
    if read.is_err() {
        return false;
    }
    let registered = String::from_utf16_lossy(&text[..(size as usize / 2).saturating_sub(1)]);
    module_path().is_some_and(|path| path.to_string_lossy().trim_start_matches(r"\\?\").eq_ignore_ascii_case(registered.trim_start_matches(r"\\?\")))
}

/// Whether the library was registered when the application loaded it.
static LOADED_REGISTERED: OnceLock<bool> = OnceLock::new();

/// Whether the user turned the option off since the application loaded the library:
/// its dialogs go back to the system's at once, without restarting it.
pub(crate) fn withdrawn() -> bool {
    LOADED_REGISTERED.get().copied().unwrap_or(false) && !registered_here()
}

/// Appends a line to the log the environment names, if it names one.
pub(crate) fn log(line: impl FnOnce() -> String) {
    static LOG: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    let Some(path) = LOG.get_or_init(|| std::env::var_os("OXYDE_FILE_DIALOG_LOG").map(Into::into)) else { return };
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = std::io::Write::write_all(&mut file, format!("[{}] {}\n", std::process::id(), line()).as_bytes());
    }
}

type GetClassObject = unsafe extern "system" fn(*const GUID, *const GUID, *mut *mut c_void) -> HRESULT;

/// `DllGetClassObject` of the system's `comdlg32.dll`, named in full: the user's
/// registration, which leads here, is not consulted.
fn system_classes() -> Option<GetClassObject> {
    static CLASSES: OnceLock<Option<usize>> = OnceLock::new();
    let found = CLASSES.get_or_init(|| unsafe {
        let mut folder = [0u16; MAX_PATH as usize];
        let length = GetSystemDirectoryW(Some(&mut folder)) as usize;
        if length == 0 || length >= folder.len() {
            return None;
        }
        let library = format!(r"{}\comdlg32.dll", String::from_utf16_lossy(&folder[..length]));
        let module = LoadLibraryW(&HSTRING::from(library)).ok()?;
        GetProcAddress(module, PCSTR(c"DllGetClassObject".as_ptr().cast())).map(|function| function as usize)
    });
    found.map(|function| unsafe { std::mem::transmute::<usize, GetClassObject>(function) })
}

/// The system's dialog of `class`.
unsafe fn system_dialog(class: &GUID) -> windows::core::Result<IFileDialog> {
    let classes = system_classes().ok_or(E_FAIL)?;
    let mut factory = std::ptr::null_mut();
    classes(class, &IClassFactory::IID, &mut factory).ok()?;
    let factory = IClassFactory::from_raw(factory);
    factory.CreateInstance(None)
}

/// The factory of one of the two classes; it lives as long as the library.
#[repr(C)]
struct Factory {
    table: &'static IClassFactory_Vtbl,
    class: GUID,
}
unsafe impl Sync for Factory {}

unsafe extern "system" fn query(this: *mut c_void, iid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    if iid.is_null() || out.is_null() {
        return E_POINTER;
    }
    if *iid == IUnknown::IID || *iid == IClassFactory::IID {
        *out = this;
        S_OK
    } else {
        *out = std::ptr::null_mut();
        E_NOINTERFACE
    }
}

unsafe extern "system" fn counted(_this: *mut c_void) -> u32 {
    1
}

unsafe extern "system" fn create(this: *mut c_void, outer: *mut c_void, iid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    if iid.is_null() || out.is_null() {
        return E_POINTER;
    }
    *out = std::ptr::null_mut();
    if !outer.is_null() {
        return CLASS_E_NOAGGREGATION;
    }
    let class = &(*this.cast::<Factory>()).class;
    match system_dialog(class) {
        Ok(inner) => dialog::Dialog::create(inner, iid, out),
        Err(error) => {
            log(|| format!("the system's dialog cannot be created: {error}"));
            error.code()
        }
    }
}

unsafe extern "system" fn lock(_this: *mut c_void, _lock: BOOL) -> HRESULT {
    S_OK
}

static FACTORY: IClassFactory_Vtbl = IClassFactory_Vtbl {
    base__: IUnknown_Vtbl { QueryInterface: query, AddRef: counted, Release: counted },
    CreateInstance: create,
    LockServer: lock,
};
static OPENS: Factory = Factory { table: &FACTORY, class: OPEN };
static SAVES: Factory = Factory { table: &FACTORY, class: SAVE };

#[no_mangle]
unsafe extern "system" fn DllMain(module: HMODULE, reason: u32, _reserved: *mut c_void) -> BOOL {
    const PROCESS_ATTACH: u32 = 1;
    if reason == PROCESS_ATTACH {
        MODULE.store(module.0, Ordering::Relaxed);
        let _ = DisableThreadLibraryCalls(module);
    }
    true.into()
}

#[no_mangle]
unsafe extern "system" fn DllGetClassObject(class: *const GUID, iid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    if class.is_null() || iid.is_null() || out.is_null() {
        return E_POINTER;
    }
    LOADED_REGISTERED.get_or_init(registered_here);
    let factory = if *class == OPEN {
        &OPENS
    } else if *class == SAVE {
        &SAVES
    } else {
        *out = std::ptr::null_mut();
        return CLASS_E_CLASSNOTAVAILABLE;
    };
    query((factory as *const Factory).cast_mut().cast(), iid, out)
}

/// Dialogs and the thread reading an answer may outlive what COM counts: the library
/// stays loaded.
#[no_mangle]
extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_FALSE
}
