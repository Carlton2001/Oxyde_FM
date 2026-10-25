//! Deferred Win32 placement, hit testing and ownership for transient GPUI surfaces.
mod keyboard;
use raw_window_handle::RawWindowHandle;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};
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
const OWNER_SUBCLASS: usize = SUBCLASS + 1;

#[derive(Clone, Copy)]
struct Placement {
    size: Option<(f32, f32, f32)>,
    scale: f32,
}

/// Anchor rectangle and its three placement flags.
type Anchor = ((f32, f32, f32, f32), bool, bool, bool);

thread_local! {
    // Coalesce pointer requests without entering GPUI through synchronous
    // WM_MOVE/WM_SIZE callbacks while its App/Window is already borrowed.
    static PENDING: RefCell<HashMap<isize, Placement>> = RefCell::new(HashMap::new());
    static ANCHORS: RefCell<HashMap<isize, Anchor>> = RefCell::new(HashMap::new());
    static MENUS: RefCell<HashSet<isize>> = RefCell::new(HashSet::new());
    static COORDINATE_OWNERS: RefCell<HashMap<isize, isize>> = RefCell::new(HashMap::new());
    static INSETS: RefCell<HashMap<isize, f32>> = RefCell::new(HashMap::new());
}

pub(super) fn configure_menu(
    handle: RawWindowHandle,
    owner: RawWindowHandle,
    coordinate_owner: RawWindowHandle,
    interactive: bool,
) -> Result<(), String> {
    configure(handle)?;
    let window = hwnd(handle)?;
    let owner = hwnd(owner)?;
    let coordinate_owner = hwnd(coordinate_owner)?;
    COORDINATE_OWNERS.with(|owners| {
        owners
            .borrow_mut()
            .insert(window.0 as isize, coordinate_owner.0 as isize)
    });
    MENUS.with(|menus| menus.borrow_mut().insert(window.0 as isize));
    keyboard::register(window, coordinate_owner, interactive)?;
    unsafe {
        let style = GetWindowLongPtrW(window, GWL_EXSTYLE) as u32;
        SetWindowLongPtrW(
            window,
            GWL_EXSTYLE,
            (style | WS_EX_NOACTIVATE.0) as isize,
        );
        if !SetWindowSubclass(owner, Some(owner_subclass), OWNER_SUBCLASS, 0).as_bool() {
            return Err("Cannot observe menu owner".into());
        }
        if !SetWindowSubclass(window, Some(owner_subclass), OWNER_SUBCLASS, 0).as_bool() {
            return Err("Cannot observe submenu owner".into());
        }
    }
    Ok(())
}

unsafe extern "system" fn owner_subclass(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    if matches!(
        message,
        WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
            | WM_NCLBUTTONDOWN | WM_NCRBUTTONDOWN | WM_NCMBUTTONDOWN | WM_NCXBUTTONDOWN
            | WM_CLOSE
    ) {
        let menus: Vec<_> = MENUS.with(|menus| menus.borrow().iter().copied().collect());
        for menu in menus {
            let menu = HWND(menu as *mut _);
            if unsafe { GetWindow(menu, GW_OWNER) }.ok() == Some(window) {
                if matches!(message, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN) {
                    let anchor =
                        ANCHORS.with(|anchors| anchors.borrow().get(&(menu.0 as isize)).copied());
                    if let Some(((x, y, width, height), ..)) = anchor {
                        let reference = COORDINATE_OWNERS
                            .with(|owners| owners.borrow().get(&(menu.0 as isize)).copied())
                            .unwrap_or(window.0 as isize);
                        let reference = HWND(reference as *mut _);
                        let scale = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(reference) }
                            as f32
                            / 96.0;
                        let mut point = POINT::default();
                        if unsafe { GetCursorPos(&mut point) }.is_ok()
                            && unsafe {
                                windows::Win32::Graphics::Gdi::ScreenToClient(reference, &mut point)
                            }
                            .as_bool()
                            && point.x as f32 >= x * scale
                            && (point.x as f32) < (x + width) * scale
                            && point.y as f32 >= y * scale
                            && (point.y as f32) < (y + height) * scale
                        {
                            continue;
                        }
                    }
                }
                // Close before the owner handles the click, so a newly opened
                // menu cannot be dismissed by a delayed close of its predecessor.
                unsafe {
                    SendMessageW(menu, WM_CLOSE, None, None);
                }
            }
        }
    }
    if message == WM_NCDESTROY {
        keyboard::remove(window);
        unsafe {
            let _ = RemoveWindowSubclass(window, Some(owner_subclass), OWNER_SUBCLASS);
        }
    }
    unsafe { DefSubclassProc(window, message, wparam, lparam) }
}

pub(super) fn point_to_owner(
    window: RawWindowHandle,
    owner: RawWindowHandle,
    point: (f32, f32),
) -> Result<(f32, f32), String> {
    let window = hwnd(window)?;
    let owner = hwnd(owner)?;
    unsafe {
        let scale = windows::Win32::UI::HiDpi::GetDpiForWindow(window) as f32 / 96.0;
        let owner_scale = windows::Win32::UI::HiDpi::GetDpiForWindow(owner) as f32 / 96.0;
        let mut point = POINT {
            x: (point.0 * scale).round() as i32,
            y: (point.1 * scale).round() as i32,
        };
        windows::Win32::Graphics::Gdi::ClientToScreen(window, &mut point)
            .ok()
            .map_err(|error| error.to_string())?;
        windows::Win32::Graphics::Gdi::ScreenToClient(owner, &mut point)
            .ok()
            .map_err(|error| error.to_string())?;
        Ok((point.x as f32 / owner_scale, point.y as f32 / owner_scale))
    }
}
pub(super) fn family_active(owner: RawWindowHandle) -> bool {
    let Ok(owner) = hwnd(owner) else {
        return false;
    };
    unsafe {
        let foreground = GetForegroundWindow();
        foreground == owner
            || (MENUS.with(|menus| menus.borrow().contains(&(foreground.0 as isize)))
                && root_owner(foreground) == owner)
    }
}
unsafe fn root_owner(mut window: HWND) -> HWND {
    for _ in 0..32 {
        let Ok(owner) = (unsafe { GetWindow(window, GW_OWNER) }) else {
            break;
        };
        if !MENUS.with(|menus| menus.borrow().contains(&(window.0 as isize))) {
            break;
        }
        window = owner;
    }
    window
}
pub(super) fn work_area_size(handle: RawWindowHandle) -> Result<(f32, f32), String> {
    let window = hwnd(handle)?;
    unsafe {
        let monitor =
            windows::Win32::Graphics::Gdi::MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return Err("Cannot read popup work area".into());
        }
        let scale = windows::Win32::UI::HiDpi::GetDpiForWindow(window) as f32 / 96.0;
        Ok((
            (info.rcWork.right - info.rcWork.left) as f32 / scale,
            (info.rcWork.bottom - info.rcWork.top) as f32 / scale,
        ))
    }
}

#[cfg(test)]
pub(super) fn set_anchor(
    handle: RawWindowHandle,
    bounds: Option<(f32, f32, f32, f32)>,
    open_left: bool,
    beside: bool,
) -> Result<(), String> {
    set_anchor_with(handle, bounds, open_left, beside, false)
}

pub(super) fn set_anchor_with(
    handle: RawWindowHandle,
    bounds: Option<(f32, f32, f32, f32)>,
    open_left: bool,
    beside: bool,
    above: bool,
) -> Result<(), String> {
    if bounds.is_some_and(|(x, y, w, h)| {
        ![x, y, w, h].into_iter().all(f32::is_finite) || w < 0.0 || h < 0.0
    }) {
        return Err("Invalid popup anchor".into());
    }
    let window = hwnd(handle)?;
    ANCHORS.with(|anchors| {
        let mut anchors = anchors.borrow_mut();
        if let Some(bounds) = bounds {
            anchors.insert(window.0 as isize, (bounds, open_left, beside, above));
        } else {
            anchors.remove(&(window.0 as isize));
        }
    });
    Ok(())
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
        WM_ACTIVATE
            if MENUS.with(|menus| menus.borrow().contains(&(window.0 as isize)))
                && (wparam.0 & 0xffff) == WA_INACTIVE as usize =>
        {
            let target = HWND(lparam.0 as *mut _);
            let same_family = unsafe { root_owner(window) == root_owner(target) }
                && MENUS.with(|menus| menus.borrow().contains(&(target.0 as isize)));
            let owner = unsafe { GetWindow(window, GW_OWNER) }.ok();
            if !same_family && owner != Some(target) {
                let _ = unsafe { PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0)) };
            }
        }
        INITIALIZE_WINDOW => {
            // GPUI creates WindowKind::PopUp with style 0 (WS_OVERLAPPED).
            // Windows gives that style a non-client border even with DWM
            // decoration disabled, shifting the painted client area sideways.
            // Change the frame outside GPUI's borrow, before FRAMECHANGED.
            unsafe {
                let style = GetWindowLongPtrW(window, GWL_STYLE) as u32;
                SetWindowLongPtrW(
                    window,
                    GWL_STYLE,
                    ((style | WS_POPUP.0)
                        & !(WS_CAPTION.0 | WS_THICKFRAME.0 | WS_BORDER.0 | WS_DLGFRAME.0)) as isize,
                );
            }
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
        WM_NCHITTEST => {
            if !MENUS.with(|menus| menus.borrow().contains(&(window.0 as isize))) {
                return LRESULT(HTTRANSPARENT as isize);
            }
            let inset = INSETS
                .with(|insets| insets.borrow().get(&(window.0 as isize)).copied())
                .unwrap_or(0.0);
            let scale = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(window) } as f32 / 96.0;
            let inset = (inset * scale).round() as i32;
            let mut point = POINT {
                x: lparam.0 as i16 as i32,
                y: (lparam.0 >> 16) as i16 as i32,
            };
            let mut client = windows::Win32::Foundation::RECT::default();
            unsafe {
                let _ = windows::Win32::Graphics::Gdi::ScreenToClient(window, &mut point);
                let _ = GetClientRect(window, &mut client);
            }
            return LRESULT(
                if point.x >= inset
                    && point.y >= inset
                    && point.x < client.right - inset
                    && point.y < client.bottom - inset
                {
                    HTCLIENT as isize
                } else {
                    HTTRANSPARENT as isize
                },
            );
        }
        WM_MOUSEACTIVATE => {
            return LRESULT(MA_NOACTIVATE as isize)
        }
        WM_NCDESTROY => unsafe {
            keyboard::remove(window);
            ANCHORS.with(|anchors| anchors.borrow_mut().remove(&(window.0 as isize)));
            MENUS.with(|menus| menus.borrow_mut().remove(&(window.0 as isize)));
            COORDINATE_OWNERS.with(|owners| owners.borrow_mut().remove(&(window.0 as isize)));
            INSETS.with(|insets| insets.borrow_mut().remove(&(window.0 as isize)));
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
    size: Option<(f32, f32, f32)>,
    scale: f32,
) -> Result<(), String> {
    let window = hwnd(handle)?;
    if let Some((width, height, inset)) = size {
        if !scale.is_finite()
            || scale <= 0.0
            || !width.is_finite()
            || !height.is_finite()
            || !inset.is_finite()
            || inset < 0.0
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

fn axis_origin(cursor: i32, extent: i32, minimum: i32, maximum: i32, gap: i32, inset: i32) -> i32 {
    let forward = cursor + gap - inset;
    let origin = if forward + extent <= maximum {
        forward
    } else {
        cursor - gap - extent + inset
    };
    origin.clamp(minimum, maximum - extent)
}

fn place_now(window: HWND, size: Option<(f32, f32, f32)>, scale: f32) -> Result<(), String> {
    unsafe {
        let Some((width, height, inset)) = size else {
            if IsWindowVisible(window).as_bool() {
                let _ = ShowWindow(window, SW_HIDE);
            }
            return Ok(());
        };
        INSETS.with(|insets| insets.borrow_mut().insert(window.0 as isize, inset));
        if !scale.is_finite()
            || scale <= 0.0
            || !width.is_finite()
            || !height.is_finite()
            || !inset.is_finite()
            || inset < 0.0
            || width <= 0.0
            || height <= 0.0
        {
            return Err("Invalid hover window dimensions".into());
        }
        let mut cursor = POINT::default();
        GetCursorPos(&mut cursor).map_err(|error| error.to_string())?;
        let anchor = ANCHORS.with(|anchors| anchors.borrow().get(&(window.0 as isize)).copied());
        let mut anchor_top = cursor.y;
        let mut anchor_width = 0;
        if let Some(((x, y, width, height), ..)) = anchor {
            let owner = COORDINATE_OWNERS
                .with(|owners| owners.borrow().get(&(window.0 as isize)).copied())
                .map(|owner| HWND(owner as *mut _))
                .map(Ok)
                .unwrap_or_else(|| GetWindow(window, GW_OWNER))
                .map_err(|error| error.to_string())?;
            let owner_scale = windows::Win32::UI::HiDpi::GetDpiForWindow(owner) as f32 / 96.0;
            cursor = POINT {
                x: (x * owner_scale).round() as i32,
                y: (y * owner_scale).round() as i32,
            };
            windows::Win32::Graphics::Gdi::ClientToScreen(owner, &mut cursor)
                .ok()
                .map_err(|error| error.to_string())?;
            anchor_top = cursor.y;
            anchor_width = (width * owner_scale).round() as i32;
            cursor.y += (height * owner_scale).round() as i32;
        }
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
        let gap = (if let Some(((.., height), _, _, above)) = anchor {
            if height > 0.0 || above {
                4.0
            } else {
                0.0
            }
        } else {
            12.0
        } * scale)
            .round() as i32;
        let inset = (inset * scale).round() as i32;
        let (x, y) = if let Some((_, open_left, beside, above)) = anchor {
            let x = if beside {
                if open_left || cursor.x + anchor_width - inset + width > work.right {
                    cursor.x - width + inset
                } else {
                    cursor.x + anchor_width - inset
                }
            } else if open_left {
                cursor.x + anchor_width - width + inset
            } else {
                cursor.x - inset
            };
            let y = if beside {
                anchor_top - inset
            } else if !above && cursor.y + gap - inset + height <= work.bottom {
                cursor.y + gap - inset
            } else {
                anchor_top - gap - height + inset
            };
            (
                // Transparent shadow margins may extend outside the work
                // area; clamp the visible frame instead of shifting its anchor.
                x.clamp(work.left - inset, work.right - width + inset),
                y.clamp(work.top - inset, work.bottom - height + inset),
            )
        } else {
            (
                axis_origin(cursor.x, width, work.left, work.right, gap, inset),
                axis_origin(cursor.y, height, work.top, work.bottom, gap, inset),
            )
        };
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
        // A reused hover keeps its creation rank among the owner's popups;
        // raise it when shown so a later native surface (Viewer video) is below.
        let showing = !IsWindowVisible(window).as_bool();
        SetWindowPos(
            window,
            showing.then_some(HWND_TOP),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE
                | SWP_NOOWNERZORDER
                | if showing { SET_WINDOW_POS_FLAGS(0) } else { SWP_NOZORDER }
                | SWP_SHOWWINDOW
                | if current.right - current.left == width && current.bottom - current.top == height
                {
                    SWP_NOSIZE
                } else {
                    SET_WINDOW_POS_FLAGS(0)
                },
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "windows_hover_window/tests/native_behavior.rs"]
mod tests;
