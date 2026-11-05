use raw_window_handle::RawWindowHandle;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, PostMessageW};

/// Undocumented but stable message the taskbar sends: `DefWindowProc` shows the system menu at
/// the screen point in `lparam` and runs the chosen command.
const WM_SYSMENU: u32 = 0x0313;

/// Windows no longer opens the system menu for a right click on a custom caption, so ask for
/// it at the pointer. Posted, so the menu loop runs after GPUI has finished the event.
pub(super) fn show(handle: RawWindowHandle) -> Result<(), String> {
    let RawWindowHandle::Win32(handle) = handle else { return Err("Not a Win32 window".into()) };
    let window = HWND(handle.hwnd.get() as *mut _);
    let mut point = POINT::default();
    unsafe {
        GetCursorPos(&mut point).map_err(|error| error.to_string())?;
        let position = (point.x as u16 as u32 | ((point.y as u16 as u32) << 16)) as isize;
        PostMessageW(Some(window), WM_SYSMENU, WPARAM(0), LPARAM(position)).map_err(|error| error.to_string())
    }
}
