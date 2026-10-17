//! Owner-locked placement and modality for fixed secondary dialogs.
use raw_window_handle::RawWindowHandle;
use std::{cell::RefCell, collections::HashMap};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::*;

const SUBCLASS: usize = 0x4f584644;
const INITIALIZE: u32 = WM_APP + 0x360;
const RECENTER: u32 = INITIALIZE + 1;

thread_local! {
    // Dialog -> owner. Owners stay disabled while one of their dialogs is shown.
    static DIALOGS: RefCell<HashMap<isize, isize>> = RefCell::new(HashMap::new());
}

fn hwnd(handle: RawWindowHandle) -> Result<HWND, String> {
    match handle {
        RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as *mut _)),
        _ => Err("Fixed dialogs require Win32 handles".into()),
    }
}

/// Own, lock and disable the owner from the message loop, never synchronously
/// inside GPUI's window construction.
pub(super) fn configure(window: RawWindowHandle, owner: RawWindowHandle) -> Result<(), String> {
    let window = hwnd(window)?;
    let owner = hwnd(owner)?;
    super::windows_window_owner::set_owner(window, owner)?;
    DIALOGS.with(|dialogs| dialogs.borrow_mut().insert(window.0 as isize, owner.0 as isize));
    unsafe {
        let style = GetWindowLongPtrW(window, GWL_EXSTYLE) as u32;
        SetWindowLongPtrW(window, GWL_EXSTYLE, (style & !WS_EX_APPWINDOW.0) as isize);
        if !SetWindowSubclass(window, Some(subclass), SUBCLASS, 0).as_bool() {
            DIALOGS.with(|dialogs| dialogs.borrow_mut().remove(&(window.0 as isize)));
            return Err("Cannot lock fixed dialog placement".into());
        }
        PostMessageW(Some(window), INITIALIZE, WPARAM(0), LPARAM(0))
            .map_err(|error| error.to_string())
    }
}

/// Queue a recentering of every fixed dialog owned by `owner`.
pub(super) fn follow_owner(owner: RawWindowHandle) -> Result<(), String> {
    let owner = hwnd(owner)?.0 as isize;
    let dialogs: Vec<_> = DIALOGS.with(|dialogs| {
        dialogs.borrow().iter().filter(|(_, o)| **o == owner).map(|(d, _)| *d).collect()
    });
    for dialog in dialogs {
        unsafe { PostMessageW(Some(HWND(dialog as *mut _)), RECENTER, WPARAM(0), LPARAM(0)) }
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn owner_of(window: HWND) -> Option<HWND> {
    DIALOGS.with(|dialogs| dialogs.borrow().get(&(window.0 as isize)).map(|o| HWND(*o as *mut _)))
}

/// Centered over the owner frame, kept inside the owner's work area.
pub(super) fn centered_origin(owner: RECT, work: RECT, width: i32, height: i32) -> (i32, i32) {
    let x = owner.left + (owner.right - owner.left - width) / 2;
    let y = owner.top + (owner.bottom - owner.top - height) / 2;
    (
        x.clamp(work.left, (work.right - width).max(work.left)),
        y.clamp(work.top, (work.bottom - height).max(work.top)),
    )
}

unsafe fn origin_for(owner: HWND, width: i32, height: i32) -> Option<(i32, i32)> {
    let mut frame = RECT::default();
    unsafe { GetWindowRect(owner, &mut frame) }.ok()?;
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    let monitor = unsafe { MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST) };
    let work = if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() { info.rcWork } else { frame };
    Some(centered_origin(frame, work, width, height))
}

/// Re-enable the owner before the dialog hides, so Windows returns activation
/// to Oxyde rather than to another application.
unsafe fn release(window: HWND) {
    let Some(owner) = DIALOGS.with(|dialogs| dialogs.borrow_mut().remove(&(window.0 as isize)).map(|o| HWND(o as *mut _))) else {
        return;
    };
    let shared = DIALOGS.with(|dialogs| dialogs.borrow().values().any(|o| *o == owner.0 as isize));
    if !shared {
        let _ = unsafe { EnableWindow(owner, true) };
    }
}

unsafe extern "system" fn subclass(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    match message {
        INITIALIZE => {
            if let Some(owner) = owner_of(window) {
                let _ = unsafe { EnableWindow(owner, false) };
            }
            let _ = unsafe { SetWindowPos(window, None, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE) };
            return LRESULT(0);
        }
        RECENTER => {
            let _ = unsafe { SetWindowPos(window, None, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE) };
            return LRESULT(0);
        }
        WM_WINDOWPOSCHANGING => {
            let position = unsafe { &mut *(lparam.0 as *mut WINDOWPOS) };
            if position.flags.contains(SWP_HIDEWINDOW) {
                unsafe { release(window) };
            } else if let Some(owner) = owner_of(window) {
                let (width, height) = if position.flags.contains(SWP_NOSIZE) {
                    let mut current = RECT::default();
                    let _ = unsafe { GetWindowRect(window, &mut current) };
                    (current.right - current.left, current.bottom - current.top)
                } else {
                    (position.cx, position.cy)
                };
                if let Some((x, y)) = unsafe { origin_for(owner, width, height) } {
                    position.x = x;
                    position.y = y;
                    position.flags &= !SWP_NOMOVE;
                }
            }
        }
        WM_SYSCOMMAND if matches!(wparam.0 as u32 & 0xfff0, SC_MOVE | SC_SIZE | SC_MAXIMIZE | SC_MINIMIZE) => {
            return LRESULT(0);
        }
        WM_DESTROY => unsafe { release(window) },
        WM_NCDESTROY => {
            let _ = unsafe { RemoveWindowSubclass(window, Some(subclass), SUBCLASS) };
        }
        _ => {}
    }
    unsafe { DefSubclassProc(window, message, wparam, lparam) }
}

#[cfg(test)]
#[path = "windows_fixed_dialog/tests/placement.rs"]
mod tests;
