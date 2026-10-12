use windows::core::w;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GetWindow, GetWindowLongPtrW, GWL_EXSTYLE, GWL_STYLE, GW_OWNER,
    WINDOW_EX_STYLE, WS_DISABLED, WS_EX_TOPMOST, WS_OVERLAPPED,
};

struct TestWindow(HWND);
impl TestWindow {
    fn new() -> Self {
        Self(unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("Oxyde ownership test"),
                WS_OVERLAPPED,
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
}
impl Drop for TestWindow {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

#[test]
fn multiple_properties_have_the_same_enabled_owner_without_global_topmost() {
    let owner = TestWindow::new();
    let first = TestWindow::new();
    let second = TestWindow::new();
    for window in [&first, &second] {
        super::set_owner(window.0, owner.0).unwrap();
        assert_eq!(unsafe { GetWindow(window.0, GW_OWNER) }.unwrap(), owner.0);
        assert_eq!(
            unsafe { GetWindowLongPtrW(window.0, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST.0,
            0
        );
    }
    assert_eq!(
        unsafe { GetWindowLongPtrW(owner.0, GWL_STYLE) } as u32 & WS_DISABLED.0,
        0
    );
}
