use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetCapture, ReleaseCapture, SetCapture};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{WM_LBUTTONDOWN, WM_LBUTTONUP, WM_NCDESTROY};

const POINTER_SUBCLASS: usize = 0x4f585950;

#[cfg(test)]
#[path = "windows_pointer/tests/capture.rs"]
mod capture_tests;

#[cfg(test)]
#[path = "windows_pointer/tests/image_cursor.rs"]
mod image_cursor_tests;

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

const IMAGE_SUBCLASS: usize = 0x4f58595a;

fn image_cursor(hwnd: HWND, dragging: bool) -> windows::Win32::UI::WindowsAndMessaging::HCURSOR {
    use windows::Win32::UI::WindowsAndMessaging::{CreateIconFromResourceEx, HCURSOR, IMAGE_FLAGS};
    use std::{collections::HashMap, sync::{Mutex, OnceLock}};
    static CURSORS: OnceLock<Mutex<HashMap<(bool, u32), usize>>> = OnceLock::new();
    let bytes: &[u8] = if dragging {
        include_bytes!("../../assets/cursors/hand-grab.cur")
    } else {
        include_bytes!("../../assets/cursors/search.cur")
    };
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) }.max(96);
    let requested = 32 * dpi / 96;
    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    let entry = (0..count).map(|i| &bytes[6 + i * 16..6 + (i + 1) * 16])
        .min_by_key(|entry| u32::from(entry[0]).abs_diff(requested)).expect("embedded cursor frames");
    let side = u32::from(entry[0]);
    let mut cache = CURSORS.get_or_init(Default::default).lock().expect("image cursor cache");
    HCURSOR(*cache.entry((dragging, side)).or_insert_with(|| {
        let length = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
        // RT_CURSOR is the hotspot followed by the DIB; the CUR directory stays outside.
        let mut resource = entry[4..8].to_vec();
        resource.extend_from_slice(&bytes[offset..offset + length]);
        match unsafe { CreateIconFromResourceEx(&resource, false, 0x00030000, side as i32, side as i32, IMAGE_FLAGS(0)) } {
            Ok(cursor) => cursor.0 as usize,
            Err(error) => { log::warn!("Cannot create Lucide image cursor: {error}"); 0 }
        }
    }) as *mut _)
}

unsafe extern "system" fn image_subclass(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM, id: usize, cursor: usize) -> LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{SetCursor, HCURSOR, WM_SETCURSOR, WM_MOUSEMOVE, HTCLIENT};
    if message == WM_SETCURSOR && lparam.0 as u16 as u32 == HTCLIENT {
        unsafe { SetCursor(Some(HCURSOR(cursor as *mut _))); }
        return LRESULT(1);
    }
    if message == WM_NCDESTROY { let _ = unsafe { RemoveWindowSubclass(hwnd, Some(image_subclass), id) }; }
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    // WM_SETCURSOR is not sent while the owner captures the pointer during a drag.
    if message == WM_MOUSEMOVE { unsafe { SetCursor(Some(HCURSOR(cursor as *mut _))); } }
    result
}

pub(super) fn set_image_cursor(hwnd: HWND, zooming: bool, dragging: bool) {
    use windows::Win32::UI::WindowsAndMessaging::SetCursor;
    if zooming || dragging {
        let cursor = image_cursor(hwnd, dragging);
        if !cursor.0.is_null() {
            let _ = unsafe { SetWindowSubclass(hwnd, Some(image_subclass), IMAGE_SUBCLASS, cursor.0 as usize) };
            unsafe { SetCursor(Some(cursor)); }
        }
    } else if unsafe { RemoveWindowSubclass(hwnd, Some(image_subclass), IMAGE_SUBCLASS) }.as_bool() {
        use windows::Win32::UI::WindowsAndMessaging::{LoadCursorW, IDC_ARROW};
        if let Ok(cursor) = unsafe { LoadCursorW(None, IDC_ARROW) } { unsafe { SetCursor(Some(cursor)); } }
    }
}
