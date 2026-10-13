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

#[test]
fn hover_is_passive_owned_mouse_transparent_and_not_globally_topmost() {
    let owner = TestWindow::new();
    let hover = TestWindow::new();
    super::super::windows_window_owner::set_owner(hover.0, owner.0).unwrap();
    configure(hover.raw()).unwrap();
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
        place(hover.raw(), Some((120.0, 80.0)), 1.0).unwrap();
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
        assert!(!IsWindowVisible(hover.0).as_bool());
    }
}
