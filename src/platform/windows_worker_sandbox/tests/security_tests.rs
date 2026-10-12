use super::super::windows_process_launch::native_probe;
use super::*;
use std::io::{Read, Write};
use std::os::windows::io::AsRawHandle;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT};
use windows::Win32::System::Registry::*;

struct RegistryFixture {
    path: Vec<u16>,
    key: HKEY,
}
struct EnvironmentFixture(Option<std::ffi::OsString>);
impl EnvironmentFixture {
    fn new() -> Self {
        let previous = std::env::var_os("OXYDE_SANDBOX_TEST_SECRET");
        std::env::set_var("OXYDE_SANDBOX_TEST_SECRET", "inherited-parent-secret");
        Self(previous)
    }
}
impl Drop for EnvironmentFixture {
    fn drop(&mut self) {
        if let Some(previous) = self.0.take() {
            std::env::set_var("OXYDE_SANDBOX_TEST_SECRET", previous);
        } else {
            std::env::remove_var("OXYDE_SANDBOX_TEST_SECRET");
        }
    }
}
impl RegistryFixture {
    fn new() -> Self {
        let path: Vec<u16> = format!("Software\\OxydeSandboxProbe-{}", uuid::Uuid::new_v4())
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut key = HKEY::default();
        unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(path.as_ptr()),
                None,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_ALL_ACCESS,
                None,
                &mut key,
                None,
            )
        }
        .ok()
        .unwrap();
        Self { path, key }
    }
    fn name(&self) -> String {
        String::from_utf16(&self.path[..self.path.len() - 1]).unwrap()
    }
}
impl Drop for RegistryFixture {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.key);
            let _ = RegDeleteKeyW(HKEY_CURRENT_USER, PCWSTR(self.path.as_ptr()));
        }
    }
}
fn report(text: &str, name: &str) -> bool {
    let value = text
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{name}=")))
        .unwrap_or_else(|| panic!("missing {name}: probe did not complete: {text}"));
    match value {
        "true" => true,
        "false" => false,
        _ => panic!("invalid probe report"),
    }
}
fn fixtures_command(
    directory: &std::path::Path,
    registry: &RegistryFixture,
    handle: usize,
    ipv4: &std::net::TcpListener,
    ipv6: &std::net::TcpListener,
    package_fixture: &std::path::Path,
) -> Command {
    let mut command = native_probe::command("report");
    command
        .arg(directory.join("source.bin"))
        .arg(directory.join("destination.bin"))
        .arg(directory.join("foreign-create.bin"))
        .arg(registry.name())
        .arg(std::process::id().to_string())
        .arg(handle.to_string())
        .arg(ipv4.local_addr().unwrap().to_string())
        .arg(ipv6.local_addr().unwrap().to_string())
        .arg(package_fixture)
        .env("OXYDE_SANDBOX_TEST_SECRET", "parent-secret");
    command.current_dir(directory);
    command
}

#[test]
fn zero_capability_lpac_denies_private_resources_and_preserves_ipc() {
    let _environment = EnvironmentFixture::new();
    let directory = tempfile::tempdir().unwrap();
    let package_fixture = tempfile::tempdir().unwrap();
    std::fs::write(package_fixture.path().join("aap.bin"), b"AAP only").unwrap();
    std::fs::write(
        package_fixture.path().join("aar.bin"),
        b"traversal and package witness",
    )
    .unwrap();
    set_fixture_acl(
        package_fixture.path(),
        "(A;OICI;GRGX;;;S-1-15-2-1)(A;OICI;GRGX;;;S-1-15-2-2)",
    );
    set_fixture_acl(
        &package_fixture.path().join("aap.bin"),
        "(A;;GR;;;S-1-15-2-1)",
    );
    set_fixture_acl(
        &package_fixture.path().join("aar.bin"),
        "(A;;GR;;;S-1-15-2-2)",
    );
    std::fs::write(directory.path().join("source.bin"), b"source-secret").unwrap();
    std::fs::write(
        directory.path().join("destination.bin"),
        b"destination-secret",
    )
    .unwrap();
    let registry = RegistryFixture::new();
    let ipv4 = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let ipv6 = std::net::TcpListener::bind("[::1]:0")
        .expect("IPv6 listener unavailable: coverage cannot be claimed");
    // This live handle is deliberately inheritable but absent from the launcher's list.
    let foreign = std::fs::File::open(directory.path().join("source.bin")).unwrap();
    unsafe {
        SetHandleInformation(
            HANDLE(foreign.as_raw_handle()),
            HANDLE_FLAG_INHERIT.0,
            HANDLE_FLAG_INHERIT,
        )
    }
    .unwrap();
    let handle = foreign.as_raw_handle() as usize;
    let make_command = || {
        fixtures_command(
            directory.path(),
            &registry,
            handle,
            &ipv4,
            &ipv6,
            package_fixture.path(),
        )
    };

    // Identical native operations outside the sandbox are witnesses that the
    // private fixtures, registry and both live endpoints are accessible.
    let mut baseline = make_command()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    baseline.stdin.take().unwrap().write_all(&[0xA7]).unwrap();
    let baseline = baseline.wait_with_output().unwrap();
    assert!(baseline.status.success());
    let baseline = String::from_utf8(baseline.stdout).unwrap();
    for name in [
        "started",
        "read",
        "write",
        "create",
        "registry",
        "parent",
        "secret",
        "ipv4",
        "ipv6",
        "child",
        "runtime_read",
        "runtime_dac",
        "runtime_owner",
        "runtime_create",
        "cwd_create",
        "aap_read",
        "aar_read",
        "network_initialized",
    ] {
        assert!(report(&baseline, name), "positive control {name} failed");
    }
    assert!(!report(&baseline, "appcontainer"));
    std::fs::remove_file(directory.path().join("foreign-create.bin")).unwrap();

    let text = sandbox_report(make_command(), true);
    let normal = sandbox_report(make_command(), false);
    assert!(
        report(&normal, "started")
            && report(&normal, "appcontainer")
            && report(&normal, "zero_capabilities")
            && report(&normal, "aap_read"),
        "ordinary AppContainer witness failed: {normal}"
    );
    eprintln!(
        "LPAC token query supported: {}",
        report(&text, "lpac_query_supported")
    );
    if report(&text, "lpac_query_supported") {
        assert!(report(&text, "lpac_query_value"));
    }
    for name in [
        "started",
        "appcontainer",
        "zero_capabilities",
        "runtime_read",
        "aar_read",
    ] {
        assert!(report(&text, name), "required {name}: {text}");
    }
    if report(&text, "network_initialized") {
        assert!(
            !report(&text, "ipv4") && !report(&text, "ipv6"),
            "sandbox permitted a controlled TCP connection: {text}"
        );
        eprintln!(
            "LPAC TCP connection probes executed; controlled IPv4/IPv6 endpoints inaccessible"
        );
    } else {
        let code: i32 = text
            .lines()
            .find_map(|line| line.strip_prefix("network_startup_error="))
            .expect("missing Winsock diagnostic")
            .parse()
            .unwrap();
        let diagnostic = match code {
            10013 => "Winsock startup denied (WSAEACCES)",
            10091 => "network subsystem unavailable (WSASYSNOTREADY)",
            10107 => {
                "underlying system call failed (WSASYSCALLFAILURE); no permission-denial conclusion"
            }
            _ => panic!("unexpected Winsock startup failure {code}: {text}"),
        };
        assert!(
            text.lines().any(|line| line == "ipv4=not_exercised")
                && text.lines().any(|line| line == "ipv6=not_exercised")
        );
        eprintln!("LPAC network validation unavailable: {diagnostic}, code={code}; IPv4/IPv6 connection probes were not executed");
        eprintln!(
            "Ordinary AppContainer Winsock startup: {}",
            normal
                .lines()
                .find(|line| line.starts_with("network_startup_error="))
                .unwrap()
        );
    }
    for name in [
        "read",
        "write",
        "create",
        "registry",
        "parent",
        "inherited",
        "secret",
        "child",
        "runtime_write",
        "runtime_dac",
        "runtime_owner",
        "runtime_create",
        "cwd_create",
        "aap_read",
    ] {
        assert!(!report(&text, name), "sandbox allowed {name}: {text}");
    }
    assert_eq!(
        std::fs::read(directory.path().join("destination.bin")).unwrap(),
        b"destination-secret"
    );
    assert!(!directory.path().join("foreign-create.bin").exists());
}

fn set_fixture_acl(path: &std::path::Path, package_entries: &str) {
    let sddl = wide(format!(
        "D:P(A;OICI;FA;;;{})(A;OICI;FA;;;SY){package_entries}",
        parent_user_sid().unwrap()
    ));
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl.as_ptr()),
            1,
            &mut descriptor,
            None,
        )
    }
    .unwrap();
    let path = wide(path);
    let result = unsafe {
        SetFileSecurityW(
            PCWSTR(path.as_ptr()),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        )
    }
    .ok();
    unsafe {
        LocalFree(Some(HLOCAL(descriptor.0)));
    }
    result.unwrap();
}

#[test]
fn missing_runtime_is_refused_before_any_child_can_report_success() {
    let mut command = Command::new("C:/missing-oxyde-sandbox-probe.exe");
    command.arg("report");
    assert!(Worker::start_duplex(command, 1024 * 1024 * 1024).is_err());
}

#[test]
fn cancellation_during_runtime_preparation_is_preserved_without_launching() {
    let calls = std::cell::Cell::new(0);
    let result =
        Worker::start_duplex_checked(native_probe::command("leaf"), 1024 * 1024 * 1024, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == 4 {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "cancel runtime preparation",
                ))
            } else {
                Ok(())
            }
        });
    assert_eq!(
        result
            .err()
            .expect("cancelled preparation launched a worker")
            .kind(),
        std::io::ErrorKind::Interrupted
    );
    assert_eq!(calls.get(), 4);
}

#[test]
fn runtime_cleanup_preserves_a_foreign_child_instead_of_recursing() {
    let mut profile = Profile::create().unwrap();
    let runtime = Runtime::create(&native_probe::executable(), profile.sid, &|| Ok(())).unwrap();
    profile.remove_before_execution().unwrap();
    let root = runtime.root.clone();
    let foreign = root.join("foreign-sentinel.bin");
    std::fs::write(&foreign, b"must survive cleanup").unwrap();
    drop(runtime);
    assert!(
        !root.join("worker.exe").exists(),
        "owned runtime binary was not cleaned"
    );
    assert_eq!(std::fs::read(&foreign).unwrap(), b"must survive cleanup");
    // Fixture owns these two exact objects; no recursive path-based cleanup.
    std::fs::remove_file(foreign).unwrap();
    std::fs::remove_dir(root).unwrap();
}

fn sandbox_report(mut command: Command, lpac: bool) -> String {
    let foreign = command
        .get_args()
        .nth(6)
        .unwrap()
        .to_str()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    let expected = File::open(command.get_args().nth(1).unwrap()).unwrap();
    command.arg("--hold");
    let (mut worker, mut output, mut input) = if lpac {
        Worker::start_duplex(command, 1024 * 1024 * 1024)
    } else {
        Worker::start_duplex_for_test(command, 1024 * 1024 * 1024, false)
    }
    .unwrap();
    // The native child is alive and waits on its selected IPC input. Inspect
    // its table from the parent so invalid handles cannot abort the child.
    let mut duplicate = HANDLE::default();
    let duplication = unsafe {
        DuplicateHandle(
            HANDLE(worker.child.as_raw_handle()),
            HANDLE(foreign as *mut _),
            GetCurrentProcess(),
            &mut duplicate,
            0,
            false,
            DUPLICATE_SAME_ACCESS,
        )
    };
    let inherited = match duplication {
        Ok(()) => {
            let duplicate = owned(duplicate);
            let mut observed =
                windows::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION::default();
            let mut known =
                windows::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION::default();
            let query = windows::Win32::Storage::FileSystem::GetFileInformationByHandle;
            unsafe { query(HANDLE(expected.as_raw_handle()), &mut known) }.unwrap();
            (unsafe {
                windows::Win32::Storage::FileSystem::GetFileType(HANDLE(duplicate.as_raw_handle()))
            }) == windows::Win32::Storage::FileSystem::FILE_TYPE_DISK
                && unsafe { query(HANDLE(duplicate.as_raw_handle()), &mut observed) }.is_ok()
                && observed.dwVolumeSerialNumber == known.dwVolumeSerialNumber
                && observed.nFileIndexHigh == known.nFileIndexHigh
                && observed.nFileIndexLow == known.nFileIndexLow
        }
        Err(error) => {
            assert_eq!(
                error.code(),
                windows::core::HRESULT::from_win32(ERROR_INVALID_HANDLE.0),
                "child handle table was not accessible for the inheritance test"
            );
            false
        }
    };
    input.write_all(&[0xB8]).unwrap();
    drop(input);
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let result = output.read_to_string(&mut text).map(|_| text);
        let _ = send.send(result);
    });
    let text = receive
        .recv_timeout(Duration::from_secs(5))
        .expect("probe did not complete its native report")
        .unwrap();
    reader.join().unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = worker.child.try_wait().unwrap() {
            assert!(status.success(), "probe crash {status:?}: {text}");
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "probe did not exit"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    format!("{text}inherited={inherited}\n")
}
