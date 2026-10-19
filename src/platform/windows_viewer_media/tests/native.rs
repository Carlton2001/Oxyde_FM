use super::*;

fn pump() {
    unsafe {
        let mut message = MSG::default();
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn await_status(control: &ViewerMediaControl, expected: u8) {
    let start = Instant::now();
    while control.status() != expected && start.elapsed() < Duration::from_secs(12) {
        pump();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        control.status(),
        expected,
        "native media status after {:?}",
        start.elapsed()
    );
}

struct CancelOnDrop(Arc<ViewerMediaControl>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

fn find_surface(owner: HWND) -> HWND {
    struct Probe {
        owner: HWND,
        found: HWND,
    }
    unsafe extern "system" fn visit(
        window: HWND,
        data: windows::Win32::Foundation::LPARAM,
    ) -> windows::core::BOOL {
        let probe = unsafe { &mut *(data.0 as *mut Probe) };
        if unsafe { GetWindow(window, GW_OWNER) }.ok() == Some(probe.owner) {
            let mut title = [0; 64];
            let length = unsafe { GetWindowTextW(window, &mut title) } as usize;
            if String::from_utf16_lossy(&title[..length]) == "Oxyde media surface" {
                probe.found = window;
            }
        }
        windows::core::BOOL(1)
    }
    let mut probe = Probe {
        owner,
        found: HWND::default(),
    };
    unsafe {
        EnumWindows(
            Some(visit),
            windows::Win32::Foundation::LPARAM(&mut probe as *mut Probe as isize),
        )
        .unwrap();
    }
    assert!(!probe.found.0.is_null(), "Missing owned video surface");
    probe.found
}

fn await_placement(surface: HWND, owner: HWND, rect: [i32; 4], visible: bool) {
    let start = Instant::now();
    loop {
        pump();
        let mut origin = POINT {
            x: rect[0],
            y: rect[1],
        };
        let mut bounds = windows::Win32::Foundation::RECT::default();
        unsafe {
            assert!(ClientToScreen(owner, &mut origin).as_bool());
            GetWindowRect(surface, &mut bounds).unwrap();
            if IsWindowVisible(surface).as_bool() == visible
                && bounds.left == origin.x
                && bounds.top == origin.y
                && bounds.right - bounds.left == rect[2]
                && bounds.bottom - bounds.top == rect[3]
            {
                return;
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "Media surface did not follow its owner/rectangle"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn exercise(path: &Path, video: bool) {
    crate::platform::windows_preview_worker::initialize().unwrap();
    let surface = Surface(unsafe {
        CreateWindowExW(
            WS_EX_NOACTIVATE,
            w!("STATIC"),
            w!("Oxyde private media test"),
            WS_OVERLAPPED,
            0,
            0,
            640,
            480,
            None,
            None,
            None,
            None,
        )
        .unwrap()
    });
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("oxyde.exe");
    assert!(
        executable.is_file(),
        "Build all binaries before native worker tests"
    );
    let control = Arc::new(ViewerMediaControl::default());
    let _cancel = CancelOnDrop(control.clone());
    *control.command.lock().unwrap() = ViewerMediaCommand {
        parent: surface.0 .0 as usize as u64,
        rect: [0, 0, 320, 240],
        visible: true,
        paused: false,
    };
    let work = control.clone();
    let path = path.to_owned();
    let (sender, receiver) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        sender
            .send(supervise_in(&executable, &path, &work))
            .unwrap();
    });
    await_status(&control, MEDIA_PLAYING);
    let video_surface = find_surface(surface.0);
    unsafe {
        assert_ne!(
            GetWindowLongPtrW(video_surface, GWL_STYLE) as u32 & WS_POPUP.0,
            0
        );
        assert_ne!(
            GetWindowLongPtrW(video_surface, GWL_EXSTYLE) as u32 & WS_EX_NOACTIVATE.0,
            0
        );
        assert_eq!(
            GetWindowLongPtrW(video_surface, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST.0,
            0
        );
        assert_ne!(GetForegroundWindow(), video_surface);
        if video {
            let _ = ShowWindow(surface.0, SW_SHOWNOACTIVATE);
        }
    }
    await_placement(video_surface, surface.0, [0, 0, 320, 240], video);
    control.command.lock().unwrap().rect = [20, 20, 900, 700];
    let mut client = RECT::default();
    unsafe {
        GetClientRect(surface.0, &mut client).unwrap();
    }
    await_placement(
        video_surface,
        surface.0,
        [20, 20, client.right - 20, client.bottom - 20],
        video,
    );
    control.command.lock().unwrap().rect = [0, 0, 320, 240];
    control.command.lock().unwrap().paused = true;
    await_status(&control, MEDIA_PAUSED);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(control.status(), MEDIA_PAUSED);
    {
        let mut command = control.command.lock().unwrap();
        command.rect = [20, 20, 600, 350];
        command.visible = false;
        command.paused = false;
    }
    await_status(&control, MEDIA_PLAYING);
    control.command.lock().unwrap().visible = true;
    if video {
        unsafe {
            SetWindowPos(
                surface.0,
                None,
                60,
                80,
                640,
                480,
                SWP_NOACTIVATE | SWP_NOZORDER,
            )
            .unwrap();
        }
    }
    await_placement(video_surface, surface.0, [20, 20, 600, 350], video);
    await_status(&control, MEDIA_ENDED);
    // An ended item stays ended until the user explicitly restarts it.
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(control.status(), MEDIA_ENDED);
    control.cancel();
    let start = Instant::now();
    let result = loop {
        pump();
        match receiver.try_recv() {
            Ok(result) => break result,
            Err(std::sync::mpsc::TryRecvError::Empty)
                if start.elapsed() < Duration::from_secs(3) =>
            {
                std::thread::sleep(Duration::from_millis(10))
            }
            other => panic!("Media worker did not stop: {other:?}"),
        }
    };
    result.unwrap();
    thread.join().unwrap();
}

#[test]
#[ignore = "Requires built oxyde.exe and native Windows codecs/audio device"]
fn native_viewer_media_audio_autoplays_pauses_resumes_ends_and_cancels() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("silence.wav");
    let samples = 44_100 * 3;
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend((36u32 + samples * 2).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(44_100u32.to_le_bytes());
    bytes.extend(88_200u32.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend((samples * 2).to_le_bytes());
    bytes.resize(44 + samples as usize * 2, 0);
    std::fs::write(&path, bytes).unwrap();
    exercise(&path, false);
}

#[test]
#[ignore = "Requires built oxyde.exe and a private A/V fixture in OXYDE_VIEWER_TEST_MEDIA"]
fn native_viewer_media_video_with_audio_uses_system_codecs() {
    let path = std::env::var_os("OXYDE_VIEWER_TEST_MEDIA")
        .expect("Set OXYDE_VIEWER_TEST_MEDIA to a private A/V fixture");
    exercise(Path::new(&path), true);
}
