//! The dialog applications get: the system's own, wrapped. Everything an application
//! sets goes on to the system's dialog, which stays ready to be shown; what Oxyde
//! needs to stand for it is noted on the way. `Show` asks Oxyde when the dialog is a
//! plain one, and the system's dialog otherwise: when the application added controls
//! Oxyde cannot show (`controls` says which it can), filters items itself, browses things that are not files, or took from
//! the system's dialog an interface this one does not have.
//!
//! The object is written by hand rather than generated, to answer for interfaces it
//! does not implement with the system dialog's: an application asking for one of them
//! keeps working, on the system's dialog. Its three interfaces are three table
//! pointers at the head of the object; a method finds the object from the pointer it
//! was called on, `N` pointers further.
use crate::ask::{ask, close, top_level, Outcome};
use crate::controls::Controls;
use crate::items;
use crate::request::{text, Answer, Asked};
use core::ffi::c_void;
use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicU32, Ordering};
use windows::core::{IUnknown, IUnknown_Vtbl, Interface, BOOL, GUID, HRESULT, PCWSTR, PWSTR};
use windows::Win32::Foundation::{ERROR_CANCELLED, E_NOINTERFACE, E_NOTIMPL, E_POINTER, HWND, S_FALSE, S_OK};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    IFileDialog, IFileDialog2, IFileDialog2_Vtbl, IFileDialogControlEvents, IFileDialogCustomize, IFileDialogCustomize_Vtbl, IFileDialogEvents, IFileDialog_Vtbl,
    IFileOpenDialog, IFileOpenDialog_Vtbl, IFileSaveDialog, IFileSaveDialog_Vtbl, IModalWindow, IModalWindow_Vtbl, IShellItem,
    CDCONTROLSTATEF, FDAP, FILEOPENDIALOGOPTIONS, FOS_ALLNONSTORAGEITEMS, FOS_ALLOWMULTISELECT, FOS_NODEREFERENCELINKS, FOS_PICKFOLDERS,
};

/// How often a choice the application refuses is asked again before giving up.
const ATTEMPTS: usize = 20;

#[repr(C)]
pub(crate) struct Dialog {
    /// `IFileOpenDialog` or `IFileSaveDialog`, then `IFileDialog2` and
    /// `IFileDialogCustomize`: the order `of` counts in.
    main: *const c_void,
    second: *const IFileDialog2_Vtbl,
    controls: *const IFileDialogCustomize_Vtbl,
    references: AtomicU32,
    /// The system's dialog, and its other faces where it has them.
    inner: IFileDialog,
    inner_open: Option<IFileOpenDialog>,
    inner_save: Option<IFileSaveDialog>,
    inner_second: Option<IFileDialog2>,
    inner_controls: Option<IFileDialogCustomize>,
    /// What makes the dialog one Oxyde does not stand for.
    special: Cell<bool>,
    /// The application's controls Oxyde shows.
    custom: RefCell<Controls>,
    filters: RefCell<Vec<(String, String)>>,
    title: RefCell<Option<String>>,
    accept: RefCell<Option<String>>,
    extension: RefCell<Option<String>>,
    folder: RefCell<Option<IShellItem>>,
    default_folder: RefCell<Option<IShellItem>>,
    save_as: RefCell<Option<IShellItem>>,
    events: RefCell<Vec<(u32, IFileDialogEvents)>>,
    /// What the user chose in Oxyde, when Oxyde was the dialog last shown.
    answer: RefCell<Option<Answer>>,
    /// The launcher's process while Oxyde is shown.
    showing: Cell<isize>,
    closed: Cell<Option<HRESULT>>,
}

fn cancelled() -> HRESULT {
    HRESULT::from_win32(ERROR_CANCELLED.0)
}

unsafe fn text_of(text: PCWSTR) -> Option<String> {
    if text.is_null() { None } else { text.to_string().ok() }
}

unsafe fn item_of(item: *mut c_void) -> Option<IShellItem> {
    IShellItem::from_raw_borrowed(&item).cloned()
}

impl Dialog {
    /// Wraps the system's dialog, `inner`, into what `iid` names.
    pub(crate) unsafe fn create(inner: IFileDialog, iid: *const GUID, out: *mut *mut c_void) -> HRESULT {
        let inner_save = inner.cast::<IFileSaveDialog>().ok();
        let dialog = Box::into_raw(Box::new(Dialog {
            main: if inner_save.is_some() { (&raw const SAVE).cast() } else { (&raw const OPEN).cast() },
            second: &SECOND,
            controls: &CONTROLS,
            references: AtomicU32::new(1),
            inner_open: inner.cast().ok(),
            inner_save,
            inner_second: inner.cast().ok(),
            inner_controls: inner.cast().ok(),
            inner,
            special: Cell::new(false),
            custom: RefCell::default(),
            filters: RefCell::default(),
            title: RefCell::default(),
            accept: RefCell::default(),
            extension: RefCell::default(),
            folder: RefCell::default(),
            default_folder: RefCell::default(),
            save_as: RefCell::default(),
            events: RefCell::default(),
            answer: RefCell::default(),
            showing: Cell::new(0),
            closed: Cell::new(None),
        }));
        let found = query::<0>(dialog.cast(), iid, out);
        release::<0>(dialog.cast());
        found
    }

    /// The object a method was called on, through its `N`th interface.
    unsafe fn of<'a, const N: usize>(this: *mut c_void) -> &'a Dialog {
        &*this.cast::<*const c_void>().sub(N).cast::<Dialog>()
    }

    fn face(&self, position: usize) -> *mut c_void {
        unsafe { (&raw const self.main).add(position).cast_mut().cast() }
    }

    fn saving(&self) -> bool {
        self.inner_save.is_some()
    }

    fn first(&self) -> Option<String> {
        self.answer.borrow().as_ref().and_then(|answer| answer.paths.first().cloned())
    }

    /// What the application asked for, as Oxyde reads it.
    unsafe fn asked(&self, options: FILEOPENDIALOGOPTIONS, owner: HWND) -> Asked {
        let save_as = self.save_as.borrow().clone();
        let typed = self.inner.GetFileName().ok().and_then(|name| {
            let read = name.to_string().ok();
            windows::Win32::System::Com::CoTaskMemFree(Some(name.0.cast()));
            read
        }).filter(|name| !name.is_empty());
        let folder = self.folder.borrow().clone().as_ref().and_then(items::path)
            .or_else(|| save_as.as_ref().and_then(|item| item.GetParent().ok()).as_ref().and_then(items::path))
            .or_else(|| self.inner.GetFolder().ok().as_ref().and_then(items::path))
            .or_else(|| self.default_folder.borrow().clone().as_ref().and_then(items::path));
        Asked {
            saving: self.saving(),
            folders: options.contains(FOS_PICKFOLDERS),
            multiple: options.contains(FOS_ALLOWMULTISELECT),
            title: self.title.borrow().clone(),
            accept: self.accept.borrow().clone(),
            name: typed.or_else(|| save_as.as_ref().and_then(items::name)),
            folder,
            extension: self.extension.borrow().clone(),
            owner: owner.0 as isize,
            filters: self.filters.borrow().clone(),
            // Until the application chooses one, the system's dialog counts none: the first is shown.
            filter: self.inner.GetFileTypeIndex().unwrap_or(1).max(1),
            choices: self.inner_controls.as_ref().map(|inner| self.custom.borrow().asked(inner)).unwrap_or_default(),
        }
    }

    /// Sets Oxyde's answers on the application's controls, and tells its listeners of
    /// each one changed, as the system's dialog does when the user changes it.
    unsafe fn set_choices(&self, choices: &[(u32, String)]) {
        let Some(inner) = &self.inner_controls else { return };
        let listeners: Vec<IFileDialogControlEvents> = self.events.borrow().iter().filter_map(|(_, listener)| listener.cast().ok()).collect();
        for (id, value) in choices {
            if self.custom.borrow().is_check(*id) {
                let checked = value == "true";
                if inner.GetCheckButtonState(*id).is_ok_and(|known| known.as_bool() == checked) || inner.SetCheckButtonState(*id, checked).is_err() {
                    continue;
                }
                for listener in &listeners {
                    let _ = (Interface::vtable(listener).OnCheckButtonToggled)(Interface::as_raw(listener), self.face(2), *id, checked.into());
                }
            } else if let Ok(item) = value.parse::<u32>() {
                if inner.GetSelectedControlItem(*id).ok() == Some(item) || inner.SetSelectedControlItem(*id, item).is_err() {
                    continue;
                }
                for listener in &listeners {
                    let _ = (Interface::vtable(listener).OnItemSelected)(Interface::as_raw(listener), self.face(2), *id, item);
                }
            }
        }
    }

    /// Whether every listener lets the dialog close on the choice just made.
    unsafe fn accepted(&self) -> bool {
        let listeners: Vec<IFileDialogEvents> = self.events.borrow().iter().map(|(_, listener)| listener.clone()).collect();
        listeners.iter().all(|listener| (Interface::vtable(listener).OnFileOk)(Interface::as_raw(listener), self.face(0)) != S_FALSE)
    }

    /// Shows Oxyde; None when the system's dialog is to be shown instead.
    unsafe fn show(&self, owner: HWND) -> Option<HRESULT> {
        if self.special.get() || crate::within_oxyde() || crate::withdrawn() {
            return None;
        }
        let options = self.inner.GetOptions().ok()?;
        if options.contains(FOS_ALLNONSTORAGEITEMS) {
            return None;
        }
        let owner = top_level(owner);
        let request = text(&self.asked(options, owner));
        crate::log(|| format!("asking Oxyde:\n{request}"));
        for _ in 0..ATTEMPTS {
            self.closed.set(None);
            let mut answer = match ask(&request, owner, &self.showing) {
                Outcome::Unavailable => return None,
                Outcome::Cancelled => return Some(cancelled()),
                Outcome::Closed => return Some(self.closed.take().unwrap_or_else(cancelled)),
                Outcome::Chosen(answer) => answer,
            };
            crate::log(|| format!("Oxyde answered {answer:?}"));
            if !self.saving() && !options.contains(FOS_NODEREFERENCELINKS) && !options.contains(FOS_PICKFOLDERS) {
                for path in &mut answer.paths {
                    if let Some(target) = items::shortcut_target(path) {
                        *path = target;
                    }
                }
            }
            if let Some(filter) = answer.filter {
                let _ = self.inner.SetFileTypeIndex(filter);
            }
            self.set_choices(&answer.choices);
            self.answer.replace(Some(answer));
            if self.accepted() {
                return Some(S_OK);
            }
            // The application refuses the choice and has said why: the user chooses again.
            self.answer.replace(None);
        }
        Some(cancelled())
    }
}

/// The interfaces the object has itself, and where each one's pointer is.
fn own_face(iid: &GUID, saving: bool, controls: bool) -> Option<usize> {
    if *iid == IUnknown::IID || *iid == IModalWindow::IID || *iid == IFileDialog::IID {
        Some(0)
    } else if *iid == IFileOpenDialog::IID {
        (!saving).then_some(0)
    } else if *iid == IFileSaveDialog::IID {
        saving.then_some(0)
    } else if *iid == IFileDialog2::IID {
        Some(1)
    } else if *iid == IFileDialogCustomize::IID {
        controls.then_some(2)
    } else {
        None
    }
}

unsafe extern "system" fn query<const N: usize>(this: *mut c_void, iid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    let dialog = Dialog::of::<N>(this);
    if iid.is_null() || out.is_null() {
        return E_POINTER;
    }
    if let Some(position) = own_face(&*iid, dialog.saving(), dialog.inner_controls.is_some()) {
        dialog.references.fetch_add(1, Ordering::Relaxed);
        *out = dialog.face(position);
        return S_OK;
    }
    // The system's dialog answers for the rest, and is then the one to show: what the
    // application does with that interface is unknown here.
    let found = dialog.inner.query(iid, out);
    if found == S_OK {
        crate::log(|| format!("the system's dialog answers for {:?}", *iid));
        dialog.special.set(true);
    } else {
        *out = std::ptr::null_mut();
    }
    if found == S_OK { S_OK } else { E_NOINTERFACE }
}

unsafe extern "system" fn add_reference<const N: usize>(this: *mut c_void) -> u32 {
    Dialog::of::<N>(this).references.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn release<const N: usize>(this: *mut c_void) -> u32 {
    let dialog = Dialog::of::<N>(this);
    let left = dialog.references.fetch_sub(1, Ordering::AcqRel) - 1;
    if left == 0 {
        drop(Box::from_raw((dialog as *const Dialog).cast_mut()));
    }
    left
}

unsafe extern "system" fn show<const N: usize>(this: *mut c_void, owner: HWND) -> HRESULT {
    let dialog = Dialog::of::<N>(this);
    dialog.answer.replace(None);
    // A fault here must not take the application down: it gets the system's dialog.
    let shown = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dialog.show(owner)));
    match shown {
        Ok(Some(result)) => result,
        _ => {
            dialog.answer.replace(None);
            (Interface::vtable(&dialog.inner).base__.Show)(Interface::as_raw(&dialog.inner), owner)
        }
    }
}

/// A method the system's dialog answers.
macro_rules! forward {
    ($name:ident => $field:ident($($argument:ident: $kind:ty),*)) => {
        unsafe extern "system" fn $name<const N: usize>(this: *mut c_void $(, $argument: $kind)*) -> HRESULT {
            let inner = &Dialog::of::<N>(this).inner;
            (Interface::vtable(inner).$field)(Interface::as_raw(inner) $(, $argument)*)
        }
    };
}

/// A method the system's dialog answers once `$noted` is kept from its text.
macro_rules! noted {
    ($name:ident => $field:ident, $noted:ident) => {
        unsafe extern "system" fn $name<const N: usize>(this: *mut c_void, text: PCWSTR) -> HRESULT {
            let dialog = Dialog::of::<N>(this);
            dialog.$noted.replace(text_of(text));
            (Interface::vtable(&dialog.inner).$field)(Interface::as_raw(&dialog.inner), text)
        }
    };
}

/// A method the system's dialog answers once `$noted` is kept from its item.
macro_rules! placed {
    ($name:ident => $field:ident, $noted:ident) => {
        unsafe extern "system" fn $name<const N: usize>(this: *mut c_void, item: *mut c_void) -> HRESULT {
            let dialog = Dialog::of::<N>(this);
            dialog.$noted.replace(item_of(item));
            (Interface::vtable(&dialog.inner).$field)(Interface::as_raw(&dialog.inner), item)
        }
    };
}

/// A method answered from Oxyde's choice when there is one, by the system's dialog
/// otherwise; `$chosen` makes the item from the first path chosen.
macro_rules! chosen_item {
    ($name:ident => $field:ident, $chosen:expr) => {
        unsafe extern "system" fn $name<const N: usize>(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
            let dialog = Dialog::of::<N>(this);
            if out.is_null() {
                return E_POINTER;
            }
            let Some(path) = dialog.first() else {
                return (Interface::vtable(&dialog.inner).$field)(Interface::as_raw(&dialog.inner), out);
            };
            let chosen: fn(&str) -> windows::core::Result<IShellItem> = $chosen;
            match chosen(&path) {
                Ok(item) => {
                    *out = item.into_raw();
                    S_OK
                }
                Err(error) => {
                    *out = std::ptr::null_mut();
                    error.code()
                }
            }
        }
    };
}

unsafe extern "system" fn set_file_types<const N: usize>(this: *mut c_void, count: u32, types: *const COMDLG_FILTERSPEC) -> HRESULT {
    let dialog = Dialog::of::<N>(this);
    if !types.is_null() {
        let types = std::slice::from_raw_parts(types, count as usize);
        dialog.filters.replace(types.iter().map(|filter| {
            (text_of(filter.pszName).unwrap_or_default(), text_of(filter.pszSpec).unwrap_or_default())
        }).collect());
    }
    (Interface::vtable(&dialog.inner).SetFileTypes)(Interface::as_raw(&dialog.inner), count, types)
}

unsafe extern "system" fn advise<const N: usize>(this: *mut c_void, listener: *mut c_void, cookie: *mut u32) -> HRESULT {
    let dialog = Dialog::of::<N>(this);
    let advised = (Interface::vtable(&dialog.inner).Advise)(Interface::as_raw(&dialog.inner), listener, cookie);
    if advised == S_OK && !cookie.is_null() {
        if let Some(listener) = IFileDialogEvents::from_raw_borrowed(&listener) {
            dialog.events.borrow_mut().push((*cookie, listener.clone()));
        }
    }
    advised
}

unsafe extern "system" fn unadvise<const N: usize>(this: *mut c_void, cookie: u32) -> HRESULT {
    let dialog = Dialog::of::<N>(this);
    // Released once the list is no longer borrowed: a listener may call back.
    let gone: Vec<(u32, IFileDialogEvents)> = {
        let mut events = dialog.events.borrow_mut();
        let (gone, kept) = events.drain(..).partition(|(known, _)| *known == cookie);
        *events = kept;
        gone
    };
    drop(gone);
    (Interface::vtable(&dialog.inner).Unadvise)(Interface::as_raw(&dialog.inner), cookie)
}

unsafe extern "system" fn get_file_name<const N: usize>(this: *mut c_void, out: *mut PWSTR) -> HRESULT {
    let dialog = Dialog::of::<N>(this);
    if out.is_null() {
        return E_POINTER;
    }
    let Some(path) = dialog.first() else {
        return (Interface::vtable(&dialog.inner).GetFileName)(Interface::as_raw(&dialog.inner), out);
    };
    let name = std::path::Path::new(&path).file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or(path);
    *out = items::task_text(&name);
    S_OK
}

unsafe extern "system" fn get_file_type_index<const N: usize>(this: *mut c_void, out: *mut u32) -> HRESULT {
    let dialog = Dialog::of::<N>(this);
    let chosen = dialog.answer.borrow().as_ref().and_then(|answer| answer.filter);
    match chosen {
        Some(filter) if !out.is_null() => {
            *out = filter;
            S_OK
        }
        _ => (Interface::vtable(&dialog.inner).GetFileTypeIndex)(Interface::as_raw(&dialog.inner), out),
    }
}

unsafe extern "system" fn close_dialog<const N: usize>(this: *mut c_void, result: HRESULT) -> HRESULT {
    let dialog = Dialog::of::<N>(this);
    dialog.closed.set(Some(result));
    if close(&dialog.showing) {
        return S_OK;
    }
    (Interface::vtable(&dialog.inner).Close)(Interface::as_raw(&dialog.inner), result)
}

unsafe extern "system" fn set_filter<const N: usize>(this: *mut c_void, filter: *mut c_void) -> HRESULT {
    let dialog = Dialog::of::<N>(this);
    // The application decides item by item what is listed: only its dialog can ask it.
    if !filter.is_null() {
        dialog.special.set(true);
    }
    (Interface::vtable(&dialog.inner).SetFilter)(Interface::as_raw(&dialog.inner), filter)
}

/// The items chosen in Oxyde, or the system dialog's.
unsafe fn chosen_items(dialog: &Dialog, out: *mut *mut c_void, selected: bool) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    let chosen = dialog.answer.borrow().as_ref().map(|answer| answer.paths.clone());
    match (chosen, &dialog.inner_open) {
        (Some(paths), _) => match items::array(&paths) {
            Ok(items) => {
                *out = items.into_raw();
                S_OK
            }
            Err(error) => {
                *out = std::ptr::null_mut();
                error.code()
            }
        },
        (None, Some(inner)) if selected => (Interface::vtable(inner).GetSelectedItems)(Interface::as_raw(inner), out),
        (None, Some(inner)) => (Interface::vtable(inner).GetResults)(Interface::as_raw(inner), out),
        (None, None) => E_NOTIMPL,
    }
}

unsafe extern "system" fn get_results(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    chosen_items(Dialog::of::<0>(this), out, false)
}

unsafe extern "system" fn get_selected_items(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    chosen_items(Dialog::of::<0>(this), out, true)
}

forward!(set_file_type_index => SetFileTypeIndex(index: u32));
forward!(set_options => SetOptions(options: FILEOPENDIALOGOPTIONS));
forward!(get_options => GetOptions(options: *mut FILEOPENDIALOGOPTIONS));
forward!(set_file_name => SetFileName(name: PCWSTR));
forward!(set_file_name_label => SetFileNameLabel(label: PCWSTR));
forward!(add_place => AddPlace(item: *mut c_void, place: FDAP));
forward!(set_client_guid => SetClientGuid(guid: *const GUID));
forward!(clear_client_data => ClearClientData());
noted!(set_title => SetTitle, title);
noted!(set_ok_button_label => SetOkButtonLabel, accept);
noted!(set_default_extension => SetDefaultExtension, extension);
placed!(set_folder => SetFolder, folder);
placed!(set_default_folder => SetDefaultFolder, default_folder);
chosen_item!(get_result => GetResult, |path| items::item(path));
chosen_item!(get_current_selection => GetCurrentSelection, |path| items::item(path));
chosen_item!(get_folder => GetFolder, |path| unsafe { items::item(path)?.GetParent() });

const fn file_dialog<const N: usize>() -> IFileDialog_Vtbl {
    IFileDialog_Vtbl {
        base__: IModalWindow_Vtbl {
            base__: IUnknown_Vtbl { QueryInterface: query::<N>, AddRef: add_reference::<N>, Release: release::<N> },
            Show: show::<N>,
        },
        SetFileTypes: set_file_types::<N>,
        SetFileTypeIndex: set_file_type_index::<N>,
        GetFileTypeIndex: get_file_type_index::<N>,
        Advise: advise::<N>,
        Unadvise: unadvise::<N>,
        SetOptions: set_options::<N>,
        GetOptions: get_options::<N>,
        SetDefaultFolder: set_default_folder::<N>,
        SetFolder: set_folder::<N>,
        GetFolder: get_folder::<N>,
        GetCurrentSelection: get_current_selection::<N>,
        SetFileName: set_file_name::<N>,
        GetFileName: get_file_name::<N>,
        SetTitle: set_title::<N>,
        SetOkButtonLabel: set_ok_button_label::<N>,
        SetFileNameLabel: set_file_name_label::<N>,
        GetResult: get_result::<N>,
        AddPlace: add_place::<N>,
        SetDefaultExtension: set_default_extension::<N>,
        Close: close_dialog::<N>,
        SetClientGuid: set_client_guid::<N>,
        ClearClientData: clear_client_data::<N>,
        SetFilter: set_filter::<N>,
    }
}

static OPEN: IFileOpenDialog_Vtbl = IFileOpenDialog_Vtbl { base__: file_dialog::<0>(), GetResults: get_results, GetSelectedItems: get_selected_items };

/// A method of the system's Save dialog.
macro_rules! save {
    ($name:ident => $field:ident($($argument:ident: $kind:ty),*)) => {
        unsafe extern "system" fn $name(this: *mut c_void $(, $argument: $kind)*) -> HRESULT {
            let Some(inner) = &Dialog::of::<0>(this).inner_save else { return E_NOTIMPL };
            (Interface::vtable(inner).$field)(Interface::as_raw(inner) $(, $argument)*)
        }
    };
}

unsafe extern "system" fn set_save_as_item(this: *mut c_void, item: *mut c_void) -> HRESULT {
    let dialog = Dialog::of::<0>(this);
    dialog.save_as.replace(item_of(item));
    let Some(inner) = &dialog.inner_save else { return E_NOTIMPL };
    (Interface::vtable(inner).SetSaveAsItem)(Interface::as_raw(inner), item)
}

save!(set_properties => SetProperties(store: *mut c_void));
save!(set_collected_properties => SetCollectedProperties(list: *mut c_void, append: BOOL));
save!(get_properties => GetProperties(store: *mut *mut c_void));
save!(apply_properties => ApplyProperties(item: *mut c_void, store: *mut c_void, owner: HWND, sink: *mut c_void));

static SAVE: IFileSaveDialog_Vtbl = IFileSaveDialog_Vtbl {
    base__: file_dialog::<0>(),
    SetSaveAsItem: set_save_as_item,
    SetProperties: set_properties,
    SetCollectedProperties: set_collected_properties,
    GetProperties: get_properties,
    ApplyProperties: apply_properties,
};

unsafe extern "system" fn set_cancel_button_label(this: *mut c_void, label: PCWSTR) -> HRESULT {
    let Some(inner) = &Dialog::of::<1>(this).inner_second else { return E_NOTIMPL };
    (Interface::vtable(inner).SetCancelButtonLabel)(Interface::as_raw(inner), label)
}

unsafe extern "system" fn set_navigation_root(this: *mut c_void, root: *mut c_void) -> HRESULT {
    let dialog = Dialog::of::<1>(this);
    // Browsing is confined to a part of the Shell, which Oxyde does not do.
    dialog.special.set(true);
    let Some(inner) = &dialog.inner_second else { return E_NOTIMPL };
    (Interface::vtable(inner).SetNavigationRoot)(Interface::as_raw(inner), root)
}

static SECOND: IFileDialog2_Vtbl = IFileDialog2_Vtbl { base__: file_dialog::<1>(), SetCancelButtonLabel: set_cancel_button_label, SetNavigationRoot: set_navigation_root };

/// A method of the controls an application adds to its dialog, answered by the
/// system's dialog, which keeps their state; `$noted` notes what Oxyde shows of it.
macro_rules! control {
    ($name:ident => $field:ident($($argument:ident: $kind:ty),*)) => {
        control!($name => $field($($argument: $kind),*), |_dialog $(, $argument)*| { $(let _ = $argument;)* });
    };
    ($name:ident => $field:ident($($argument:ident: $kind:ty),*), $noted:expr) => {
        unsafe extern "system" fn $name(this: *mut c_void $(, $argument: $kind)*) -> HRESULT {
            let dialog = Dialog::of::<2>(this);
            let Some(inner) = &dialog.inner_controls else { return E_NOTIMPL };
            let done = (Interface::vtable(inner).$field)(Interface::as_raw(inner) $(, $argument)*);
            if done == S_OK {
                let noted: unsafe fn(&Dialog $(, $kind)*) = $noted;
                noted(dialog $(, $argument)*);
            }
            done
        }
    };
}

/// A control Oxyde has nothing like: the dialog is the system's to show.
fn unlike(dialog: &Dialog) {
    dialog.special.set(true);
}

unsafe fn text_or_empty(text: PCWSTR) -> String {
    text_of(text).unwrap_or_default()
}

control!(enable_open_drop_down => EnableOpenDropDown(control: u32), |dialog, _| unlike(dialog));
control!(add_menu => AddMenu(control: u32, label: PCWSTR), |dialog, _, _| unlike(dialog));
control!(add_push_button => AddPushButton(control: u32, label: PCWSTR), |dialog, _, _| unlike(dialog));
control!(add_edit_box => AddEditBox(control: u32, text: PCWSTR), |dialog, _, _| unlike(dialog));
control!(add_combo_box => AddComboBox(control: u32), |dialog, id| dialog.custom.borrow_mut().add_choice(id));
control!(add_radio_button_list => AddRadioButtonList(control: u32), |dialog, id| dialog.custom.borrow_mut().add_choice(id));
control!(add_check_button => AddCheckButton(control: u32, label: PCWSTR, checked: BOOL), |dialog, id, text, _| dialog.custom.borrow_mut().add_check(id, &text_or_empty(text)));
control!(add_separator => AddSeparator(control: u32));
control!(add_text => AddText(control: u32, text: PCWSTR), |dialog, id, text| dialog.custom.borrow_mut().add_text(id, &text_or_empty(text)));
control!(set_control_label => SetControlLabel(control: u32, label: PCWSTR), |dialog, id, text| dialog.custom.borrow_mut().set_label(id, &text_or_empty(text)));
control!(get_control_state => GetControlState(control: u32, state: *mut CDCONTROLSTATEF));
control!(set_control_state => SetControlState(control: u32, state: CDCONTROLSTATEF));
control!(get_edit_box_text => GetEditBoxText(control: u32, text: *mut *mut u16));
control!(set_edit_box_text => SetEditBoxText(control: u32, text: PCWSTR));
control!(get_check_button_state => GetCheckButtonState(control: u32, checked: *mut BOOL));
control!(set_check_button_state => SetCheckButtonState(control: u32, checked: BOOL));
control!(add_control_item => AddControlItem(control: u32, item: u32, label: PCWSTR), |dialog, id, item, text| dialog.custom.borrow_mut().add_item(id, item, &text_or_empty(text)));
control!(remove_control_item => RemoveControlItem(control: u32, item: u32), |dialog, id, item| dialog.custom.borrow_mut().remove_item(id, item));
control!(remove_all_control_items => RemoveAllControlItems(control: u32), |dialog, id| dialog.custom.borrow_mut().remove_items(id));
control!(get_control_item_state => GetControlItemState(control: u32, item: u32, state: *mut CDCONTROLSTATEF));
control!(set_control_item_state => SetControlItemState(control: u32, item: u32, state: CDCONTROLSTATEF));
control!(get_selected_control_item => GetSelectedControlItem(control: u32, item: *mut u32));
control!(set_selected_control_item => SetSelectedControlItem(control: u32, item: u32));
control!(start_visual_group => StartVisualGroup(control: u32, label: PCWSTR), |dialog, id, text| dialog.custom.borrow_mut().start_group(id, &text_or_empty(text)));
control!(end_visual_group => EndVisualGroup(), |dialog| dialog.custom.borrow_mut().end_group());
control!(make_prominent => MakeProminent(control: u32));
control!(set_control_item_text => SetControlItemText(control: u32, item: u32, label: PCWSTR), |dialog, id, item, text| dialog.custom.borrow_mut().set_item_text(id, item, &text_or_empty(text)));

static CONTROLS: IFileDialogCustomize_Vtbl = IFileDialogCustomize_Vtbl {
    base__: IUnknown_Vtbl { QueryInterface: query::<2>, AddRef: add_reference::<2>, Release: release::<2> },
    EnableOpenDropDown: enable_open_drop_down,
    AddMenu: add_menu,
    AddPushButton: add_push_button,
    AddComboBox: add_combo_box,
    AddRadioButtonList: add_radio_button_list,
    AddCheckButton: add_check_button,
    AddEditBox: add_edit_box,
    AddSeparator: add_separator,
    AddText: add_text,
    SetControlLabel: set_control_label,
    GetControlState: get_control_state,
    SetControlState: set_control_state,
    GetEditBoxText: get_edit_box_text,
    SetEditBoxText: set_edit_box_text,
    GetCheckButtonState: get_check_button_state,
    SetCheckButtonState: set_check_button_state,
    AddControlItem: add_control_item,
    RemoveControlItem: remove_control_item,
    RemoveAllControlItems: remove_all_control_items,
    GetControlItemState: get_control_item_state,
    SetControlItemState: set_control_item_state,
    GetSelectedControlItem: get_selected_control_item,
    SetSelectedControlItem: set_selected_control_item,
    StartVisualGroup: start_visual_group,
    EndVisualGroup: end_visual_group,
    MakeProminent: make_prominent,
    SetControlItemText: set_control_item_text,
};
