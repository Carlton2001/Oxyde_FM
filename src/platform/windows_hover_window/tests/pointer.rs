use super::*;
use raw_window_handle::Win32WindowHandle;
use std::num::NonZeroIsize;
use windows::core::w;

struct TestWindow(HWND);
impl TestWindow {
    fn new() -> Self {
        Self(unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST, w!("STATIC"), w!("Oxyde pointer test"), WS_POPUP,
                0, 0, 100, 100, None, None, None, None,
            )
            .unwrap()
        })
    }
    fn raw(&self) -> RawWindowHandle {
        RawWindowHandle::Win32(Win32WindowHandle::new(NonZeroIsize::new(self.0 .0 as isize).unwrap()))
    }
}
impl Drop for TestWindow {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

fn apply_pending(window: HWND) {
    unsafe {
        let mut message = MSG::default();
        while PeekMessageW(&mut message, Some(window), APPLY_PLACEMENT, INITIALIZE_WINDOW, PM_REMOVE).as_bool() {
            DispatchMessageW(&message);
        }
    }
}

/// A menu 200 × 100 inside a 16 px shadow margin, owned by `owner`.
fn menu(owner: &TestWindow, interactive: bool) -> TestWindow {
    let menu = TestWindow::new();
    crate::platform::windows_window_owner::set_owner(menu.0, owner.0).unwrap();
    configure_menu(menu.raw(), owner.raw(), owner.raw(), interactive).unwrap();
    apply_pending(menu.0);
    set_anchor(menu.raw(), Some((200.0, 200.0, 40.0, 20.0)), false, false).unwrap();
    let scale = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(menu.0) } as f32 / 96.0;
    place(menu.raw(), Some((232.0, 132.0, 16.0)), scale).unwrap();
    apply_pending(menu.0);
    menu
}

fn hit_at(menu: &TestWindow, x: i32, y: i32) -> isize {
    let coordinates = (x as u16 as u32) | ((y as u16 as u32) << 16);
    unsafe { SendMessageW(menu.0, WM_NCHITTEST, None, Some(LPARAM(coordinates as isize))) }.0
}

fn bounds(menu: &TestWindow) -> windows::Win32::Foundation::RECT {
    let mut bounds = windows::Win32::Foundation::RECT::default();
    unsafe { GetWindowRect(menu.0, &mut bounds) }.unwrap();
    bounds
}

#[test]
fn an_interactive_menu_keeps_the_pointer_over_its_visible_shadow_only() {
    let owner = TestWindow::new();
    let menu = menu(&owner, true);
    let b = bounds(&menu);
    let scale = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(menu.0) } as f32 / 96.0;
    let inset = (16.0 * scale).round() as i32;
    let middle = (b.left + b.right) / 2;
    // Just below the frame, in the shadow; at the margin's end, beyond it; above, none.
    assert_eq!(hit_at(&menu, middle, b.bottom - inset + 2), HTCLIENT as isize);
    assert_eq!(hit_at(&menu, middle, b.bottom - 1), HTTRANSPARENT as isize);
    assert_eq!(hit_at(&menu, middle, b.top + inset - 2), HTTRANSPARENT as isize);
    assert_eq!(hit_at(&menu, b.left + inset - 2, (b.top + b.bottom) / 2), HTCLIENT as isize);
    assert_eq!(hit_at(&menu, b.left + 1, (b.top + b.bottom) / 2), HTTRANSPARENT as isize);
}

#[test]
fn a_passive_surface_lets_the_pointer_through_its_shadow() {
    let owner = TestWindow::new();
    let menu = menu(&owner, false);
    let b = bounds(&menu);
    let inset = (16.0 * unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(menu.0) } as f32 / 96.0).round() as i32;
    assert_eq!(hit_at(&menu, (b.left + b.right) / 2, b.bottom - inset + 2), HTTRANSPARENT as isize);
}

#[test]
fn a_press_that_dismisses_a_menu_stays_from_the_form_with_its_release() {
    let owner = TestWindow::new();
    let _menu = menu(&owner, true);
    unsafe { SendMessageW(owner.0, WM_LBUTTONDOWN, None, None) };
    assert!(SWALLOWED_RELEASE.with(|swallowed| *swallowed.borrow() == Some((owner.0 .0 as isize, WM_LBUTTONUP))));
    assert!(swallowed(owner.0, WM_LBUTTONUP));
    assert!(!swallowed(owner.0, WM_LBUTTONUP));
}

#[test]
fn a_right_press_dismisses_and_still_reaches_the_form() {
    let owner = TestWindow::new();
    let _menu = menu(&owner, true);
    unsafe { SendMessageW(owner.0, WM_RBUTTONDOWN, None, None) };
    assert!(SWALLOWED_RELEASE.with(|swallowed| swallowed.borrow().is_none()));
}
