use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetCapture, ReleaseCapture, SetCapture};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{WM_LBUTTONDOWN, WM_LBUTTONUP, WM_NCDESTROY};

const POINTER_SUBCLASS: usize = 0x4f585950;

#[cfg(test)]
#[path = "windows_pointer/tests/capture.rs"]
mod capture_tests;

unsafe extern "system" fn pointer_subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    _: usize,
) -> LRESULT {
    if message == WM_LBUTTONDOWN {
        unsafe {
            SetCapture(hwnd);
        }
    }
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    if message == WM_LBUTTONUP && unsafe { GetCapture() } == hwnd {
        let _ = unsafe { ReleaseCapture() };
    }
    if message == WM_NCDESTROY {
        let _ = unsafe { RemoveWindowSubclass(hwnd, Some(pointer_subclass), id) };
    }
    result
}

pub(super) fn install(hwnd: HWND) -> Result<(), String> {
    if unsafe { SetWindowSubclass(hwnd, Some(pointer_subclass), POINTER_SUBCLASS, 0) }.as_bool() {
        Ok(())
    } else {
        Err("Cannot install pointer capture handler".into())
    }
}
