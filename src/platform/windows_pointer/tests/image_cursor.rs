use super::*;
use windows::{core::w, Win32::UI::WindowsAndMessaging::*};

#[test]
fn lucide_cursors_load_and_hand_survives_captured_mouse_movement() {
    unsafe {
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0), w!("STATIC"), w!("Oxyde cursor test"), WS_OVERLAPPED,
            0, 0, 100, 100, None, None, None, None,
        ).unwrap();
        let previous = GetCursor();
        let result = std::panic::catch_unwind(|| {
            let search = image_cursor(hwnd, false);
            let hand = image_cursor(hwnd, true);
            assert!(!search.0.is_null(), "Lucide search resource must load");
            assert!(!hand.0.is_null(), "Lucide hand-grab resource must load");
            assert_ne!(search, hand);
            set_image_cursor(hwnd, true, false);
            assert_eq!(GetCursor(), search);
            set_image_cursor(hwnd, true, true);
            assert_eq!(GetCursor(), hand, "drag takes priority over the zoom timer");
            SetCursor(Some(search));
            SendMessageW(hwnd, WM_MOUSEMOVE, None, None);
            assert_eq!(GetCursor(), hand, "mouse movement must retain the grab cursor");
            set_image_cursor(hwnd, false, false);
            assert_eq!(GetCursor(), LoadCursorW(None, IDC_ARROW).unwrap());
        });
        SetCursor(Some(previous));
        DestroyWindow(hwnd).unwrap();
        result.unwrap();
    }
}
