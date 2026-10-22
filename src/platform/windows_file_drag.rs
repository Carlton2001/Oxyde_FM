//! OLE sessions run from a posted message, outside any GPUI App/Window borrow.
mod data;
mod target;
use super::{FileDragFeedback, FileDropEffect};
use raw_window_handle::RawWindowHandle;
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap, HashSet},
    rc::Rc,
};
pub(super) use target::install_target;
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        System::{Com::IDataObject, Ole::*, SystemServices::*},
        UI::{
            Input::KeyboardAndMouse::{GetAsyncKeyState, GetCapture, ReleaseCapture, VK_LBUTTON},
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    },
};

const SUBCLASS: usize = 0x4f585947;
const START: u32 = WM_APP + 0x361;
const PLACE: u32 = START + 1;
const DISMISS: u32 = PLACE + 1;
type Feedback = Rc<RefCell<Box<dyn FnMut(FileDragFeedback)>>>;
struct Pending {
    paths: Vec<String>,
    feedback: Feedback,
}
#[derive(Clone, Copy)]
struct Placement {
    size: [f32; 2],
    scale: f32,
    native: bool,
}
thread_local! {
    static PENDING: RefCell<HashMap<isize, Pending>> = RefCell::new(HashMap::new());
    static PLACEMENTS: RefCell<HashMap<isize, Placement>> = RefCell::new(HashMap::new());
    static DISMISSED: RefCell<HashSet<isize>> = RefCell::new(HashSet::new());
}

pub(super) fn hwnd(raw: RawWindowHandle) -> std::result::Result<HWND, String> {
    match raw {
        RawWindowHandle::Win32(raw) => Ok(HWND(raw.hwnd.get() as *mut _)),
        _ => Err("File dragging requires a Win32 window".into()),
    }
}

pub(super) fn effect(effect: DROPEFFECT) -> FileDropEffect {
    if effect.0 & DROPEFFECT_MOVE.0 != 0 {
        FileDropEffect::Move
    } else if effect.0 & DROPEFFECT_COPY.0 != 0 {
        FileDropEffect::Copy
    } else {
        FileDropEffect::None
    }
}

pub(super) fn pointer_outside(raw: RawWindowHandle) -> bool {
    let Ok(owner) = hwnd(raw) else {
        return false;
    };
    let mut point = POINT::default();
    if unsafe { GetCursorPos(&mut point) }.is_err() {
        return false;
    }
    let target = unsafe { WindowFromPoint(point) };
    unsafe { GetAncestor(target, GA_ROOTOWNER) != GetAncestor(owner, GA_ROOTOWNER) }
}

pub(super) fn queue(
    raw: RawWindowHandle,
    paths: Vec<String>,
    callback: Box<dyn FnMut(FileDragFeedback)>,
) -> std::result::Result<(), String> {
    if paths.is_empty()
        || paths
            .iter()
            .any(|path| path.contains('\0') || !std::path::Path::new(path).is_absolute())
    {
        return Err("Invalid file drag paths".into());
    }
    let window = hwnd(raw)?;
    if !unsafe { SetWindowSubclass(window, Some(subclass), SUBCLASS, 0) }.as_bool() {
        return Err("Cannot install native drag session".into());
    }
    let inserted = PENDING.with(|pending| {
        let mut pending = pending.borrow_mut();
        if pending.contains_key(&(window.0 as isize)) {
            return false;
        }
        pending.insert(
            window.0 as isize,
            Pending {
                paths,
                feedback: Rc::new(RefCell::new(callback)),
            },
        );
        true
    });
    if !inserted {
        return Err("A native file drag is already pending".into());
    }
    if let Err(error) = unsafe { PostMessageW(Some(window), START, WPARAM(0), LPARAM(0)) } {
        PENDING.with(|pending| pending.borrow_mut().remove(&(window.0 as isize)));
        return Err(error.to_string());
    }
    Ok(())
}

pub(super) fn place_ghost(
    raw: RawWindowHandle,
    size: [f32; 2],
    scale: f32,
    native: bool,
) -> std::result::Result<(), String> {
    let window = hwnd(raw)?;
    if DISMISSED.with(|dismissed| dismissed.borrow().contains(&(window.0 as isize))) {
        return Ok(());
    }
    if !unsafe { SetWindowSubclass(window, Some(subclass), SUBCLASS, 0) }.as_bool() {
        return Err("Cannot place native drag ghost".into());
    }
    let pending = PLACEMENTS.with(|placements| {
        placements
            .borrow_mut()
            .insert(
                window.0 as isize,
                Placement {
                    size,
                    scale,
                    native,
                },
            )
            .is_some()
    });
    if pending {
        return Ok(());
    }
    if let Err(error) = unsafe { PostMessageW(Some(window), PLACE, WPARAM(0), LPARAM(0)) } {
        PLACEMENTS.with(|placements| placements.borrow_mut().remove(&(window.0 as isize)));
        return Err(error.to_string());
    }
    Ok(())
}

pub(super) fn dismiss_ghost(raw: RawWindowHandle) -> std::result::Result<(), String> {
    let window = hwnd(raw)?;
    if !unsafe { SetWindowSubclass(window, Some(subclass), SUBCLASS, 0) }.as_bool() {
        return Err("Cannot dismiss native drag ghost".into());
    }
    // Invalidate synchronously, but perform native hiding outside GPUI's borrow.
    // Late measurements and already-posted PLACE messages must never show it again.
    DISMISSED.with(|dismissed| dismissed.borrow_mut().insert(window.0 as isize));
    PLACEMENTS.with(|placements| placements.borrow_mut().remove(&(window.0 as isize)));
    unsafe { PostMessageW(Some(window), DISMISS, WPARAM(0), LPARAM(0)) }
        .map_err(|error| error.to_string())
}

fn flush_dismissals() {
    let windows =
        DISMISSED.with(|dismissed| dismissed.borrow().iter().copied().collect::<Vec<_>>());
    // The feedback callback has released GPUI. Hide now, before entering a
    // foreign Drop which may block without pumping our posted window messages.
    for window in windows {
        let _ = unsafe {
            SendMessageW(
                HWND(window as *mut _),
                DISMISS,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
        };
    }
}

unsafe extern "system" fn subclass(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    _: usize,
) -> LRESULT {
    if message == START {
        let pending = PENDING.with(|pending| pending.borrow_mut().remove(&(window.0 as isize)));
        if let Some(pending) = pending {
            run(window, pending);
        }
        return LRESULT(0);
    }
    if message == PLACE {
        let placement =
            PLACEMENTS.with(|placements| placements.borrow_mut().remove(&(window.0 as isize)));
        if let Some(placement) = placement {
            let mut point = POINT::default();
            if unsafe { GetCursorPos(&mut point) }.is_ok() {
                // Use the cursor monitor's DPI once GPUI observes WM_DPICHANGED.
                let scale = placement.scale.max(0.1);
                let z = if placement.native {
                    HWND_TOPMOST
                } else {
                    HWND_NOTOPMOST
                };
                let width = (placement.size[0] * scale).ceil() as i32;
                let height = (placement.size[1] * scale).ceil() as i32;
                let mut x = point.x + (12.0 * scale) as i32;
                let mut y = point.y + (4.0 * scale) as i32;
                use windows::Win32::Graphics::Gdi::{
                    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
                };
                let mut info = MONITORINFO {
                    cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                if unsafe {
                    GetMonitorInfoW(MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST), &mut info)
                }
                .as_bool()
                {
                    if x + width > info.rcWork.right {
                        x = point.x - width - (12.0 * scale) as i32;
                    }
                    if y + height > info.rcWork.bottom {
                        y = point.y - height - (4.0 * scale) as i32;
                    }
                    x = x.clamp(
                        info.rcWork.left,
                        (info.rcWork.right - width).max(info.rcWork.left),
                    );
                    y = y.clamp(
                        info.rcWork.top,
                        (info.rcWork.bottom - height).max(info.rcWork.top),
                    );
                }
                let _ = unsafe {
                    SetWindowPos(
                        window,
                        Some(z),
                        x,
                        y,
                        width,
                        height,
                        SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    )
                };
            }
        }
        return LRESULT(0);
    }
    if message == DISMISS {
        let _ = unsafe {
            SetWindowPos(
                window,
                Some(HWND_NOTOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_HIDEWINDOW,
            )
        };
        return LRESULT(0);
    }
    if message == WM_NCDESTROY {
        PENDING.with(|pending| pending.borrow_mut().remove(&(window.0 as isize)));
        PLACEMENTS.with(|placements| placements.borrow_mut().remove(&(window.0 as isize)));
        DISMISSED.with(|dismissed| dismissed.borrow_mut().remove(&(window.0 as isize)));
        let _ = unsafe { RemoveWindowSubclass(window, Some(subclass), id) };
    }
    unsafe { DefSubclassProc(window, message, wparam, lparam) }
}

#[implement(IDropSource)]
struct Source {
    feedback: Feedback,
    owner: HWND,
    preview_dismissed: Cell<bool>,
}
#[allow(non_snake_case)]
impl IDropSource_Impl for Source_Impl {
    fn QueryContinueDrag(&self, escape: BOOL, keys: MODIFIERKEYS_FLAGS) -> HRESULT {
        let result = if escape.as_bool()
            || keys.0 & MK_RBUTTON.0 != 0
            || !unsafe { IsWindow(Some(self.owner)) }.as_bool()
        {
            DRAGDROP_S_CANCEL
        } else if keys.0 & MK_LBUTTON.0 == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        };
        if result != S_OK && !self.preview_dismissed.replace(true) {
            (self.feedback.borrow_mut())(FileDragFeedback {
                effect: FileDropEffect::None,
                dismiss_preview: true,
                finished: false,
                cancelled: result == DRAGDROP_S_CANCEL,
                delete_sources: false,
            });
            flush_dismissals();
        }
        result
    }
    fn GiveFeedback(&self, current: DROPEFFECT) -> HRESULT {
        (self.feedback.borrow_mut())(FileDragFeedback {
            effect: effect(current),
            dismiss_preview: self.preview_dismissed.get(),
            finished: false,
            cancelled: false,
            delete_sources: false,
        });
        // Preserve the existing pointer. The ghost carries the negotiated action.
        S_OK
    }
}

fn run(window: HWND, pending: Pending) {
    let mut result = FileDragFeedback {
        effect: FileDropEffect::None,
        dismiss_preview: true,
        finished: true,
        cancelled: true,
        delete_sources: false,
    };
    // Release can arrive between promotion and this posted message. Never start a
    // new drag after that release, including after switching applications.
    if unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) } < 0
        && unsafe { IsWindow(Some(window)) }.as_bool()
    {
        if unsafe { GetCapture() } == window {
            let _ = unsafe { ReleaseCapture() };
        }
        let formats = Rc::new(RefCell::new(BTreeMap::from([(
            CF_HDROP.0,
            super::windows_clipboard_write::file_bytes(&pending.paths),
        )])));
        let object: IDataObject = data::FileData {
            formats: formats.clone(),
        }
        .into();
        let source: IDropSource = Source {
            feedback: pending.feedback.clone(),
            owner: window,
            preview_dismissed: Cell::new(false),
        }
        .into();
        let performed = data::registered(w!("Performed DropEffect"));
        let mut current = DROPEFFECT_NONE;
        let status = unsafe {
            DoDragDrop(
                &object,
                &source,
                DROPEFFECT_COPY | DROPEFFECT_MOVE,
                &mut current,
            )
        };
        result.effect = effect(current);
        result.cancelled = status != DRAGDROP_S_DROP;
        let reported = formats
            .borrow()
            .get(&performed)
            .and_then(|bytes| bytes.get(..4))
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()));
        // Optimized Shell targets have already moved the files themselves. Only
        // the explicit unoptimized move acknowledgement permits source cleanup.
        result.delete_sources =
            !result.cancelled && current == DROPEFFECT_MOVE && reported == Some(DROPEFFECT_MOVE.0);
    }
    (pending.feedback.borrow_mut())(result);
    flush_dismissals();
}

#[cfg(test)]
#[path = "windows_file_drag/tests/data.rs"]
mod tests;

#[cfg(test)]
#[path = "windows_file_drag/tests/preview.rs"]
mod preview_tests;
