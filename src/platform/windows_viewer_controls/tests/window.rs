use super::*;
use raw_window_handle::Win32WindowHandle;
use std::num::NonZeroIsize;
use windows::core::w;

struct TestWindow(HWND);
impl Drop for TestWindow {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

fn create() -> TestWindow {
    create_with(WINDOW_EX_STYLE(0))
}

/// GPUI creates its PopUp windows as topmost tool windows.
fn create_popup() -> TestWindow {
    create_with(WS_EX_TOOLWINDOW | WS_EX_TOPMOST)
}

fn create_with(extended: WINDOW_EX_STYLE) -> TestWindow {
    TestWindow(unsafe {
        CreateWindowExW(
            extended,
            w!("STATIC"),
            w!("Oxyde controls test"),
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

fn raw(window: HWND) -> RawWindowHandle {
    RawWindowHandle::Win32(Win32WindowHandle::new(
        NonZeroIsize::new(window.0 as isize).unwrap(),
    ))
}

#[test]
fn fullscreen_controls_are_owned_nonactivating_and_never_globally_topmost() {
    let owner = create();
    let controls = create_popup();
    let foreground = unsafe { GetForegroundWindow() };
    place(raw(controls.0), raw(owner.0), [20, 30, 500, 57], false, false).unwrap();
    unsafe {
        assert_eq!(GetWindow(controls.0, GW_OWNER).unwrap(), owner.0);
        let style = GetWindowLongPtrW(controls.0, GWL_EXSTYLE) as u32;
        assert_ne!(style & WS_EX_NOACTIVATE.0, 0);
        assert_ne!(style & WS_EX_TOOLWINDOW.0, 0);
        assert_eq!(style & (WS_EX_TOPMOST.0 | WS_EX_APPWINDOW.0), 0);
        assert!(!IsWindowVisible(controls.0).as_bool());
        assert_eq!(
            SendMessageW(controls.0, WM_MOUSEACTIVATE, None, None).0,
            MA_NOACTIVATE as isize
        );
        assert_eq!(GetForegroundWindow(), foreground);
    }
}

#[test]
fn video_overlays_are_owned_by_the_surface_and_released_before_it_closes() {
    // The class is process-wide: register it with the production procedure, or a
    // stub registered first would answer the media surface tests instead.
    let class = crate::platform::windows_viewer_media::surface_class().unwrap();
    let owner = create();
    let surface = TestWindow(unsafe {
        CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW, class, w!("surface"), WS_POPUP,
            0, 0, 50, 50, Some(owner.0), None, None, None).unwrap()
    });
    let overlay = create();
    unsafe {
        let _ = ShowWindow(surface.0, SW_SHOWNOACTIVATE);
        place(raw(overlay.0), raw(owner.0), [0, 0, 10, 10], false, true).unwrap();
        assert_eq!(GetWindow(overlay.0, GW_OWNER).unwrap(), surface.0);
        release_overlays(raw(owner.0));
        assert_eq!(GetWindow(overlay.0, GW_OWNER).unwrap(), owner.0);
        // Without the surface flag, the UI window owns the popup again.
        place(raw(overlay.0), raw(owner.0), [0, 0, 10, 10], false, false).unwrap();
        assert_eq!(GetWindow(overlay.0, GW_OWNER).unwrap(), owner.0);
    }
}

#[test]
fn video_overlays_show_above_an_inactive_owner_without_global_topmost() {
    let owner = create();
    let overlay = create_popup();
    unsafe {
        let _ = ShowWindow(owner.0, SW_SHOWNOACTIVATE);
        // The owner is shown without activation, so it is not in the foreground.
        place(raw(overlay.0), raw(owner.0), [0, 0, 10, 10], true, true).unwrap();
        assert!(IsWindowVisible(overlay.0).as_bool());
        assert_eq!(GetWindowLongPtrW(overlay.0, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST.0, 0);
        place(raw(overlay.0), raw(owner.0), [0, 0, 10, 10], false, true).unwrap();
        assert!(!IsWindowVisible(overlay.0).as_bool());
    }
}
