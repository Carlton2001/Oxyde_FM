//! Nonactivating controls above the worker's video popup, without global topmost.
use raw_window_handle::RawWindowHandle;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Gdi::ClientToScreen,
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

const SUBCLASS: usize = 0x4f585943;

unsafe extern "system" fn controls_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    _: usize,
) -> LRESULT {
    if message == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    if message == WM_NCDESTROY {
        let _ = unsafe { RemoveWindowSubclass(hwnd, Some(controls_proc), id) };
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

pub(super) fn place(
    window: RawWindowHandle,
    owner: RawWindowHandle,
    rect: [i32; 4],
    visible: bool,
) -> Result<(), String> {
    let (RawWindowHandle::Win32(window), RawWindowHandle::Win32(owner)) = (window, owner) else {
        return Err("Expected Win32 playback windows".into());
    };
    let window = HWND(window.hwnd.get() as *mut _);
    let owner = HWND(owner.hwnd.get() as *mut _);
    unsafe {
        if !IsWindow(Some(owner)).as_bool() {
            return Ok(());
        }
        if GetWindow(window, GW_OWNER).ok() != Some(owner) {
            super::windows_window_owner::set_owner(window, owner)?;
            let style = GetWindowLongPtrW(window, GWL_EXSTYLE) as u32;
            SetWindowLongPtrW(
                window,
                GWL_EXSTYLE,
                ((style | WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) & !WS_EX_APPWINDOW.0) as isize,
            );
            if !SetWindowSubclass(window, Some(controls_proc), SUBCLASS, 0).as_bool() {
                return Err("Cannot configure playback controls".into());
            }
            super::windows_pointer::install(window)?;
            use windows::Win32::Graphics::Dwm::{
                DwmSetWindowAttribute, DWMNCRP_DISABLED, DWMWA_NCRENDERING_POLICY,
            };
            let _ = DwmSetWindowAttribute(
                window,
                DWMWA_NCRENDERING_POLICY,
                &DWMNCRP_DISABLED as *const _ as *const _,
                std::mem::size_of_val(&DWMNCRP_DISABLED) as u32,
            );
        }
        let foreground = GetForegroundWindow();
        let active = foreground == owner
            || GetAncestor(foreground, GA_ROOTOWNER) == GetAncestor(owner, GA_ROOTOWNER);
        if !visible
            || !active
            || !IsWindowVisible(owner).as_bool()
            || IsIconic(owner).as_bool()
            || !windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(owner).as_bool()
        {
            if IsWindowVisible(window).as_bool() {
                let _ = ShowWindow(window, SW_HIDE);
            }
            return Ok(());
        }
        let [x, y, width, height] = rect;
        let mut origin = POINT { x, y };
        ClientToScreen(owner, &mut origin)
            .ok()
            .map_err(|error| error.to_string())?;
        SetWindowPos(
            window,
            Some(HWND_TOP),
            origin.x,
            origin.y,
            width.max(1),
            height.max(1),
            SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        )
        .map_err(|error| error.to_string())?;
        if !IsWindowVisible(window).as_bool() {
            let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "windows_viewer_controls/tests/window.rs"]
mod tests;
