//! Win+E while Oxyde is the default for folders. Since Windows 11 26H2, Explorer opens
//! its own window for Win+E, without the `opennewwindow` command, while its session
//! names a window to reach directly: `sihost.exe` writes that value each time Explorer
//! starts, logon and restarts included, under the session's
//! `HKCU\…\Explorer\SessionInfo\<session>\DirectLaunchHandlers`. Removed, it leaves
//! Win+E to the command again, the launcher's.
//!
//! Nothing stays running for it. Explorer runs the user's `RunOnce` entries at each of
//! its starts, removing each before running it: the launcher is listed there with
//! [`SWITCH`], waits for the value, removes it and lists itself again for the next
//! start. A Windows without the key is left alone; the entry goes with the choice.
use super::{fail, open, wide};
use std::time::{Duration, Instant};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Registry::{RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, KEY_READ, REG_SZ, RRF_RT_ANY};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::GetCurrentProcessId;

/// The launcher's argument for the step Explorer runs at its start.
pub(in crate::platform) const SWITCH: &str = "--reclaim-win-e";
const RUN_ONCE: &str = r"Software\Microsoft\Windows\CurrentVersion\RunOnce";
const ENTRY: &str = "Oxyde Win+E";
const HANDLER: &str = "microsoft.windows.explorer";
/// How long the step waits for `sihost.exe` to name Explorer's window; it was there
/// within 40 ms when measured.
const WAIT: Duration = Duration::from_secs(30);

fn handlers(session: u32) -> String {
    format!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\SessionInfo\{session}\DirectLaunchHandlers")
}

fn command(executable: &str) -> String {
    format!("\"{executable}\" {SWITCH}")
}

/// This session's key, when this Windows reaches Explorer directly.
fn session_handlers() -> Option<String> {
    let mut session = 0u32;
    unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) }.ok()?;
    let path = handlers(session);
    open(HKEY_CURRENT_USER, &path, KEY_READ).map(|_| path)
}

fn named(path: &str) -> bool {
    let found = unsafe { RegGetValueW(HKEY_CURRENT_USER, PCWSTR(wide(path).as_ptr()), PCWSTR(wide(HANDLER).as_ptr()), RRF_RT_ANY, None, None, None) };
    found == ERROR_SUCCESS
}

fn unname(path: &str) {
    unsafe { let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, PCWSTR(wide(path).as_ptr()), PCWSTR(wide(HANDLER).as_ptr())); }
}

fn list(executable: &str) -> Result<(), String> {
    let data: Vec<u8> = wide(&command(executable)).into_iter().flat_map(u16::to_le_bytes).collect();
    let set = unsafe {
        RegSetKeyValueW(HKEY_CURRENT_USER, PCWSTR(wide(RUN_ONCE).as_ptr()), PCWSTR(wide(ENTRY).as_ptr()), REG_SZ.0, Some(data.as_ptr().cast()), data.len() as u32)
    };
    if set == ERROR_SUCCESS { Ok(()) } else { Err(fail(set)) }
}

/// Gives Win+E to the launcher `executable` now and at Explorer's next starts.
pub(super) fn claim(executable: &str) -> Result<(), String> {
    let Some(path) = session_handlers() else { return Ok(()) };
    unname(&path);
    list(executable)
}

/// Leaves Win+E to Explorer from its next start.
pub(super) fn withdraw() -> Result<(), String> {
    let removed = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, PCWSTR(wide(RUN_ONCE).as_ptr()), PCWSTR(wide(ENTRY).as_ptr())) };
    if removed == ERROR_SUCCESS || removed == ERROR_FILE_NOT_FOUND { Ok(()) } else { Err(fail(removed)) }
}

/// The step Explorer runs at its start: only while folders are still this launcher's.
pub(in crate::platform) fn run() {
    if super::enabled() != Some(true) {
        return;
    }
    let (Some(executable), Some(path)) = (super::executable(), session_handlers()) else { return };
    // Listed again first, so that a step cut short still runs at the next start.
    if list(&executable).is_err() {
        return;
    }
    let asked = Instant::now();
    while !named(&path) {
        if asked.elapsed() > WAIT {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    unname(&path);
}

#[cfg(test)]
#[path = "tests/direct_launch.rs"]
mod tests;
