use super::*;
use std::os::windows::io::{FromRawHandle, OwnedHandle};
use windows::Win32::Foundation::WAIT_OBJECT_0;
use windows::Win32::System::Threading::{GetProcessHandleCount, GetProcessId, WaitForSingleObject};

fn executable() -> std::path::PathBuf {
    let path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("oxyde-image-worker.exe");
    assert!(
        path.is_file(),
        "Build oxyde-image-worker before running this test"
    );
    path
}
fn start() -> Session {
    Session::start(&executable(), Instant::now(), DEADLINE).unwrap()
}
fn pixels(bytes: &[u8]) -> Vec<image::Frame> {
    let Reply::Images { frames, .. } = preview_protocol::decode(bytes).unwrap() else {
        panic!("Expected pixels");
    };
    frames
}
fn source(directory: &Path, name: &str, color: [u8; 4]) -> std::fs::File {
    let path = directory.join(name);
    image::RgbaImage::from_pixel(256, 128, image::Rgba(color))
        .save(&path)
        .unwrap();
    std::fs::File::open(path).unwrap()
}

#[test]
fn persistent_image_session_reuses_pid_for_different_files_and_output_policies() {
    let directory = tempfile::tempdir().unwrap();
    let red = source(directory.path(), "red.png", [255, 0, 0, 255]);
    let blue = source(directory.path(), "blue.png", [0, 0, 255, 255]);
    let cold = Instant::now();
    let mut session = start();
    let process = HANDLE(session.worker.child.as_raw_handle());
    let pid = unsafe { GetProcessId(process) };
    let first = pixels(
        &session
            .request(&red, 0, Instant::now(), DEADLINE)
            .unwrap(),
    );
    let cold_time = cold.elapsed();
    assert_eq!(first[0].buffer().dimensions(), (256, 128));
    assert_eq!(first[0].buffer().get_pixel(128, 64).0, [0, 0, 255, 255]);
    let warm = Instant::now();
    let second = pixels(
        &session
            .request(&blue, 1, Instant::now(), DEADLINE)
            .unwrap(),
    );
    eprintln!(
        "Image session cold: {cold_time:?}; warm: {:?}",
        warm.elapsed()
    );
    assert_eq!(second[0].buffer().dimensions(), (128, 64));
    assert_eq!(second[0].buffer().get_pixel(64, 32).0, [255, 0, 0, 255]);
    assert_eq!(unsafe { GetProcessId(process) }, pid);
    assert!(session.reusable());
    // Normal decoding closes the duplicated source handles, not just the parent
    // copies: deleting both documents succeeds while the child is still alive.
    drop(red);
    drop(blue);
    std::fs::remove_file(directory.path().join("red.png")).unwrap();
    std::fs::remove_file(directory.path().join("blue.png")).unwrap();
}

#[test]
fn persistent_image_session_renders_svg_then_raster_without_stale_pixels() {
    let directory = tempfile::tempdir().unwrap();
    let svg = directory.path().join("red.svg");
    std::fs::write(&svg, br#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><rect width="200" height="100" fill="red"/></svg>"#).unwrap();
    let mut session = start();
    for thumbnail in [false, true] {
        let file = std::fs::File::open(&svg).unwrap();
        let frames = pixels(
            &session
                .request(&file, u8::from(thumbnail), Instant::now(), DEADLINE)
                .unwrap(),
        );
        let width = if thumbnail { 128 } else { 1024 };
        assert_eq!(frames[0].buffer().dimensions(), (width, width / 2));
        assert_eq!(
            frames[0].buffer().get_pixel(width / 2, width / 4).0,
            [0, 0, 255, 255]
        );
    }
    let file = source(directory.path(), "blue.png", [0, 0, 255, 255]);
    let frames = pixels(
        &session
            .request(&file, 0, Instant::now(), DEADLINE)
            .unwrap(),
    );
    assert_eq!(frames[0].buffer().get_pixel(128, 64).0, [255, 0, 0, 255]);
}

#[test]
fn persistent_image_session_closes_sources_and_recycles_after_32_requests() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.png");
    drop(source(directory.path(), "image.png", [3, 2, 1, 255]));
    let mut session = start();
    let process = HANDLE(session.worker.child.as_raw_handle());
    let mut baseline = 0;
    for index in 1..=SESSION_REQUESTS {
        let file = std::fs::File::open(&path).unwrap();
        pixels(
            &session
                .request(&file, 1, Instant::now(), DEADLINE)
                .unwrap(),
        );
        if index < SESSION_REQUESTS {
            let mut count = 0;
            unsafe { GetProcessHandleCount(process, &mut count) }.unwrap();
            if index == 1 {
                baseline = count;
            }
            assert!(
                count <= baseline + 4,
                "Handle count grew from {baseline} to {count}"
            );
        }
    }
    assert!(!session.reusable());
    let file = std::fs::File::open(path).unwrap();
    assert!(session
        .request(&file, 0, Instant::now(), DEADLINE)
        .is_err());
}

#[test]
fn persistent_image_session_marks_decoder_errors_for_recycling() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("invalid.png");
    std::fs::write(&path, b"invalid image").unwrap();
    let mut session = start();
    let reply = session
        .request(
            &std::fs::File::open(&path).unwrap(),
            0,
            Instant::now(),
            DEADLINE,
        )
        .unwrap();
    assert!(matches!(
        preview_protocol::decode(&reply).unwrap(),
        Reply::Error { .. }
    ));
    assert!(!session.reusable());
}

#[test]
fn persistent_image_session_enforces_idle_age_and_reply_deadlines() {
    let mut session = start();
    session.last_used = Instant::now() - PREVIEW.idle;
    assert!(!session.reusable());
    session.last_used = Instant::now();
    session.created = Instant::now() - PREVIEW.max_age;
    assert!(!session.reusable());
    // The real child waits for a request. A silent/blocked decoder cannot hold
    // a response read past its deadline; dropping its session then kills it.
    let mut header = [0; 12];
    let start = Instant::now();
    let error = session
        .read_exact(&mut header, start, Duration::from_millis(30))
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(start.elapsed() < Duration::from_secs(2));
    let handle = unsafe {
        std::os::windows::io::BorrowedHandle::borrow_raw(session.worker.child.as_raw_handle())
    }
    .try_clone_to_owned()
    .unwrap();
    drop(session);
    assert_eq!(
        unsafe { WaitForSingleObject(HANDLE(handle.as_raw_handle()), 1000) },
        WAIT_OBJECT_0
    );
}

#[test]
fn image_session_rejects_oversized_empty_or_mismatched_reply_headers() {
    for (id, length) in [
        (1u64, 0u32),
        (2, 10),
        (1, preview_protocol::MAX_REPLY as u32 + 1),
        (1, u32::MAX),
    ] {
        let mut header = [0; 12];
        header[..8].copy_from_slice(&id.to_le_bytes());
        header[8..].copy_from_slice(&length.to_le_bytes());
        assert!(reply_length(header, 1).is_err());
    }
}

#[test]
fn image_session_service_recovers_from_decoder_error_and_stops_without_idle_wait() {
    let directory = tempfile::tempdir().unwrap();
    let invalid = directory.path().join("invalid.png");
    std::fs::write(&invalid, b"invalid image").unwrap();
    drop(source(directory.path(), "valid.png", [255, 0, 0, 255]));
    let (sender, receiver) = mpsc::sync_channel(1);
    let (done, completion) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        serve(receiver, &executable());
        done.send(()).unwrap();
    });
    for (name, valid) in [
        ("invalid.png", false),
        ("valid.png", true),
        ("valid.png", true),
    ] {
        let (reply, response) = mpsc::channel();
        let source = crate::platform::platform()
            .retain_archive_source(&directory.path().join(name))
            .unwrap();
        sender
            .send(Message::Request(Request {
                source,
                kind: 1,
                reply,
            }))
            .unwrap();
        let result = response
            .recv_timeout(Duration::from_secs(20))
            .unwrap()
            .unwrap();
        assert_eq!(
            matches!(
                preview_protocol::decode(&result).unwrap(),
                Reply::Images { .. }
            ),
            valid
        );
    }
    sender.send(Message::Stop).unwrap();
    completion.recv_timeout(Duration::from_secs(2)).unwrap();
    thread.join().unwrap();
}

#[test]
fn persistent_image_session_grants_only_read_access_to_the_selected_source() {
    let directory = tempfile::tempdir().unwrap();
    drop(source(directory.path(), "image.png", [3, 2, 1, 255]));
    let path = directory.path().join("image.png");
    let original = std::fs::read(&path).unwrap();
    let selected = crate::platform::platform()
        .retain_archive_source(&path)
        .unwrap();
    let session = start();
    let remote = session.grant_source(&selected.file).unwrap();
    // Inspect the exact handle granted into the live decoder. SAME_ACCESS must
    // not turn the parent's read-only file into a write-capable authority.
    let mut returned = HANDLE::default();
    unsafe {
        DuplicateHandle(
            HANDLE(session.worker.child.as_raw_handle()),
            remote,
            GetCurrentProcess(),
            &mut returned,
            0,
            false,
            DUPLICATE_SAME_ACCESS,
        )
    }
    .unwrap();
    let mut file = std::fs::File::from(unsafe { OwnedHandle::from_raw_handle(returned.0) });
    let mut magic = [0; 8];
    file.read_exact(&mut magic).unwrap();
    assert_eq!(&magic, &original[..8]);
    assert_eq!(
        file.write_all(b"modified").unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn persistent_image_session_rejects_a_crashed_child() {
    let directory = tempfile::tempdir().unwrap();
    let file = source(directory.path(), "image.png", [3, 2, 1, 255]);
    let mut session = start();
    let process = HANDLE(session.worker.child.as_raw_handle());
    unsafe { windows::Win32::System::Threading::TerminateProcess(process, 1) }.unwrap();
    assert_eq!(unsafe { WaitForSingleObject(process, 1000) }, WAIT_OBJECT_0);
    assert!(!session.reusable());
    assert!(session.request(&file, 0, Instant::now(), DEADLINE).is_err());
}

#[test]
fn viewer_session_keeps_full_resolution_and_is_reused() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("large.png");
    image::RgbaImage::from_pixel(3000, 2000, image::Rgba([200, 30, 10, 255])).save(&path).unwrap();
    let started = Instant::now();
    let mut session = Session::start_with(&executable(), started, VIEWER).unwrap();
    let startup = started.elapsed();
    let mut decode = Vec::new();
    for _ in 0..2 {
        let file = std::fs::File::open(&path).unwrap();
        let request = Instant::now();
        let reply = session.request(&file, VIEWER_KIND, request, VIEWER.deadline).unwrap();
        decode.push(request.elapsed());
        let Reply::Images { frames, .. } = preview_protocol::decode_with(&reply, preview_protocol::VIEWER).unwrap() else {
            panic!("Expected viewer pixels");
        };
        assert_eq!(frames[0].buffer().dimensions(), (3000, 2000));
    }
    eprintln!("viewer session startup {startup:?}, requests {decode:?}");
    assert_eq!(session.requests, 2, "Both requests must reuse one process");
}
