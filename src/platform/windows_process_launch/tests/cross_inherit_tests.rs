use super::native_probe;
use super::*;
use std::io::Write;
use std::os::windows::io::FromRawHandle;
use std::os::windows::process::CommandExt;
use std::sync::{mpsc, TryLockError};
use std::time::Duration;
use windows::Win32::Foundation::{
    CompareObjectHandles, DuplicateHandle, DUPLICATE_SAME_ACCESS, ERROR_INVALID_HANDLE,
    WAIT_OBJECT_0,
};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{GetCurrentProcess, WaitForSingleObject};

fn pipe() -> (OwnedHandle, OwnedHandle) {
    let (mut read, mut write) = (HANDLE::default(), HANDLE::default());
    unsafe { CreatePipe(&mut read, &mut write, None, 0) }.unwrap();
    unsafe {
        (
            OwnedHandle::from_raw_handle(read.0),
            OwnedHandle::from_raw_handle(write.0),
        )
    }
}
fn reference(handle: &OwnedHandle) -> OwnedHandle {
    let mut duplicate = HANDLE::default();
    unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            HANDLE(handle.as_raw_handle()),
            GetCurrentProcess(),
            &mut duplicate,
            0,
            false,
            DUPLICATE_SAME_ACCESS,
        )
    }
    .unwrap();
    unsafe { OwnedHandle::from_raw_handle(duplicate.0) }
}
struct FixtureChild(Child);
impl Drop for FixtureChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        if unsafe { WaitForSingleObject(HANDLE(self.0.as_raw_handle()), 1000) } == WAIT_OBJECT_0 {
            let _ = self.0.wait();
        }
    }
}

#[test]
fn concurrent_spawn_waits_until_inheritable_pipes_close_and_cannot_keep_them_alive() {
    // Junction and executable fixtures elsewhere in the suite launch outside
    // Oxyde's lock. Isolate this exact test so those unrelated children cannot
    // retain its deliberately inheritable reader. Keep every identity/lifetime
    // assertion below and the concurrent launcher under test unchanged.
    const INNER: &str = "OXYDE_CROSS_INHERIT_TEST_INNER";
    if std::env::var_os(INNER).is_none() {
        let module = module_path!().split_once("::").unwrap().1;
        let name = format!("{module}::concurrent_spawn_waits_until_inheritable_pipes_close_and_cannot_keep_them_alive");
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", &name, "--nocapture"])
            .env(INNER, "1")
            .creation_flags(0x08000000);
        let output = super::output(command).unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("running 1 test"),
            "isolated inheritance test was not discovered: {stdout}"
        );
        assert!(
            output.status.success(),
            "isolated inheritance test failed: {stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    // Compile before opening the inheritance interval: this test's compiler is
    // deliberately outside the process-launch mechanism under test.
    let mut command = native_probe::command("hold-no-gate");
    command
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let (child_input, parent_input) = pipe();
    let (_parent_output, child_output) = pipe();
    let (_parent_error, child_error) = pipe();
    let references = [
        reference(&child_input),
        reference(&child_output),
        reference(&child_error),
    ];
    let inherited = InheritedHandles::new([child_input, child_output, child_error]).unwrap();
    let raw = inherited.raw();
    assert!(matches!(LAUNCH.try_lock(), Err(TryLockError::WouldBlock)));

    let (attempt, attempted) = mpsc::sync_channel(1);
    let (spawned, receive) = mpsc::sync_channel(1);
    let launcher = std::thread::spawn(move || {
        attempt.send(()).unwrap();
        let result = spawn(command).map(FixtureChild);
        let _ = spawned.send(result);
    });
    attempted.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(
        matches!(
            receive.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ),
        "concurrent spawn escaped the inheritance interval"
    );
    drop(inherited); // Every child endpoint closes before the common lock opens.
    let mut child = receive
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
        .unwrap();
    launcher.join().unwrap();
    assert!(
        child.0.try_wait().unwrap().is_none(),
        "fixture exited before the inheritance audit"
    );

    for (numeric, expected) in raw.into_iter().zip(references.iter()) {
        let mut duplicate = HANDLE::default();
        let result = unsafe {
            DuplicateHandle(
                HANDLE(child.0.as_raw_handle()),
                numeric,
                GetCurrentProcess(),
                &mut duplicate,
                0,
                false,
                DUPLICATE_SAME_ACCESS,
            )
        };
        match result {
            Ok(()) => {
                let duplicate = unsafe { OwnedHandle::from_raw_handle(duplicate.0) };
                assert!(
                    !unsafe {
                        CompareObjectHandles(
                            HANDLE(duplicate.as_raw_handle()),
                            HANDLE(expected.as_raw_handle()),
                        )
                    }
                    .as_bool(),
                    "the unrelated child inherited a sandbox pipe"
                );
            }
            Err(error) => assert_eq!(
                error.code(),
                windows::core::HRESULT::from_win32(ERROR_INVALID_HANDLE.0),
                "the child handle table was not accessible for inspection"
            ),
        }
    }
    drop(references); // In particular, close our non-inherited input reader clone.
    let mut input = std::fs::File::from(parent_input);
    assert_eq!(
        input.write_all(&[1]).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe,
        "unrelated process kept the sandbox input reader alive"
    );
    assert!(
        child.0.try_wait().unwrap().is_none(),
        "fixture did not survive the pipe lifetime test"
    );
    child.0.kill().unwrap();
    assert_eq!(
        unsafe { WaitForSingleObject(HANDLE(child.0.as_raw_handle()), 1000) },
        WAIT_OBJECT_0,
        "fixture termination did not finish within its cleanup bound"
    );
    child.0.wait().unwrap();
}
