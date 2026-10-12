use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::GetCapture;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, SendMessageW, WINDOW_EX_STYLE, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WS_OVERLAPPED,
};

struct TestWindow(HWND);
impl Drop for TestWindow {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

#[test]
fn native_capture_is_released_when_button_up_is_outside_client_area() {
    let window = TestWindow(unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("Oxyde capture test"),
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
    });
    super::install(window.0).unwrap();
    unsafe {
        SendMessageW(window.0, WM_LBUTTONDOWN, Some(WPARAM(1)), Some(LPARAM(0)));
    }
    assert_eq!(unsafe { GetCapture() }, window.0);
    // Signed coordinates (-100, -100), beyond every client edge.
    let outside = (((-100i16 as u16 as u32) << 16) | (-100i16 as u16 as u32)) as isize;
    unsafe {
        SendMessageW(
            window.0,
            WM_LBUTTONUP,
            Some(WPARAM(0)),
            Some(LPARAM(outside)),
        );
    }
    assert_ne!(unsafe { GetCapture() }, window.0);
}
