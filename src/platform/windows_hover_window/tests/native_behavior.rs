use super::*;
use raw_window_handle::Win32WindowHandle;
use std::num::NonZeroIsize;
use windows::core::w;

struct TestWindow(HWND);
impl TestWindow {
    fn new() -> Self {
        Self(unsafe {
            CreateWindowExW(
                WS_EX_APPWINDOW | WS_EX_TOPMOST,
                w!("STATIC"),
                w!("Oxyde hover test"),
                WS_POPUP,
                0,
                0,
                100,
                100,
                None,
                None,
                None,
                None,
            )
            .unwrap()
        })
    }
    fn raw(&self) -> RawWindowHandle {
        RawWindowHandle::Win32(Win32WindowHandle::new(
            NonZeroIsize::new(self.0 .0 as isize).unwrap(),
        ))
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
        while PeekMessageW(
            &mut message,
            Some(window),
            APPLY_PLACEMENT,
            INITIALIZE_WINDOW,
            PM_REMOVE,
        )
        .as_bool()
        {
            DispatchMessageW(&message);
        }
    }
}

#[test]
fn hover_is_passive_owned_mouse_transparent_and_not_globally_topmost() {
    let owner = TestWindow::new();
    let hover = TestWindow::new();
    super::super::windows_window_owner::set_owner(hover.0, owner.0).unwrap();
    configure(hover.raw()).unwrap();
    apply_pending(hover.0);
    unsafe {
        let style = GetWindowLongPtrW(hover.0, GWL_EXSTYLE) as u32;
        assert_ne!(style & WS_EX_TOOLWINDOW.0, 0);
        assert_ne!(style & WS_EX_NOACTIVATE.0, 0);
        assert_eq!(style & (WS_EX_APPWINDOW.0 | WS_EX_TOPMOST.0), 0);
        assert_eq!(GetWindow(hover.0, GW_OWNER).unwrap(), owner.0);
        assert_eq!(
            SendMessageW(hover.0, WM_NCHITTEST, None, None).0,
            HTTRANSPARENT as isize
        );
        assert_eq!(
            SendMessageW(hover.0, WM_MOUSEACTIVATE, None, None).0,
            MA_NOACTIVATE as isize
        );
        let foreground = GetForegroundWindow();
        place(hover.raw(), Some((120.0, 80.0, 0.0)), 1.0).unwrap();
        apply_pending(hover.0);
        assert!(IsWindowVisible(hover.0).as_bool());
        assert_eq!(GetForegroundWindow(), foreground);
        let mut bounds = windows::Win32::Foundation::RECT::default();
        GetWindowRect(hover.0, &mut bounds).unwrap();
        let mut cursor = POINT::default();
        GetCursorPos(&mut cursor).unwrap();
        assert!(
            !(cursor.x >= bounds.left
                && cursor.x < bounds.right
                && cursor.y >= bounds.top
                && cursor.y < bounds.bottom)
        );
        place(hover.raw(), None, 1.0).unwrap();
        apply_pending(hover.0);
        assert!(!IsWindowVisible(hover.0).as_bool());
    }
}

#[test]
fn placements_are_deferred_coalesced_and_cancelled_by_hide() {
    let hover = TestWindow::new();
    configure(hover.raw()).unwrap();
    place(hover.raw(), Some((120.0, 80.0, 0.0)), 1.0).unwrap();
    place(hover.raw(), Some((180.0, 100.0, 0.0)), 1.0).unwrap();
    assert!(!unsafe { IsWindowVisible(hover.0) }.as_bool());
    PENDING.with(|pending| {
        assert_eq!(
            pending.borrow().get(&(hover.0 .0 as isize)).unwrap().size,
            Some((180.0, 100.0, 0.0))
        )
    });
    apply_pending(hover.0);
    let mut bounds = windows::Win32::Foundation::RECT::default();
    unsafe { GetWindowRect(hover.0, &mut bounds) }.unwrap();
    assert_eq!(
        (bounds.right - bounds.left, bounds.bottom - bounds.top),
        (180, 100)
    );
    place(hover.raw(), Some((200.0, 120.0, 0.0)), 1.0).unwrap();
    place(hover.raw(), None, 1.0).unwrap();
    apply_pending(hover.0);
    assert!(!unsafe { IsWindowVisible(hover.0) }.as_bool());
    PENDING.with(|pending| assert!(!pending.borrow().contains_key(&(hover.0 .0 as isize))));
}

#[test]
fn visible_frame_stays_12_pixels_from_cursor_excluding_shadow_margin() {
    for scale in [1, 2] {
        let cursor = 500 * scale;
        let extent = 300 * scale;
        let inset = 32 * scale;
        let gap = 12 * scale;
        let origin = axis_origin(cursor, extent, 0, 2000 * scale, gap, inset);
        assert_eq!(origin + inset - cursor, gap);
        let cursor = 1900 * scale;
        let origin = axis_origin(cursor, extent, 0, 2000 * scale, gap, inset);
        assert_eq!(cursor - (origin + extent - inset), gap);
    }
    let origin = axis_origin(-1000, 300, -1920, 0, 12, 32);
    assert_eq!(origin + 32 - (-1000), 12);
    assert_eq!(axis_origin(0, 300, 0, 800, 12, 32), 0);
}

#[test]
fn interactive_menu_receives_mouse_and_submenus_share_the_same_family() {
    let owner = TestWindow::new();
    let menu = TestWindow::new();
    let child = TestWindow::new();
    super::super::windows_window_owner::set_owner(menu.0, owner.0).unwrap();
    super::super::windows_window_owner::set_owner(child.0, menu.0).unwrap();
    configure_menu(menu.raw(), owner.raw(), owner.raw(), true).unwrap();
    configure_menu(child.raw(), menu.raw(), owner.raw(), true).unwrap();
    apply_pending(menu.0);
    apply_pending(child.0);
    assert_eq!(unsafe { root_owner(child.0) }, owner.0);
    let style = unsafe { GetWindowLongPtrW(menu.0, GWL_EXSTYLE) } as u32;
    assert_eq!(style & (WS_EX_NOACTIVATE.0 | WS_EX_TOPMOST.0), 0);
    assert_ne!(
        unsafe { SendMessageW(menu.0, WM_NCHITTEST, None, None) }.0,
        HTTRANSPARENT as isize
    );
    unsafe {
        SendMessageW(
            menu.0,
            WM_ACTIVATE,
            Some(WPARAM(WA_INACTIVE as usize)),
            Some(LPARAM(child.0 .0 as isize)),
        );
    }
    let mut message = MSG::default();
    assert!(
        !unsafe { PeekMessageW(&mut message, Some(menu.0), WM_CLOSE, WM_CLOSE, PM_REMOVE) }
            .as_bool()
    );
}

#[test]
fn parent_click_closes_child_menus_before_the_parent_handles_the_click() {
    let owner = TestWindow::new();
    let menu = TestWindow::new();
    super::super::windows_window_owner::set_owner(menu.0, owner.0).unwrap();
    configure_menu(menu.raw(), owner.raw(), owner.raw(), true).unwrap();
    unsafe {
        SendMessageW(owner.0, WM_LBUTTONDOWN, None, None);
    }
    assert!(!unsafe { IsWindow(Some(menu.0)) }.as_bool());
    MENUS.with(|menus| assert!(!menus.borrow().contains(&(menu.0 .0 as isize))));
    COORDINATE_OWNERS.with(|owners| assert!(!owners.borrow().contains_key(&(menu.0 .0 as isize))));
}

#[test]
fn popup_points_round_trip_through_the_coordinate_owner() {
    let owner = TestWindow::new();
    let menu = TestWindow::new();
    configure(menu.raw()).unwrap();
    let point = point_to_owner(menu.raw(), owner.raw(), (12.0, 20.0)).unwrap();
    assert_eq!(point, (12.0, 20.0));
    set_anchor(menu.raw(), Some((20.0, 30.0, 18.0, 24.0)), false, false).unwrap();
    ANCHORS.with(|anchors| {
        assert_eq!(
            anchors.borrow().get(&(menu.0 .0 as isize)).unwrap().0,
            (20.0, 30.0, 18.0, 24.0)
        )
    });
}

#[test]
fn menu_shadow_does_not_intercept_pointer_input() {
    let owner = TestWindow::new();
    let menu = TestWindow::new();
    super::super::windows_window_owner::set_owner(menu.0, owner.0).unwrap();
    configure_menu(menu.raw(), owner.raw(), owner.raw(), true).unwrap();
    apply_pending(menu.0);
    set_anchor(menu.raw(), Some((200.0, 200.0, 40.0, 20.0)), false, false).unwrap();
    let scale = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(menu.0) } as f32 / 96.0;
    place(menu.raw(), Some((200.0, 100.0, 16.0)), scale).unwrap();
    apply_pending(menu.0);
    let mut bounds = windows::Win32::Foundation::RECT::default();
    unsafe { GetWindowRect(menu.0, &mut bounds) }.unwrap();
    let hit = |offset: i32| {
        let x = bounds.left + offset;
        let y = bounds.top + offset;
        let coordinates = (x as u16 as u32) | ((y as u16 as u32) << 16);
        unsafe {
            SendMessageW(
                menu.0,
                WM_NCHITTEST,
                None,
                Some(LPARAM(coordinates as isize)),
            )
        }
        .0
    };
    assert_eq!(hit(1), HTTRANSPARENT as isize);
    assert_eq!(hit((20.0 * scale).ceil() as i32), HTCLIENT as isize);
}

#[test]
fn invalid_anchors_are_rejected_without_overwriting_the_valid_reference() {
    let menu = TestWindow::new();
    configure(menu.raw()).unwrap();
    set_anchor(menu.raw(), Some((20.0, 30.0, 18.0, 24.0)), false, false).unwrap();
    assert!(set_anchor(menu.raw(), Some((f32::NAN, 0.0, 1.0, 1.0)), false, false).is_err());
    assert!(set_anchor(menu.raw(), Some((0.0, 0.0, -1.0, 1.0)), false, false).is_err());
    ANCHORS.with(|anchors| {
        assert_eq!(
            anchors.borrow().get(&(menu.0 .0 as isize)).unwrap().0,
            (20.0, 30.0, 18.0, 24.0)
        )
    });
}

#[test]
fn popup_coordinates_account_for_different_owner_and_menu_origins() {
    let owner = TestWindow::new();
    let menu = TestWindow::new();
    unsafe {
        SetWindowPos(
            owner.0,
            None,
            200,
            200,
            100,
            100,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
        .unwrap();
        SetWindowPos(
            menu.0,
            None,
            300,
            400,
            100,
            100,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
        .unwrap();
    }
    let owner_scale = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(owner.0) } as f32 / 96.0;
    assert_eq!(
        point_to_owner(menu.raw(), owner.raw(), (0.0, 0.0)).unwrap(),
        (100.0 / owner_scale, 200.0 / owner_scale)
    );
}

#[test]
fn element_anchor_places_the_visible_frame_below_the_element() {
    let owner = TestWindow::new();
    let hover = TestWindow::new();
    super::super::windows_window_owner::set_owner(hover.0, owner.0).unwrap();
    configure(hover.raw()).unwrap();
    let scale = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(owner.0) } as f32 / 96.0;
    let monitor = unsafe {
        windows::Win32::Graphics::Gdi::MonitorFromWindow(owner.0, MONITOR_DEFAULTTONEAREST)
    };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    assert!(unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool());
    let mut reference = POINT {
        x: info.rcWork.left + (info.rcWork.right - info.rcWork.left) / 3,
        y: info.rcWork.top + (info.rcWork.bottom - info.rcWork.top) / 3,
    };
    let screen_reference = reference;
    assert!(
        unsafe { windows::Win32::Graphics::Gdi::ScreenToClient(owner.0, &mut reference) }.as_bool()
    );
    set_anchor(
        hover.raw(),
        Some((
            reference.x as f32 / scale,
            reference.y as f32 / scale,
            40.0,
            20.0,
        )),
        false,
        false,
    )
    .unwrap();
    place(hover.raw(), Some((160.0, 100.0, 16.0)), scale).unwrap();
    apply_pending(hover.0);
    let mut bounds = windows::Win32::Foundation::RECT::default();
    unsafe { GetWindowRect(hover.0, &mut bounds) }.unwrap();
    assert_eq!(
        bounds.left + (16.0 * scale).round() as i32,
        screen_reference.x
    );
    assert_eq!(
        bounds.top + (16.0 * scale).round() as i32,
        screen_reference.y + (20.0 * scale).round() as i32 + (4.0 * scale).round() as i32
    );
}

#[test]
fn completion_menu_receives_clicks_without_activating_or_stealing_focus() {
    let owner = TestWindow::new();
    let menu = TestWindow::new();
    super::super::windows_window_owner::set_owner(menu.0, owner.0).unwrap();
    configure_menu(menu.raw(), owner.raw(), owner.raw(), false).unwrap();
    apply_pending(menu.0);
    let foreground = unsafe { GetForegroundWindow() };
    assert_ne!(
        unsafe { GetWindowLongPtrW(menu.0, GWL_EXSTYLE) } as u32 & WS_EX_NOACTIVATE.0,
        0
    );
    assert_eq!(
        unsafe { SendMessageW(menu.0, WM_MOUSEACTIVATE, None, None) }.0,
        MA_NOACTIVATE as isize
    );
    assert_eq!(
        unsafe { SendMessageW(menu.0, WM_NCHITTEST, None, None) }.0,
        HTCLIENT as isize
    );
    set_anchor(menu.raw(), Some((200.0, 200.0, 40.0, 20.0)), false, false).unwrap();
    place(menu.raw(), Some((160.0, 100.0, 16.0)), 1.0).unwrap();
    apply_pending(menu.0);
    assert!(unsafe { IsWindowVisible(menu.0) }.as_bool());
    assert_eq!(unsafe { GetForegroundWindow() }, foreground);
}
