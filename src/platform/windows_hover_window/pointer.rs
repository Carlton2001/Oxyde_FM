//! What the pointer reaches around an interactive menu: its visible shadow belongs to
//! it, and a click that dismisses it stays with it, as Windows' own menus do.
use super::*;

/// Where the shadow (`style::rem_shadow`, below the frame) stays visible, as fractions of
/// the shadow margin: on the sides, below, and none above.
const SHADOW_SIDES: f32 = 0.25;
const SHADOW_BELOW: f32 = 0.5625;

thread_local! {
    /// The release of a press that only dismissed menus, and the window it goes to.
    static SWALLOWED_RELEASE: RefCell<Option<(isize, u32)>> = const { RefCell::new(None) };
}

/// The release matching a press, for the buttons whose dismissing press is kept.
/// A right click goes through: it opens the menu of what it hits.
pub(super) fn dismissing_release(press: u32) -> Option<u32> {
    match press {
        WM_LBUTTONDOWN => Some(WM_LBUTTONUP),
        WM_MBUTTONDOWN => Some(WM_MBUTTONUP),
        WM_XBUTTONDOWN => Some(WM_XBUTTONUP),
        _ => None,
    }
}

/// Keeps the press that closed a menu from the form beneath, and its release after it.
pub(super) fn swallow(window: HWND, release: u32) {
    SWALLOWED_RELEASE.with(|swallowed| *swallowed.borrow_mut() = Some((window.0 as isize, release)));
}

/// Whether `message` is the release of a press kept from `window`.
pub(super) fn swallowed(window: HWND, message: u32) -> bool {
    if !matches!(message, WM_LBUTTONUP | WM_MBUTTONUP | WM_XBUTTONUP) {
        return false;
    }
    SWALLOWED_RELEASE.with(|swallowed| {
        let mut swallowed = swallowed.borrow_mut();
        let matches = *swallowed == Some((window.0 as isize, message));
        // The release may come on another window (the pointer left): it is spent all the same.
        if matches || swallowed.is_some_and(|(_, release)| release == message) {
            *swallowed = None;
        }
        matches
    })
}

/// The visible frame of a shown menu, in screen pixels.
fn frame(menu: HWND) -> Option<windows::Win32::Foundation::RECT> {
    if !unsafe { IsWindowVisible(menu) }.as_bool() {
        return None;
    }
    let inset = INSETS.with(|insets| insets.borrow().get(&(menu.0 as isize)).copied())?;
    let scale = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(menu) } as f32 / 96.0;
    let inset = (inset * scale).round() as i32;
    let mut rect = windows::Win32::Foundation::RECT::default();
    unsafe { GetWindowRect(menu, &mut rect) }.ok()?;
    Some(windows::Win32::Foundation::RECT {
        left: rect.left + inset,
        top: rect.top + inset,
        right: rect.right - inset,
        bottom: rect.bottom - inset,
    })
}

fn contains(rect: &windows::Win32::Foundation::RECT, point: POINT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

/// The hit test of a menu at `screen`: its frame, then for an interactive menu its
/// visible shadow, unless another menu's frame lies there; the rest of the margin
/// lets the pointer through.
pub(super) fn hit(window: HWND, screen: POINT, interactive: bool) -> LRESULT {
    let inset = INSETS
        .with(|insets| insets.borrow().get(&(window.0 as isize)).copied())
        .unwrap_or(0.0);
    let scale = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(window) } as f32 / 96.0;
    let inset = (inset * scale).round() as i32;
    let mut point = screen;
    let mut client = windows::Win32::Foundation::RECT::default();
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::ScreenToClient(window, &mut point);
        let _ = GetClientRect(window, &mut client);
    }
    let inner = windows::Win32::Foundation::RECT {
        left: inset,
        top: inset,
        right: client.right - inset,
        bottom: client.bottom - inset,
    };
    if contains(&inner, point) {
        return LRESULT(HTCLIENT as isize);
    }
    let sides = (inset as f32 * SHADOW_SIDES).round() as i32;
    let shadow = windows::Win32::Foundation::RECT {
        left: inner.left - sides,
        top: inner.top,
        right: inner.right + sides,
        bottom: inner.bottom + (inset as f32 * SHADOW_BELOW).round() as i32,
    };
    let covers_another = || {
        MENUS.with(|menus| menus.borrow().iter().copied().collect::<Vec<_>>())
            .into_iter()
            .filter(|menu| *menu != window.0 as isize)
            .filter_map(|menu| frame(HWND(menu as *mut _)))
            .any(|frame| contains(&frame, screen))
    };
    LRESULT(if interactive && contains(&shadow, point) && !covers_another() {
        HTCLIENT as isize
    } else {
        HTTRANSPARENT as isize
    })
}

#[cfg(test)]
#[path = "tests/pointer.rs"]
mod tests;
