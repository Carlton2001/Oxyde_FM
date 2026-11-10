//! One Oxyde per session and per storage folder: a later launch, such as the Shell
//! opening a folder with Oxyde, hands its locations to the running one over a named
//! pipe and ends. The first process owns the pipe's only instance; the pipe is named
//! after the session and the storage folder, so a portable copy and a debug build keep
//! to themselves.
use super::{InstanceClaim, InstanceMessage, InstanceRequest};
use futures::channel::mpsc::{unbounded, UnboundedSender};
use std::io::Write;
use std::os::windows::io::AsRawHandle;
use std::time::Duration;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, LocalFree, ERROR_NO_DATA, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, HANDLE, HLOCAL};
use windows::Win32::Security::Authorization::{GetSecurityInfo, SE_KERNEL_OBJECT};
use windows::Win32::Security::{EqualSid, GetTokenInformation, TokenOwner, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, TOKEN_OWNER, TOKEN_QUERY};
use windows::Win32::Storage::FileSystem::{ReadFile, FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeServerProcessId, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{GetCurrentProcess, GetCurrentProcessId, OpenProcessToken};
use windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;

/// First line of a message, which a later version may change.
const HEADER: &str = "oxyde-open 1";
/// Well above any command line Windows accepts.
const LIMIT: usize = 256 * 1024;
/// How long a later launch waits for the running Oxyde to take its message.
const PATIENCE: Duration = Duration::from_secs(5);

fn pipe_name(session: u32, storage: &str, debug: bool) -> String {
    let storage = blake3::hash(storage.to_lowercase().as_bytes()).to_hex();
    let kind = if debug { "Devel" } else { "Release" };
    format!(r"\\.\pipe\Oxyde.Instance.{kind}.{session}.{}", &storage[..16])
}

fn encode(locations: &[String]) -> String {
    let mut message = String::from(HEADER);
    for location in locations.iter().filter(|location| !location.contains(['\r', '\n'])) {
        message.push('\n');
        message.push_str(location);
    }
    message
}

fn decode(message: &str) -> Option<Vec<String>> {
    let mut lines = message.lines();
    (lines.next()? == HEADER).then(|| lines.filter(|line| !line.is_empty()).map(str::to_owned).collect())
}

struct Pipe(HANDLE);
// The handle is only used by the thread that serves it.
unsafe impl Send for Pipe {}

/// The pipe's only instance, or None when another process already owns it.
fn create(name: &str) -> Option<Pipe> {
    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(HSTRING::from(name).as_ptr()),
            PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            0,
            64 * 1024,
            0,
            None,
        )
    };
    (!handle.is_invalid()).then_some(Pipe(handle))
}

/// One message, read until its sender closes the pipe.
fn receive(pipe: &Pipe) -> Option<String> {
    let mut message = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let mut read = 0u32;
        let more = unsafe { ReadFile(pipe.0, Some(&mut chunk), Some(&mut read), None) }.is_ok() && read > 0;
        if !more || message.len() + read as usize > LIMIT {
            break;
        }
        message.extend_from_slice(&chunk[..read as usize]);
    }
    String::from_utf8(message).ok()
}

fn serve(pipe: Pipe, sender: UnboundedSender<InstanceMessage>) {
    loop {
        let connected = unsafe { ConnectNamedPipe(pipe.0, None) };
        // A sender may have connected, and even written and left, before this call.
        if connected.as_ref().is_err_and(|error| ![ERROR_PIPE_CONNECTED.to_hresult(), ERROR_NO_DATA.to_hresult()].contains(&error.code())) {
            log::warn!("Cannot wait for later launches: {connected:?}");
            return;
        }
        let message = receive(&pipe);
        unsafe { let _ = DisconnectNamedPipe(pipe.0); }
        let Some(locations) = message.as_deref().and_then(decode) else {
            log::warn!("Ignoring a message that is not a later launch's");
            continue;
        };
        let request = InstanceRequest::Folders(super::windows_startup::folders(&locations));
        if sender.unbounded_send(InstanceMessage { request, activation_token: None }).is_err() {
            return;
        }
    }
}

/// The owner new objects of this process get, which is the pipe's when this user's
/// Oxyde created it.
fn own_owner() -> Option<Vec<u8>> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).ok()?;
        let mut size = 0u32;
        let _ = GetTokenInformation(token, TokenOwner, None, 0, &mut size);
        let mut buffer = vec![0u8; size as usize];
        let read = GetTokenInformation(token, TokenOwner, Some(buffer.as_mut_ptr().cast()), size, &mut size);
        let _ = CloseHandle(token);
        read.ok().map(|()| buffer)
    }
}

/// Whether the pipe is this user's: another account could have created the name first
/// to read what is opened.
fn owned_by_this_user(pipe: HANDLE) -> bool {
    let Some(own) = own_owner() else { return false };
    unsafe {
        let own = (*own.as_ptr().cast::<TOKEN_OWNER>()).Owner;
        let mut owner = PSID::default();
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        if GetSecurityInfo(pipe, SE_KERNEL_OBJECT, OWNER_SECURITY_INFORMATION, Some(&mut owner), None, None, None, Some(&mut descriptor)).is_err() {
            return false;
        }
        let same = EqualSid(own, owner).is_ok();
        let _ = LocalFree(Some(HLOCAL(descriptor.0)));
        same
    }
}

/// Hands `locations` to the Oxyde that owns the pipe and lets it come forward.
fn forward(name: &str, locations: &[String]) -> std::io::Result<()> {
    let started = std::time::Instant::now();
    let mut pipe = loop {
        match std::fs::OpenOptions::new().write(true).open(name) {
            Ok(pipe) => break pipe,
            // The running Oxyde is reading another launch's message.
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY.0 as i32) && started.elapsed() < PATIENCE => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(error) => return Err(error),
        }
    };
    let handle = HANDLE(pipe.as_raw_handle());
    if !owned_by_this_user(handle) {
        return Err(std::io::Error::other("the pipe belongs to another account"));
    }
    // Only the process the user just started may bring a window forward.
    let mut server = 0u32;
    if unsafe { GetNamedPipeServerProcessId(handle, &mut server) }.is_ok() {
        unsafe { let _ = AllowSetForegroundWindow(server); }
    }
    pipe.write_all(encode(locations).as_bytes())?;
    pipe.flush()
}

/// The pipe of this session's Oxyde for this storage folder and build.
fn own_pipe_name() -> Option<String> {
    let storage = crate::app_paths::config_dir()?;
    let mut session = 0u32;
    unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) }.ok()?;
    Some(pipe_name(session, &storage.to_string_lossy(), cfg!(debug_assertions)))
}

/// Hands `locations` to the running Oxyde; false when none runs, or none answers.
pub(super) fn hand_over(locations: &[String]) -> bool {
    own_pipe_name().is_some_and(|name| forward(&name, locations).is_ok())
}

pub(super) fn claim(locations: &[String]) -> InstanceClaim {
    let Some(name) = own_pipe_name() else { return InstanceClaim::Alone };
    // A launch that loses the race for the pipe finds it owned at its second look.
    for _ in 0..2 {
        if let Some(pipe) = create(&name) {
            let (sender, receiver) = unbounded();
            let served = std::thread::Builder::new().name("oxyde-instance".into()).spawn(move || serve(pipe, sender));
            return match served {
                Ok(_) => InstanceClaim::First(receiver),
                Err(error) => {
                    log::warn!("Cannot listen for later launches: {error}");
                    InstanceClaim::Alone
                }
            };
        }
        match forward(&name, locations) {
            Ok(()) => return InstanceClaim::Forwarded,
            Err(error) => log::warn!("Cannot reach the running Oxyde: {error}"),
        }
    }
    InstanceClaim::Alone
}

#[cfg(test)]
#[path = "windows_instance/tests/messages.rs"]
mod tests;
