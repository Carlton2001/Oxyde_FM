//! One Oxyde per session and per storage folder: a later launch, such as the Shell
//! opening a folder with Oxyde, hands its locations to the running one over a named
//! pipe and ends. The first process owns the pipe's only instance; the pipe is named
//! after the session and the storage folder, so a portable copy and a debug build keep
//! to themselves. An Oxyde run as administrator has a pipe of its own, which the
//! launcher of folders reaches too: a folder then opens in the Oxyde that runs, not in
//! one more.
use super::{InstanceClaim, InstanceMessage, InstanceRequest};
use futures::channel::mpsc::{unbounded, UnboundedSender};
use std::io::Write;
use std::os::windows::io::AsRawHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;
use windows::core::{HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, LocalFree, ERROR_NO_DATA, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, HANDLE, HLOCAL};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, SDDL_REVISION_1, SE_KERNEL_OBJECT,
};
use windows::Win32::Security::{
    EqualSid, GetTokenInformation, TokenElevation, TokenUser, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    SECURITY_ATTRIBUTES, TOKEN_ELEVATION, TOKEN_INFORMATION_CLASS, TOKEN_QUERY, TOKEN_USER,
};
use windows::Win32::Storage::FileSystem::{ReadFile, FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeClientProcessId, GetNamedPipeServerProcessId, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{GetCurrentProcess, GetCurrentProcessId, OpenProcessToken};
use windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;

/// First line of a message, which a later version may change: folders to open, items
/// to select in their folder, or the dialog of an application to answer
/// (`windows_file_dialog`).
const OPEN: &str = "oxyde-open 1";
const SELECT: &str = "oxyde-select 1";
const CHOOSE: &str = "oxyde-choose 1";
/// Well above any command line Windows accepts.
const LIMIT: usize = 256 * 1024;
/// How long a later launch waits for the running Oxyde to take its message.
const PATIENCE: Duration = Duration::from_secs(5);

fn pipe_name(session: u32, storage: &str, debug: bool, elevated: bool) -> String {
    let storage = blake3::hash(storage.to_lowercase().as_bytes()).to_hex();
    let kind = if debug { "Devel" } else { "Release" };
    let rights = if elevated { ".Elevated" } else { "" };
    format!(r"\\.\pipe\Oxyde.Instance.{kind}{rights}.{session}.{}", &storage[..16])
}

fn encode(header: &str, locations: &[String]) -> String {
    let mut message = String::from(header);
    for location in locations.iter().filter(|location| !location.contains(['\r', '\n'])) {
        message.push('\n');
        message.push_str(location);
    }
    message
}

/// The header of a message and its locations.
fn decode(message: &str) -> Option<(&'static str, Vec<String>)> {
    let mut lines = message.lines();
    let first = lines.next()?;
    let header = [OPEN, SELECT, CHOOSE].into_iter().find(|header| *header == first)?;
    Some((header, lines.filter(|line| !line.is_empty()).map(str::to_owned).collect()))
}

/// The pipe this process serves, and whether it gave it up to quit.
static CLAIMED: OnceLock<String> = OnceLock::new();
static RELEASED: AtomicBool = AtomicBool::new(false);

pub(super) struct Pipe(pub(super) HANDLE);
// The handle is only used by the thread that serves it.
unsafe impl Send for Pipe {}
impl Drop for Pipe {
    fn drop(&mut self) { unsafe { let _ = CloseHandle(self.0); } }
}

/// What this process's token says of `class`, in a buffer aligned for its structure.
fn token_information(class: TOKEN_INFORMATION_CLASS) -> Option<Vec<u64>> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).ok()?;
        let mut size = 0u32;
        let _ = GetTokenInformation(token, class, None, 0, &mut size);
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        let read = GetTokenInformation(token, class, Some(buffer.as_mut_ptr().cast()), size, &mut size);
        let _ = CloseHandle(token);
        read.ok().map(|()| buffer)
    }
}

/// Whether this process runs as administrator.
fn elevated() -> bool {
    token_information(TokenElevation).is_some_and(|elevation| unsafe { (*elevation.as_ptr().cast::<TOKEN_ELEVATION>()).TokenIsElevated != 0 })
}

struct Descriptor(PSECURITY_DESCRIPTOR);
impl Drop for Descriptor {
    fn drop(&mut self) { unsafe { let _ = LocalFree(Some(HLOCAL(self.0 .0))); } }
}

/// The pipe is the user's and the user's processes write to it, whatever rights its
/// creator runs with: run as administrator, Oxyde would otherwise own it as the
/// Administrators group and be out of reach of the launcher, which is not.
fn descriptor() -> Option<Descriptor> {
    let user = token_information(TokenUser)?;
    let user = unsafe {
        let mut text = PWSTR::null();
        ConvertSidToStringSidW((*user.as_ptr().cast::<TOKEN_USER>()).User.Sid, &mut text).ok()?;
        let user = text.to_string();
        let _ = LocalFree(Some(HLOCAL(text.0.cast())));
        user.ok()?
    };
    // Medium integrity: what a pipe gets unlabelled, written here so that it does not
    // depend on its creator's.
    let text = HSTRING::from(format!("O:{user}D:(A;;GRGW;;;{user})(A;;GA;;;SY)S:(ML;;NW;;;ME)"));
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(&text, SDDL_REVISION_1, &mut descriptor, None) }.ok()?;
    Some(Descriptor(descriptor))
}

/// The pipe's only instance, or None when another process already owns it.
pub(super) fn create(name: &str) -> Option<Pipe> {
    let descriptor = descriptor()?;
    let security = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0 .0,
        bInheritHandle: false.into(),
    };
    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(HSTRING::from(name).as_ptr()),
            PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            0,
            64 * 1024,
            0,
            Some(&security),
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

/// Serves later launches until the application quits. The pipe then closes at once,
/// before anything is read: a launch that had reached it fails to write and starts an
/// Oxyde of its own.
fn serve(pipe: Pipe, sender: UnboundedSender<InstanceMessage>, released: &AtomicBool) {
    while !released.load(Ordering::SeqCst) {
        let connected = unsafe { ConnectNamedPipe(pipe.0, None) };
        if released.load(Ordering::SeqCst) {
            return;
        }
        // A sender may have connected, and even written and left, before this call.
        if connected.as_ref().is_err_and(|error| ![ERROR_PIPE_CONNECTED.to_hresult(), ERROR_NO_DATA.to_hresult()].contains(&error.code())) {
            log::warn!("Cannot wait for later launches: {connected:?}");
            return;
        }
        let message = receive(&pipe);
        let mut sender_process = 0u32;
        let sender_process = unsafe { GetNamedPipeClientProcessId(pipe.0, &mut sender_process) }.ok().map(|()| sender_process);
        unsafe { let _ = DisconnectNamedPipe(pipe.0); }
        let Some((header, locations)) = message.as_deref().and_then(decode) else {
            log::warn!("Ignoring a message that is not a later launch's");
            continue;
        };
        let request = if header == CHOOSE {
            match super::windows_file_dialog::request(&locations, sender_process) {
                Some(request) => InstanceRequest::ChooseFile(request),
                None => {
                    log::warn!("Ignoring a dialog that names no pipe to answer on");
                    continue;
                }
            }
        } else if header == SELECT {
            InstanceRequest::Items(super::windows_startup::items(&locations))
        } else {
            InstanceRequest::Folders(super::windows_startup::folders(&locations))
        };
        if sender.unbounded_send(InstanceMessage { request, activation_token: None }).is_err() {
            return;
        }
    }
}

/// Whether the pipe is this user's: another account could have created the name first
/// to read what is opened.
pub(super) fn owned_by_this_user(pipe: HANDLE) -> bool {
    let Some(user) = token_information(TokenUser) else { return false };
    unsafe {
        let user = (*user.as_ptr().cast::<TOKEN_USER>()).User.Sid;
        let mut owner = PSID::default();
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        if GetSecurityInfo(pipe, SE_KERNEL_OBJECT, OWNER_SECURITY_INFORMATION, Some(&mut owner), None, None, None, Some(&mut descriptor)).is_err() {
            return false;
        }
        let same = EqualSid(user, owner).is_ok();
        let _ = LocalFree(Some(HLOCAL(descriptor.0)));
        same
    }
}

/// Hands `locations` to the Oxyde that owns the pipe and lets it come forward.
fn forward(name: &str, header: &str, locations: &[String]) -> std::io::Result<()> {
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
    pipe.write_all(encode(header, locations).as_bytes())?;
    pipe.flush()
}

/// The pipe of this session's Oxyde for this storage folder and build, run as
/// administrator or not.
fn own_pipe_name(elevated: bool) -> Option<String> {
    let storage = crate::app_paths::config_dir()?;
    let mut session = 0u32;
    unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) }.ok()?;
    Some(pipe_name(session, &storage.to_string_lossy(), cfg!(debug_assertions), elevated))
}

/// Hands a message to the running Oxyde, the one with this process's rights before
/// the other; false when none runs, or none answers.
fn hand(header: &str, locations: &[String]) -> bool {
    let own = elevated();
    [own, !own].into_iter().any(|elevated| own_pipe_name(elevated).is_some_and(|name| forward(&name, header, locations).is_ok()))
}

/// Has the running Oxyde open `locations`, each in a tab.
pub(super) fn hand_over(locations: &[String]) -> bool {
    hand(OPEN, locations)
}

/// Has the running Oxyde select `items` in their folder.
pub(super) fn hand_over_selection(items: &[String]) -> bool {
    hand(SELECT, items)
}

/// Has the running Oxyde answer the dialog these lines describe.
pub(super) fn hand_over_dialog(lines: &[String]) -> bool {
    hand(CHOOSE, lines)
}

/// Wakes the thread serving `name`, which waits for a launch: one that says nothing.
fn wake(name: &str) {
    for _ in 0..40 {
        match std::fs::OpenOptions::new().write(true).open(name) {
            // The thread is reading a launch's message and looks at the flag next.
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY.0 as i32) => std::thread::sleep(Duration::from_millis(25)),
            _ => return,
        }
    }
}

/// Gives the pipe up as the application quits. Its last saves can take seconds, during
/// which a folder handed over would be taken and never shown; the next launch finds no
/// pipe and starts an Oxyde of its own.
pub(super) fn release() {
    let Some(name) = CLAIMED.get() else { return };
    if !RELEASED.swap(true, Ordering::SeqCst) {
        let _ = std::thread::Builder::new().name("oxyde-instance-release".into()).spawn(move || wake(name));
    }
}

pub(super) fn claim(locations: &[String]) -> InstanceClaim {
    // Run as administrator on purpose, Oxyde does not give way to one that is not.
    let Some(name) = own_pipe_name(elevated()) else { return InstanceClaim::Alone };
    // A launch that loses the race for the pipe finds it owned at its second look.
    for _ in 0..2 {
        if let Some(pipe) = create(&name) {
            let (sender, receiver) = unbounded();
            let served = std::thread::Builder::new().name("oxyde-instance".into()).spawn(move || serve(pipe, sender, &RELEASED));
            return match served {
                Ok(_) => {
                    let _ = CLAIMED.set(name);
                    InstanceClaim::First(receiver)
                }
                Err(error) => {
                    log::warn!("Cannot listen for later launches: {error}");
                    InstanceClaim::Alone
                }
            };
        }
        // Started to answer a dialog, which the launcher hands to the Oxyde that runs:
        // nothing is to be shown, and its window stays where it is.
        if std::env::args_os().any(|argument| argument == super::windows_file_dialog::relay::FOR_DIALOG) {
            return InstanceClaim::Forwarded;
        }
        match forward(&name, OPEN, locations) {
            Ok(()) => return InstanceClaim::Forwarded,
            Err(error) => log::warn!("Cannot reach the running Oxyde: {error}"),
        }
    }
    InstanceClaim::Alone
}

#[cfg(test)]
#[path = "windows_instance/tests/messages.rs"]
mod tests;
