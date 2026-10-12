use windows::Win32::Foundation::{GetLastError, SetLastError, HWND, WIN32_ERROR};
use windows::Win32::UI::WindowsAndMessaging::{SetWindowLongPtrW, GWLP_HWNDPARENT};

pub(super) fn set_owner(window: HWND, owner: HWND) -> Result<(), String> {
    if window == owner || window.0.is_null() || owner.0.is_null() {
        return Err("Invalid tool window owner".into());
    }
    unsafe {
        // These are top-level windows: GWLP_HWNDPARENT sets ownership, not
        // WS_CHILD parenting. The owner remains enabled and other apps can
        // cover both windows; no global topmost flag is used.
        SetLastError(WIN32_ERROR(0));
        let previous = SetWindowLongPtrW(window, GWLP_HWNDPARENT, owner.0 as isize);
        let error = GetLastError();
        if previous == 0 && error.0 != 0 {
            return Err(std::io::Error::from_raw_os_error(error.0 as i32).to_string());
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "windows_window_owner/tests/ownership.rs"]
mod tests;
