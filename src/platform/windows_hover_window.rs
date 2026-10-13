//! Native behavior for the experimental GPUI image/PDF hover surface.
use raw_window_handle::RawWindowHandle;
use std::{cell::RefCell, collections::HashMap};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMNCRP_DISABLED, DWMWA_NCRENDERING_POLICY,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::*;

const SUBCLASS: usize = 0x4f585948;
const APPLY_PLACEMENT: u32 = WM_APP + 0x348;
const INITIALIZE_WINDOW: u32 = APPLY_PLACEMENT + 1;

#[derive(Clone, Copy)]
struct Placement {
    size: Option<(f32, f32)>,
    scale: f32,
}

thread_local! {
    // Coalesce pointer requests without entering GPUI through synchronous
    // WM_MOVE/WM_SIZE callbacks while its App/Window is already borrowed.
    static PENDING: RefCell<HashMap<isize, Placement>> = RefCell::new(HashMap::new());
}

fn hwnd(handle: RawWindowHandle) -> Result<HWND, String> {
    match handle {
        RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as *mut _)),
        _ => Err("Hover windows require Win32 handles".into()),
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
        INITIALIZE_WINDOW => {
            // Keep the prototype's native surface plain: no system halo and
            // no additional custom shadow pass around moving previews.
            let policy = DWMNCRP_DISABLED;
            if let Err(error) = unsafe {
                DwmSetWindowAttribute(
                    window,
                    DWMWA_NCRENDERING_POLICY,
                    std::ptr::from_ref(&policy).cast(),
                    std::mem::size_of_val(&policy) as u32,
                )
            } {
                log::warn!("Cannot disable native hover decoration: {error}");
            }
            // Ownership supplies stacking without being globally topmost.
            // Run outside the GPUI App borrow, just like movement and resize.
            if let Err(error) = unsafe {
                SetWindowPos(
                    window,
                    Some(HWND_NOTOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                )
            } {
                log::warn!("Cannot initialize hover window: {error}");
            }
            return LRESULT(0);
        }
        APPLY_PLACEMENT => {
            let pending = PENDING.with(|pending| pending.borrow_mut().remove(&(window.0 as isize)));
            if let Some(pending) = pending {
                if let Err(error) = place_now(window, pending.size, pending.scale) {
                    log::warn!("Cannot apply hover placement: {error}");
                }
            }
            return LRESULT(0);
        }
        WM_NCHITTEST => return LRESULT(HTTRANSPARENT as isize),
        WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
        WM_NCDESTROY => unsafe {
            PENDING.with(|pending| pending.borrow_mut().remove(&(window.0 as isize)));
            let _ = RemoveWindowSubclass(window, Some(subclass), SUBCLASS);
        },
        _ => {}
    }
    unsafe { DefSubclassProc(window, message, wparam, lparam) }
}

pub(super) fn configure(handle: RawWindowHandle) -> Result<(), String> {
    let window = hwnd(handle)?;
    unsafe {
        let style = GetWindowLongPtrW(window, GWL_EXSTYLE) as u32;
        SetWindowLongPtrW(
            window,
            GWL_EXSTYLE,
            ((style | WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) & !WS_EX_APPWINDOW.0) as isize,
        );
        if !SetWindowSubclass(window, Some(subclass), SUBCLASS, 0).as_bool() {
            return Err("Cannot install hover window hit testing".into());
        }
        PostMessageW(Some(window), INITIALIZE_WINDOW, WPARAM(0), LPARAM(0))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(super) fn place(
    handle: RawWindowHandle,
    size: Option<(f32, f32)>,
    scale: f32,
) -> Result<(), String> {
    let window = hwnd(handle)?;
    if let Some((width, height)) = size {
        if !scale.is_finite()
            || scale <= 0.0
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
        {
            return Err("Invalid hover window dimensions".into());
        }
    }
    let key = window.0 as isize;
    let queued = PENDING.with(|pending| {
        pending
            .borrow_mut()
            .insert(key, Placement { size, scale })
            .is_some()
    });
    if !queued {
        if let Err(error) =
            unsafe { PostMessageW(Some(window), APPLY_PLACEMENT, WPARAM(0), LPARAM(0)) }
        {
            PENDING.with(|pending| pending.borrow_mut().remove(&key));
            return Err(error.to_string());
        }
    }
    Ok(())
}

fn place_now(window: HWND, size: Option<(f32, f32)>, scale: f32) -> Result<(), String> {
    unsafe {
        let Some((width, height)) = size else {
            if IsWindowVisible(window).as_bool() {
                let _ = ShowWindow(window, SW_HIDE);
            }
            return Ok(());
        };
        if !scale.is_finite()
            || scale <= 0.0
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
        {
            return Err("Invalid hover window dimensions".into());
        }
        let mut cursor = POINT::default();
        GetCursorPos(&mut cursor).map_err(|error| error.to_string())?;
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return Err("Cannot read hover monitor bounds".into());
        }
        let work = info.rcWork;
        let width = ((width * scale).ceil() as i32).clamp(1, (work.right - work.left).max(1));
        let height = ((height * scale).ceil() as i32).clamp(1, (work.bottom - work.top).max(1));
        let gap = (24.0 * scale).round() as i32;
        let x = if cursor.x + gap + width <= work.right {
            cursor.x + gap
        } else {
            cursor.x - gap - width
        };
        let y = if cursor.y + gap + height <= work.bottom {
            cursor.y + gap
        } else {
            cursor.y - gap - height
        };
        let x = x.clamp(work.left, work.right - width);
        let y = y.clamp(work.top, work.bottom - height);
        // The lifecycle watcher also runs while the pointer is stationary.
        // Do not generate redundant native resize/paint events on each tick.
        let mut current = windows::Win32::Foundation::RECT::default();
        GetWindowRect(window, &mut current).map_err(|error| error.to_string())?;
        if IsWindowVisible(window).as_bool()
            && current.left == x
            && current.top == y
            && current.right - current.left == width
            && current.bottom - current.top == height
        {
            return Ok(());
        }
        SetWindowPos(
            window,
            None,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE
                | SWP_NOZORDER
                | SWP_SHOWWINDOW
                | if current.right - current.left == width && current.bottom - current.top == height
                {
                    SWP_NOSIZE
                } else {
                    SET_WINDOW_POS_FLAGS(0)
                },
        )
        .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
#[path = "windows_hover_window/tests/native_behavior.rs"]
mod tests;
