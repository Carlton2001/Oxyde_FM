use super::*;

#[test]
fn video_surface_routes_left_clicks_across_threads_and_consumes_right_clicks() {
    let owner = Surface(unsafe {
        CreateWindowExW(
            WS_EX_NOACTIVATE, w!("STATIC"), w!("Oxyde pointer test"), WS_OVERLAPPED,
            40, 60, 640, 480, None, None, None, None,
        ).unwrap()
    });
    let owner_id = owner.0.0 as usize;
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let worker = std::thread::spawn(move || {
        let surface = create_surface(HWND(owner_id as *mut _)).unwrap();
        unsafe {
            SetWindowPos(surface.0, None, 90, 120, 320, 240, SWP_NOACTIVATE | SWP_NOZORDER).unwrap();
        }
        ready_tx.send(surface.0.0 as usize).unwrap();
        while !worker_stop.load(Ordering::Acquire) {
            let mut message = MSG::default();
            unsafe {
                while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                    DispatchMessageW(&message);
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    let surface = HWND(ready_rx.recv_timeout(Duration::from_secs(3)).unwrap() as *mut _);
    let result = std::panic::catch_unwind(|| unsafe {
        assert!(windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(surface).as_bool());
        assert_ne!(GetWindowLongPtrW(surface, GWL_EXSTYLE) as u32 & WS_EX_NOACTIVATE.0, 0);
        assert_eq!(SendMessageW(surface, WM_MOUSEACTIVATE, None, None).0, MA_NOACTIVATE as isize);
        let mut expected = POINT { x: 12, y: 18 };
        assert!(ClientToScreen(surface, &mut expected).as_bool());
        assert!(windows::Win32::Graphics::Gdi::ScreenToClient(owner.0, &mut expected).as_bool());
        let foreground = GetForegroundWindow();
        // GPUI counts double clicks from successive downs, not WM_*DBLCLK.
        for message in [WM_MOUSEMOVE, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE] {
            SendMessageW(surface, message, Some(WPARAM(1)), Some(LPARAM(12 | (18 << 16))));
            let mut posted = MSG::default();
            assert!(PeekMessageW(&mut posted, Some(owner.0), message, message, PM_REMOVE).as_bool());
            assert_eq!(posted.lParam.0 as i16 as i32, expected.x);
            assert_eq!((posted.lParam.0 >> 16) as i16 as i32, expected.y);
        }
        for message in [WM_RBUTTONDOWN, WM_RBUTTONUP, WM_RBUTTONDBLCLK, WM_CONTEXTMENU] {
            assert_eq!(SendMessageW(surface, message, None, None).0, 0);
            let mut posted = MSG::default();
            assert!(!PeekMessageW(&mut posted, Some(owner.0), message, message, PM_REMOVE).as_bool());
        }
        assert_eq!(GetForegroundWindow(), foreground);
    });
    stop.store(true, Ordering::Release);
    worker.join().unwrap();
    result.unwrap();
}
