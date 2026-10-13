//! Standalone native fixture, compiled by the test harness (never shipped).
use std::io::{Read, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::time::Duration;
type Handle = *mut std::ffi::c_void;
#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> Handle;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn LocalFree(memory: Handle) -> Handle;
    fn LoadLibraryW(path: *const u16) -> Handle;
    fn GetProcAddress(module: Handle, name: *const u8) -> *mut std::ffi::c_void;
    fn CreateEventW(attributes: *const std::ffi::c_void, manual: i32, initial: i32, name: *const u16) -> Handle;
}
#[link(name = "advapi32")]
extern "system" {
    fn OpenProcessToken(process: Handle, access: u32, token: *mut Handle) -> i32;
    fn GetTokenInformation(
        token: Handle,
        class: u32,
        data: *mut u32,
        size: u32,
        used: *mut u32,
    ) -> i32;
    fn RegOpenKeyExW(
        key: Handle,
        path: *const u16,
        options: u32,
        access: u32,
        result: *mut Handle,
    ) -> i32;
    fn RegCloseKey(key: Handle) -> i32;
}
#[link(name = "ws2_32")]
extern "system" {
    fn WSAStartup(version: u16, data: *mut u8) -> i32;
    fn WSACleanup() -> i32;
}
fn token_flag(class: u32) -> std::io::Result<bool> {
    unsafe {
        let mut token = std::ptr::null_mut();
        assert_ne!(OpenProcessToken(GetCurrentProcess(), 8, &mut token), 0);
        let (mut value, mut used) = ([0u32; 32], 0);
        let ok = GetTokenInformation(token, class, value.as_mut_ptr(), 128, &mut used);
        let error = std::io::Error::last_os_error();
        CloseHandle(token);
        if ok == 0 {
            Err(error)
        } else {
            Ok(value[0] != 0)
        }
    }
}
fn flag(name: &str, value: bool) {
    println!("{name}={value}");
}
fn no_capabilities() -> bool {
    unsafe {
        let mut token = std::ptr::null_mut();
        assert_ne!(OpenProcessToken(GetCurrentProcess(), 8, &mut token), 0);
        let mut used = 0;
        GetTokenInformation(token, 30, std::ptr::null_mut(), 0, &mut used);
        let mut storage = vec![0u64; (used as usize + 7) / 8];
        let ok = GetTokenInformation(token, 30, storage.as_mut_ptr().cast(), used, &mut used);
        CloseHandle(token);
        assert_ne!(ok, 0);
        *(storage.as_ptr().cast::<u32>()) == 0
    }
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args[1] == "leaf" {
        return;
    }
    if args[1] == "hold-no-gate" {
        std::thread::sleep(Duration::from_secs(10));
        return;
    }
    let mut gate = [0];
    std::io::stdin().read_exact(&mut gate).unwrap();
    assert_eq!(gate, [0xA7]);
    if args[1] == "report" && args.get(11).is_some_and(|v| v == "--hold") {
        std::io::stdin().read_exact(&mut gate).unwrap();
        assert_eq!(gate, [0xB8]);
    }
    if ["sleep", "crash", "malformed", "memory"].contains(&args[1].as_str()) {
        let ready: Vec<u8> = args[2]
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        std::io::stdout().write_all(&ready).unwrap();
        std::io::stdout().flush().unwrap();
        // Parent must observe the native readiness frame before the adverse action.
        std::thread::sleep(Duration::from_millis(100));
    }
    match args[1].as_str() {
        "dpapi" => unsafe {
            #[repr(C)] struct Blob { size: u32, data: *mut u8 }
            type Protect = unsafe extern "system" fn(*const Blob, *const u16, *const Blob, Handle, Handle, u32, *mut Blob) -> i32;
            let name: Vec<_> = "crypt32.dll".encode_utf16().chain([0]).collect();
            let library = LoadLibraryW(name.as_ptr());
            assert!(!library.is_null());
            let function = GetProcAddress(library, b"CryptProtectData\0".as_ptr());
            assert!(!function.is_null());
            let protect: Protect = std::mem::transmute(function);
            let input = Blob { size: 6, data: b"forged".as_ptr().cast_mut() };
            let mut output = Blob { size: 0, data: std::ptr::null_mut() };
            let result = protect(&input, std::ptr::null(), std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), 1, &mut output);
            let error = std::io::Error::last_os_error().raw_os_error().unwrap_or(-1);
            flag("dpapi_protect", result != 0);
            if result == 0 { println!("dpapi_error={error}"); }
            if !output.data.is_null() { LocalFree(output.data.cast()); }
        },
        "success" => { std::io::stdout().write_all(&[1]).unwrap(); }
        "source-handle" => {
            use std::os::windows::io::FromRawHandle;
            let handle = args.windows(2).find(|pair| pair[0] == "--source-handle").unwrap()[1]
                .parse::<usize>().unwrap();
            let mut file = unsafe { std::fs::File::from_raw_handle(handle as Handle) };
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap();
            flag("selected_read", bytes == b"selected witness");
            flag("selected_write", file.write_all(b"hostile").is_ok());
            flag("private_path_read", std::fs::read(&args[2]).is_ok());
        }
        "native-services" => unsafe {
            unsafe fn procedure(module: &str, name: &[u8]) -> *mut std::ffi::c_void {
                let path: Vec<_> = module.encode_utf16().chain([0]).collect();
                let library = LoadLibraryW(path.as_ptr());
                if library.is_null() { return std::ptr::null_mut(); }
                GetProcAddress(library, name.as_ptr())
            }
            let co = procedure("ole32.dll", b"CoInitializeEx\0");
            if !co.is_null() {
                let call: unsafe extern "system" fn(Handle, u32) -> i32 = std::mem::transmute(co);
                println!("com_initialize={:08X}", call(std::ptr::null_mut(), 0) as u32);
            } else { println!("com_load_error={}", std::io::Error::last_os_error()); }
            let mf = procedure("mfplat.dll", b"MFStartup\0");
            if !mf.is_null() {
                let call: unsafe extern "system" fn(u32, u32) -> i32 = std::mem::transmute(mf);
                println!("media_initialize={:08X}", call(0x20070, 0) as u32);
            } else { println!("media_load_error={}", std::io::Error::last_os_error()); }
            let create = procedure("combase.dll", b"WindowsCreateString\0");
            let activate = procedure("combase.dll", b"RoGetActivationFactory\0");
            if !create.is_null() && !activate.is_null() {
                let create: unsafe extern "system" fn(*const u16, u32, *mut Handle) -> i32 = std::mem::transmute(create);
                let activate: unsafe extern "system" fn(Handle, *const u32, *mut Handle) -> i32 = std::mem::transmute(activate);
                let unknown: [u32; 4] = [0, 0, 0x000000C0, 0x46000000];
                for class in ["Windows.Data.Pdf.PdfDocument", "Windows.Storage.StorageFile"] {
                    let units: Vec<_> = class.encode_utf16().collect();
                    let mut name = std::ptr::null_mut();
                    assert!(create(units.as_ptr(), units.len() as u32, &mut name) >= 0);
                    let mut factory = std::ptr::null_mut();
                    println!("{class}={:08X}", activate(name, unknown.as_ptr(), &mut factory) as u32);
                }
            } else { println!("winrt_load_error={}", std::io::Error::last_os_error()); }
        },
        "sleep" => std::thread::sleep(Duration::from_secs(10)),
        "crash" => std::process::exit(9),
        "malformed" => {
            std::io::stdout().write_all(&[0]).unwrap();
        }
        "memory" => {
            let mut bytes = vec![0u8; 256 * 1024 * 1024];
            for offset in (0..bytes.len()).step_by(4096) {
                bytes[offset] = 1;
            }
            std::hint::black_box(&bytes);
            std::thread::sleep(Duration::from_secs(10));
        }
        "memory-bytes" => {
            let count: usize = args[2].parse().unwrap();
            let mut bytes = Vec::new();
            let permitted = bytes.try_reserve_exact(count).is_ok();
            if permitted { bytes.resize(count, 0u8); }
            flag("allocation", permitted);
            std::io::stdout().flush().unwrap();
            std::io::stdin().read_exact(&mut gate).unwrap();
            std::hint::black_box(&bytes);
        }
        "handles" => {
            let mut handles = Vec::new();
            for _ in 0..5000 {
                let handle = unsafe { CreateEventW(std::ptr::null(), 0, 0, std::ptr::null()) };
                assert!(!handle.is_null());
                handles.push(handle);
            }
            flag("handles_ready", true);
            std::io::stdout().flush().unwrap();
            std::thread::sleep(Duration::from_secs(10));
            for handle in handles { unsafe { CloseHandle(handle); } }
        }
        "frames" => {
            let bytes: Vec<u8> = args[2]
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            std::io::stdout().write_all(&bytes).unwrap();
            std::io::stdout().flush().unwrap();
            std::thread::sleep(Duration::from_secs(10));
        }
        "report" => {
            flag("started", true);
            let appcontainer = token_flag(29).unwrap();
            flag("appcontainer", appcontainer);
            if appcontainer {
                match token_flag(46) {
                    Ok(value) => {
                        flag("lpac_query_supported", true);
                        flag("lpac_query_value", value);
                    }
                    Err(error) if error.raw_os_error() == Some(87) => {
                        flag("lpac_query_supported", false)
                    }
                    Err(error) => panic!("unexpected LPAC query failure: {error}"),
                }
            } else {
                flag("lpac_query_supported", false);
            }
            flag("zero_capabilities", appcontainer && no_capabilities());
            flag(
                "aap_read",
                std::fs::read(std::path::Path::new(&args[10]).join("aap.bin")).is_ok(),
            );
            flag(
                "aar_read",
                std::fs::read(std::path::Path::new(&args[10]).join("aar.bin")).is_ok(),
            );
            let runtime = std::env::current_exe().unwrap();
            flag("runtime_read", std::fs::read(&runtime).is_ok());
            flag(
                "runtime_write",
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(&runtime)
                    .is_ok(),
            );
            for (name, rights) in [("runtime_dac", 0x40000), ("runtime_owner", 0x80000)] {
                flag(
                    name,
                    std::fs::OpenOptions::new()
                        .access_mode(rights)
                        .share_mode(7)
                        .open(&runtime)
                        .is_ok(),
                );
            }
            flag(
                "runtime_create",
                std::fs::write(
                    runtime.parent().unwrap().join("runtime-write.bin"),
                    b"probe",
                )
                .is_ok(),
            );
            flag(
                "cwd_create",
                std::fs::write("cwd-write.bin", b"probe").is_ok(),
            );
            flag("read", std::fs::read(&args[2]).is_ok());
            flag(
                "write",
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(&args[3])
                    .is_ok(),
            );
            flag("create", std::fs::write(&args[4], b"probe").is_ok());
            let path: Vec<u16> = args[5].encode_utf16().chain(Some(0)).collect();
            unsafe {
                let mut key = std::ptr::null_mut();
                // HKCU is a sign-extended predefined handle.
                let ok = RegOpenKeyExW(
                    0x80000001u32 as i32 as isize as Handle,
                    path.as_ptr(),
                    0,
                    0x20019,
                    &mut key,
                ) == 0;
                flag("registry", ok);
                if ok {
                    RegCloseKey(key);
                }
                let mut dangerous = false;
                for access in [0x1, 0x2, 0x8, 0x10, 0x20, 0x40, 0x200] {
                    let handle = OpenProcess(access, 0, args[6].parse().unwrap());
                    dangerous |= !handle.is_null();
                    if !handle.is_null() {
                        CloseHandle(handle);
                    }
                }
                flag("parent", dangerous);
            }
            flag(
                "secret",
                std::env::var_os("OXYDE_SANDBOX_TEST_SECRET").is_some(),
            );
            let mut wsa_data = [0u64; 64];
            let network_error = unsafe { WSAStartup(0x202, wsa_data.as_mut_ptr().cast()) };
            println!("network_startup_error={network_error}");
            flag("network_initialized", network_error == 0);
            std::io::stdout().flush().unwrap();
            for (name, address) in [("ipv4", &args[8]), ("ipv6", &args[9])] {
                if network_error != 0 {
                    println!("{name}=not_exercised");
                    continue;
                }
                let result = std::net::TcpStream::connect_timeout(
                    &address.parse().unwrap(),
                    Duration::from_millis(600),
                );
                flag(name, result.is_ok());
                if let Err(error) = result {
                    println!("{name}_error={:?}", error.raw_os_error());
                }
            }
            if network_error == 0 {
                unsafe {
                    WSACleanup();
                }
            }
            flag(
                "child",
                std::process::Command::new(std::env::current_exe().unwrap())
                    .arg("leaf")
                    .status()
                    .is_ok_and(|s| s.success()),
            );
            std::io::stdout().flush().unwrap();
        }
        other => panic!("unknown probe {other}"),
    }
}
