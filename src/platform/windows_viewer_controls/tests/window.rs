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
    TestWindow(unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
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
    let controls = create();
    let foreground = unsafe { GetForegroundWindow() };
    place(raw(controls.0), raw(owner.0), [20, 30, 500, 57], false).unwrap();
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
