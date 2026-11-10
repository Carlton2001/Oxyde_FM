//! "Show in folder" for the launcher of folders. A program that shows a file in its
//! folder (`SHOpenFolderAndSelectItems`) opens the folder, which starts the launcher,
//! then looks among the Shell's windows for the one showing that folder and asks its
//! view to select the file. Explorer registers its windows there; so does the launcher,
//! as the launcher of Files (files-community) does: a window nobody sees, said to show
//! the folder, whose view only notes what it is asked to select. The folder is handed
//! to Oxyde meanwhile, and the items after it.
//!
//! The Shell finds a window by the identifier list of its folder, compared as it is
//! written, and a folder of the user's profile has two: one through the drive, as
//! `ILCreateFromPath` writes it (Firefox, Zed), one through the known folder it is in,
//! as `SHParseDisplayName` does (Explorer, Task Manager). The launcher lists a window
//! for each.
use super::windows_com::ComApartment;
use std::cell::RefCell;
use std::rc::Rc;
use windows::core::{implement, w, IUnknownImpl, Interface, Ref, BOOL, BSTR, GUID, HRESULT, HSTRING, PCWSTR};
use windows::Win32::Foundation::{
    DISP_E_MEMBERNOTFOUND, DISP_E_UNKNOWNNAME, E_NOINTERFACE, E_NOTIMPL, E_POINTER, HWND, LPARAM, RECT, SHANDLE_PTR, VARIANT_BOOL,
    VARIANT_FALSE, VARIANT_TRUE,
};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CLOAK};
use windows::Win32::System::Com::{
    CoCreateInstance, CoTaskMemFree, IDispatch, IDispatch_Impl, IServiceProvider, IServiceProvider_Impl, ITypeInfo, CLSCTX_ALL,
    DISPATCH_FLAGS, DISPPARAMS, EXCEPINFO,
};
use windows::Win32::System::Ole::IOleWindow_Impl;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::System::Variant::{InitVariantFromBuffer, VARIANT};
use windows::Win32::UI::Controls::LPFNSVADDPROPSHEETPAGE;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    ILCombine, ILCreateFromPathW, ILGetSize, ILIsEqual, IShellBrowser, IShellItem, IShellView, IShellView_Impl, IShellWindows, IWebBrowserApp, IWebBrowserApp_Impl,
    IWebBrowser_Impl, SHCreateItemFromIDList, SHParseDisplayName, ShellWindows, FOLDERSETTINGS, SIGDN_FILESYSPATH, SVSI_EDIT,
    SVSI_ENSUREVISIBLE, SVSI_FOCUSED, SVSI_SELECT, SWC_BROWSER, _SVGIO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, GetMessageW, SetTimer, TranslateMessage, MSG, WINDOW_STYLE, WM_TIMER,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

/// The timer that ends the wait.
const TIMER: usize = 1;
/// How long a program may take to ask for its selection once the folder is opened. It
/// usually asks at once; Files waits as long for those that look for the window late.
const PATIENCE: u32 = 10_000;
/// How long after an item the next one of the same selection may come. The Shell asks
/// for them in one go, a call each; the user waits this long to see the selection.
const SETTLE: u32 = 20;

/// A Shell item identifier list, freed with the Shell's allocator.
struct IdList(*mut ITEMIDLIST);
impl Drop for IdList {
    fn drop(&mut self) { unsafe { CoTaskMemFree(Some(self.0.cast())) } }
}

/// What a window and its view share: the folder said to be shown, as one of its
/// identifier lists, and the items asked to be selected in it, which the windows of
/// both lists note together.
struct Shown {
    window: HWND,
    folder: IdList,
    selection: Rc<Selection>,
}

struct Selection {
    items: RefCell<Vec<String>>,
    /// The window whose timer ends the wait.
    clock: HWND,
}

/// The window as the Shell sees it: a browser window, and the view inside it.
#[implement(IWebBrowserApp, IServiceProvider, IShellView)]
struct ShellWindow(Rc<Shown>);

impl ShellWindow_Impl {
    fn this<T: Interface>(&self) -> windows::core::Result<T> {
        let mut interface = std::ptr::null_mut();
        unsafe {
            self.QueryInterface(&T::IID, &mut interface).ok()?;
            Ok(T::from_raw(interface))
        }
    }
}

fn unsupported<T>() -> windows::core::Result<T> {
    Err(E_NOTIMPL.into())
}

impl IDispatch_Impl for ShellWindow_Impl {
    fn GetTypeInfoCount(&self) -> windows::core::Result<u32> { Ok(0) }
    fn GetTypeInfo(&self, _: u32, _: u32) -> windows::core::Result<ITypeInfo> { unsupported() }
    fn GetIDsOfNames(&self, _: *const GUID, _: *const PCWSTR, _: u32, _: u32, _: *mut i32) -> windows::core::Result<()> {
        Err(DISP_E_UNKNOWNNAME.into())
    }
    fn Invoke(&self, _: i32, _: *const GUID, _: u32, _: DISPATCH_FLAGS, _: *const DISPPARAMS, _: *mut VARIANT, _: *mut EXCEPINFO, _: *mut u32) -> windows::core::Result<()> {
        Err(DISP_E_MEMBERNOTFOUND.into())
    }
}

impl IWebBrowser_Impl for ShellWindow_Impl {
    fn GoBack(&self) -> windows::core::Result<()> { unsupported() }
    fn GoForward(&self) -> windows::core::Result<()> { unsupported() }
    fn GoHome(&self) -> windows::core::Result<()> { unsupported() }
    fn GoSearch(&self) -> windows::core::Result<()> { unsupported() }
    fn Navigate(&self, _: &BSTR, _: *const VARIANT, _: *const VARIANT, _: *const VARIANT, _: *const VARIANT) -> windows::core::Result<()> { unsupported() }
    fn Refresh(&self) -> windows::core::Result<()> { Ok(()) }
    fn Refresh2(&self, _: *const VARIANT) -> windows::core::Result<()> { Ok(()) }
    fn Stop(&self) -> windows::core::Result<()> { Ok(()) }
    fn Application(&self) -> windows::core::Result<IDispatch> { self.this() }
    fn Parent(&self) -> windows::core::Result<IDispatch> { self.this() }
    fn Container(&self) -> windows::core::Result<IDispatch> { self.this() }
    fn Document(&self) -> windows::core::Result<IDispatch> { self.this() }
    fn TopLevelContainer(&self) -> windows::core::Result<VARIANT_BOOL> { Ok(VARIANT_TRUE) }
    fn Type(&self) -> windows::core::Result<BSTR> { Ok(BSTR::from("Oxyde")) }
    fn Left(&self) -> windows::core::Result<i32> { Ok(0) }
    fn SetLeft(&self, _: i32) -> windows::core::Result<()> { unsupported() }
    fn Top(&self) -> windows::core::Result<i32> { Ok(0) }
    fn SetTop(&self, _: i32) -> windows::core::Result<()> { unsupported() }
    fn Width(&self) -> windows::core::Result<i32> { Ok(0) }
    fn SetWidth(&self, _: i32) -> windows::core::Result<()> { unsupported() }
    fn Height(&self) -> windows::core::Result<i32> { Ok(0) }
    fn SetHeight(&self, _: i32) -> windows::core::Result<()> { unsupported() }
    fn LocationName(&self) -> windows::core::Result<BSTR> { Ok(BSTR::new()) }
    fn LocationURL(&self) -> windows::core::Result<BSTR> { Ok(BSTR::new()) }
    fn Busy(&self) -> windows::core::Result<VARIANT_BOOL> { Ok(VARIANT_FALSE) }
}

impl IWebBrowserApp_Impl for ShellWindow_Impl {
    fn Quit(&self) -> windows::core::Result<()> { Ok(()) }
    fn ClientToWindow(&self, width: *mut i32, height: *mut i32) -> windows::core::Result<()> {
        if width.is_null() || height.is_null() { Err(E_POINTER.into()) } else { Ok(()) }
    }
    fn PutProperty(&self, _: &BSTR, _: &VARIANT) -> windows::core::Result<()> { unsupported() }
    fn GetProperty(&self, _: &BSTR) -> windows::core::Result<VARIANT> { unsupported() }
    fn Name(&self) -> windows::core::Result<BSTR> { Ok(BSTR::from("Oxyde")) }
    fn HWND(&self) -> windows::core::Result<SHANDLE_PTR> { Ok(SHANDLE_PTR(self.0.window.0 as isize)) }
    fn FullName(&self) -> windows::core::Result<BSTR> {
        Ok(BSTR::from(std::env::current_exe().map(|program| program.to_string_lossy().into_owned()).unwrap_or_default()))
    }
    fn Path(&self) -> windows::core::Result<BSTR> { self.FullName() }
    fn Visible(&self) -> windows::core::Result<VARIANT_BOOL> { Ok(VARIANT_FALSE) }
    fn SetVisible(&self, _: VARIANT_BOOL) -> windows::core::Result<()> { unsupported() }
    fn StatusBar(&self) -> windows::core::Result<VARIANT_BOOL> { Ok(VARIANT_FALSE) }
    fn SetStatusBar(&self, _: VARIANT_BOOL) -> windows::core::Result<()> { unsupported() }
    fn StatusText(&self) -> windows::core::Result<BSTR> { Ok(BSTR::new()) }
    fn SetStatusText(&self, _: &BSTR) -> windows::core::Result<()> { unsupported() }
    fn ToolBar(&self) -> windows::core::Result<i32> { Ok(0) }
    fn SetToolBar(&self, _: i32) -> windows::core::Result<()> { unsupported() }
    fn MenuBar(&self) -> windows::core::Result<VARIANT_BOOL> { Ok(VARIANT_FALSE) }
    fn SetMenuBar(&self, _: VARIANT_BOOL) -> windows::core::Result<()> { unsupported() }
    fn FullScreen(&self) -> windows::core::Result<VARIANT_BOOL> { Ok(VARIANT_FALSE) }
    fn SetFullScreen(&self, _: VARIANT_BOOL) -> windows::core::Result<()> { unsupported() }
}

impl IServiceProvider_Impl for ShellWindow_Impl {
    /// Whatever the service, the window is its own browser and view.
    fn QueryService(&self, _: *const GUID, interface: *const GUID, object: *mut *mut core::ffi::c_void) -> windows::core::Result<()> {
        if interface.is_null() || object.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe { self.QueryInterface(interface, object).ok() }
    }
}

impl IOleWindow_Impl for ShellWindow_Impl {
    fn GetWindow(&self) -> windows::core::Result<HWND> { Ok(self.0.window) }
    fn ContextSensitiveHelp(&self, _: BOOL) -> windows::core::Result<()> { unsupported() }
}

impl IShellView_Impl for ShellWindow_Impl {
    fn TranslateAccelerator(&self, _: *const MSG) -> HRESULT { E_NOTIMPL }
    fn EnableModeless(&self, _: BOOL) -> windows::core::Result<()> { Ok(()) }
    fn UIActivate(&self, _: u32) -> windows::core::Result<()> { Ok(()) }
    fn Refresh(&self) -> windows::core::Result<()> { Ok(()) }
    fn CreateViewWindow(&self, _: Ref<IShellView>, _: *const FOLDERSETTINGS, _: Ref<IShellBrowser>, _: *const RECT) -> windows::core::Result<HWND> { unsupported() }
    fn DestroyViewWindow(&self) -> windows::core::Result<()> { Ok(()) }
    fn GetCurrentInfo(&self) -> windows::core::Result<FOLDERSETTINGS> { Ok(FOLDERSETTINGS::default()) }
    fn AddPropertySheetPages(&self, _: u32, _: LPFNSVADDPROPSHEETPAGE, _: LPARAM) -> windows::core::Result<()> { unsupported() }
    fn SaveViewState(&self) -> windows::core::Result<()> { Ok(()) }
    /// Notes the item; the wait ends once no other follows.
    fn SelectItem(&self, item: *const ITEMIDLIST, flags: u32) -> windows::core::Result<()> {
        let selecting = (SVSI_SELECT.0 | SVSI_FOCUSED.0 | SVSI_ENSUREVISIBLE.0 | SVSI_EDIT.0) as u32;
        if item.is_null() || flags & selecting == 0 {
            return Ok(());
        }
        let shown = &self.0;
        let path = unsafe {
            let absolute = IdList(ILCombine(Some(shown.folder.0), Some(item)));
            let name = SHCreateItemFromIDList::<IShellItem>(absolute.0)?.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path = name.to_string();
            CoTaskMemFree(Some(name.0.cast()));
            path
        };
        if let Ok(path) = path {
            let mut items = shown.selection.items.borrow_mut();
            if !items.contains(&path) {
                items.push(path);
            }
            unsafe { SetTimer(Some(shown.selection.clock), TIMER, SETTLE, None) };
        }
        Ok(())
    }
    fn GetItemObject(&self, _: &_SVGIO, _: *const GUID, object: *mut *mut core::ffi::c_void) -> windows::core::Result<()> {
        if !object.is_null() {
            unsafe { *object = std::ptr::null_mut() };
        }
        Err(E_NOINTERFACE.into())
    }
}

/// A window while the Shell lists it.
struct Listed {
    shown: Rc<Shown>,
    windows: IShellWindows,
    cookies: Vec<i32>,
    _object: IWebBrowserApp,
}

impl Drop for Listed {
    fn drop(&mut self) {
        for cookie in &self.cookies {
            unsafe { let _ = self.windows.Revoke(*cookie); }
        }
        unsafe { let _ = DestroyWindow(self.shown.window); }
    }
}

/// A window nobody sees: never shown, and cloaked should a program show it to bring it
/// forward.
fn hidden_window() -> windows::core::Result<HWND> {
    unsafe {
        let window = CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, w!("Static"), w!("Oxyde"), WINDOW_STYLE(0), 0, 0, 0, 0, None, None, None, None)?;
        let cloaked = BOOL::from(true);
        let _ = DwmSetWindowAttribute(window, DWMWA_CLOAK, (&raw const cloaked).cast(), std::mem::size_of::<BOOL>() as u32);
        Ok(window)
    }
}

/// The identifier lists programs know `folder` by: through its known folder, then
/// through the drive when that is another.
fn identifier_lists(folder: &str) -> windows::core::Result<Vec<IdList>> {
    let name = HSTRING::from(folder);
    unsafe {
        let mut item: *mut ITEMIDLIST = std::ptr::null_mut();
        SHParseDisplayName(PCWSTR(name.as_ptr()), None, &mut item, 0, None)?;
        let mut lists = vec![IdList(item)];
        let plain = IdList(ILCreateFromPathW(PCWSTR(name.as_ptr())));
        if !plain.0.is_null() && !ILIsEqual(plain.0, item).as_bool() {
            lists.push(plain);
        }
        Ok(lists)
    }
}

impl Listed {
    /// Has the Shell list `window` as showing `folder`.
    fn new(window: HWND, folder: IdList, selection: Rc<Selection>, windows: &IShellWindows) -> windows::core::Result<Self> {
        unsafe {
            let location = InitVariantFromBuffer(folder.0.cast(), ILGetSize(Some(folder.0)));
            let shown = Rc::new(Shown { window, folder, selection });
            let object: IWebBrowserApp = ShellWindow(shown.clone()).into();
            let mut listed = Self { shown, windows: windows.clone(), cookies: Vec::new(), _object: object.clone() };
            let location = location?;
            // A program waiting for the window of the folder it opened is told one comes
            // from this thread, then that it is there.
            let pending = listed.windows.RegisterPending(GetCurrentThreadId() as i32, &location, &VARIANT::default(), SWC_BROWSER)?;
            listed.cookies.push(pending);
            let cookie = listed.windows.Register(&object.cast::<IDispatch>()?, window.0 as i32, SWC_BROWSER)?;
            // Registering may complete the pending registration under the same cookie.
            if cookie != pending {
                listed.cookies.push(cookie);
            }
            listed.windows.OnNavigate(cookie, &location)?;
            Ok(listed)
        }
    }
}

/// The windows standing for `folder`, one for each of its identifier lists.
struct Standing {
    listed: Vec<Listed>,
    selection: Rc<Selection>,
}

impl Standing {
    fn new(folder: &str) -> windows::core::Result<Self> {
        let lists = identifier_lists(folder)?;
        let windows: IShellWindows = unsafe { CoCreateInstance(&ShellWindows, None, CLSCTX_ALL) }?;
        let clock = hidden_window()?;
        let selection = Rc::new(Selection { items: RefCell::default(), clock });
        let mut listed = Vec::new();
        for (index, list) in lists.into_iter().enumerate() {
            let window = if index == 0 { Ok(clock) } else { hidden_window() };
            match window.and_then(|window| Listed::new(window, list, selection.clone(), &windows)) {
                Ok(window) => listed.push(window),
                // Without its first window the clock would be nobody's to destroy.
                Err(error) if index == 0 => {
                    unsafe { let _ = DestroyWindow(clock); }
                    return Err(error);
                }
                Err(error) => log::warn!("Cannot stand for {folder} under its other identifier list: {error}"),
            }
        }
        Ok(Self { listed, selection })
    }

    /// Answers the Shell until a selection has come whole, or none does.
    fn selection(self) -> Vec<String> {
        let clock = self.selection.clock;
        unsafe {
            // An item may have come already, while the windows were being listed.
            let wait = if self.selection.items.borrow().is_empty() { PATIENCE } else { SETTLE };
            SetTimer(Some(clock), TIMER, wait, None);
            let mut message = MSG::default();
            while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                if message.hwnd == clock && message.message == WM_TIMER {
                    break;
                }
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        drop(self.listed);
        self.selection.items.take()
    }
}

/// Stands for an Explorer window showing `folder` while `open` hands the folder to
/// Oxyde, and for a while after; returns the items a program asked that window to
/// select. `open` runs whatever happens.
pub(super) fn selection(folder: &str, open: impl FnOnce()) -> Vec<String> {
    let apartment = ComApartment::sta().ok();
    let standing = apartment.as_ref().and_then(|_| {
        Standing::new(folder).inspect_err(|error| log::warn!("Cannot stand for the Explorer window of {folder}: {error}")).ok()
    });
    open();
    standing.map(Standing::selection).unwrap_or_default()
}

#[cfg(test)]
#[path = "windows_shell_window/tests/selection.rs"]
mod tests;
