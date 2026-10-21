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
    above_surface: bool,
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
        // Overlays over the video are owned by its surface: Windows then keeps
        // them above it whatever the worker or an activation restacks.
        let surface = if above_surface { media_surface(owner) } else { None };
        let initialized = GetWindowLongPtrW(window, GWL_EXSTYLE) as u32 & WS_EX_NOACTIVATE.0 != 0;
        if !initialized {
            super::windows_window_owner::set_owner(window, owner)?;
            let style = GetWindowLongPtrW(window, GWL_EXSTYLE) as u32;
            SetWindowLongPtrW(
                window,
                GWL_EXSTYLE,
                ((style | WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) & !WS_EX_APPWINDOW.0) as isize,
            );
            // GPUI creates PopUp windows as WS_OVERLAPPED, whose implicit side
            // and bottom borders shrink the client area and clip the bar's end.
            let style = GetWindowLongPtrW(window, GWL_STYLE) as u32;
            SetWindowLongPtrW(
                window,
                GWL_STYLE,
                ((style | WS_POPUP.0)
                    & !(WS_CAPTION.0 | WS_THICKFRAME.0 | WS_BORDER.0 | WS_DLGFRAME.0)) as isize,
            );
            // GPUI PopUp windows are created topmost. Ownership alone keeps
            // them above the surface, and other applications may cover both.
            let _ = SetWindowPos(
                window,
                Some(HWND_NOTOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
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
        let wanted = surface.unwrap_or(owner);
        if GetWindow(window, GW_OWNER).ok() != Some(wanted) {
            super::windows_window_owner::set_owner(window, wanted)?;
        }
        if !visible
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

/// The worker's video popups owned by `owner`, found by their class.
fn media_surfaces(owner: HWND) -> Vec<HWND> {
    let mut surfaces = Vec::new();
    let mut previous = None;
    while let Ok(surface) =
        unsafe { FindWindowExW(None, previous, windows::core::w!("OxydeMediaSurface"), None) }
    {
        if unsafe { GetWindow(surface, GW_OWNER) }.ok() == Some(owner) {
            surfaces.push(surface);
        }
        previous = Some(surface);
    }
    surfaces
}

fn media_surface(owner: HWND) -> Option<HWND> {
    media_surfaces(owner).into_iter().find(|surface| unsafe { IsWindowVisible(*surface) }.as_bool())
}

/// Before a surface is destroyed, its overlays return to the UI window:
/// Windows destroys owned windows with their owner.
pub(super) fn release_overlays(owner: RawWindowHandle) {
    let RawWindowHandle::Win32(owner) = owner else { return };
    let owner = HWND(owner.hwnd.get() as *mut _);
    for surface in media_surfaces(owner) {
        let mut owned: Vec<HWND> = Vec::new();
        unsafe extern "system" fn collect(window: HWND, data: LPARAM) -> windows::core::BOOL {
            let (surface, owned) = unsafe { &mut *(data.0 as *mut (HWND, &mut Vec<HWND>)) };
            if unsafe { GetWindow(window, GW_OWNER) }.ok() == Some(*surface) {
                owned.push(window);
            }
            true.into()
        }
        let mut data = (surface, &mut owned);
        let _ = unsafe { EnumWindows(Some(collect), LPARAM(&mut data as *mut _ as isize)) };
        for window in owned {
            unsafe {
                let _ = ShowWindow(window, SW_HIDE);
            }
            let _ = super::windows_window_owner::set_owner(window, owner);
        }
    }
}

pub(super) fn cursor_client_position(handle: RawWindowHandle) -> Option<[i32; 2]> {
    let RawWindowHandle::Win32(handle) = handle else { return None };
    let window = HWND(handle.hwnd.get() as *mut _);
    let mut point = POINT::default();
    unsafe {
        GetCursorPos(&mut point).ok()?;
        windows::Win32::Graphics::Gdi::ScreenToClient(window, &mut point).as_bool().then_some([point.x, point.y])
    }
}

#[cfg(test)]
#[path = "windows_viewer_controls/tests/window.rs"]
mod tests;
