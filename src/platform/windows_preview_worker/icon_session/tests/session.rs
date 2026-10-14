use super::*;

#[test]
fn framing_rejects_wrong_request_ids_and_oversized_replies() {
    let header = |id: u64, length: u32| {
        let mut bytes = [0; 12];
        bytes[..8].copy_from_slice(&id.to_le_bytes());
        bytes[8..].copy_from_slice(&length.to_le_bytes());
        bytes
    };
    assert_eq!(reply_length(header(1, 100), 1).unwrap(), 100);
    assert!(reply_length(header(2, 100), 1).is_err());
    assert!(reply_length(header(1, 0), 1).is_err());
    assert!(reply_length(header(1, MAX_REPLY as u32 + 1), 1).is_err());
}

#[test]
#[ignore = "requires OXYDE_TEST_ICON_WORKER pointing to the freshly built oxyde executable"]
fn native_session_reuses_one_process_for_multiple_icons_and_sizes() {
    let executable =
        std::env::var_os("OXYDE_TEST_ICON_WORKER").expect("set OXYDE_TEST_ICON_WORKER");
    let mut session = Session::start_in(Path::new(&executable), Instant::now()).unwrap();
    let pid = session.worker.child.id();
    let cold = Instant::now();
    for (path, large) in [
        ("oxyde_dir_generic", false),
        ("oxyde_dir_generic", true),
        ("oxyde_ext_txt", false),
        ("oxyde_file_generic", false),
    ] {
        let started = Instant::now();
        let reply = session.request(path, large, Instant::now()).unwrap();
        eprintln!(
            "Icon request {path}, large={large}: {:?}",
            started.elapsed()
        );
        assert!(!reply.is_empty());
        assert_eq!(session.worker.child.id(), pid);
        assert!(session.worker.child.try_wait().unwrap().is_none());
    }
    assert_eq!(session.requests, 4);
    eprintln!("Four icons in one process: {:?}", cold.elapsed());
    assert!(session.reusable(100));
    assert!(!session.reusable(MAX_INPUT));
    assert!(session
        .request("x", false, Instant::now() - DEADLINE)
        .is_err());
    session.last_used = Instant::now() - IDLE;
    assert!(!session.reusable(0));
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use windows::Win32::Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{GetCurrentProcess, WaitForSingleObject};
    let mut handle = HANDLE::default();
    unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            HANDLE(session.worker.child.as_raw_handle()),
            GetCurrentProcess(),
            &mut handle,
            0,
            false,
            DUPLICATE_SAME_ACCESS,
        )
    }
    .unwrap();
    let retained = unsafe { OwnedHandle::from_raw_handle(handle.0) };
    drop(session);
    assert_eq!(
        unsafe { WaitForSingleObject(HANDLE(retained.as_raw_handle()), 2000) },
        WAIT_OBJECT_0
    );
}
