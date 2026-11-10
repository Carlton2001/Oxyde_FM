//! The launcher's side of an application's dialog: what the library asks comes on the
//! standard input and the answer leaves on the standard output. In between, the
//! launcher makes a pipe for the answer, hands its name and the request to the running
//! Oxyde, or to one it starts without a main window, and waits. It ends with
//! `UNAVAILABLE` when Oxyde shows no window in time or leaves without an answer: the
//! library then shows the system's dialog. The library kills it to close the dialog.
use std::io::{Read, Write};
use std::os::windows::io::AsRawHandle;
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT};
use windows::Win32::Storage::FileSystem::ReadFile;
use windows::Win32::System::Pipes::ConnectNamedPipe;

/// The switch the library runs the launcher with.
pub(in crate::platform) const SWITCH: &str = "--choose";
/// The switch of an Oxyde started to answer a dialog: it opens no main window.
pub(in crate::platform) const FOR_DIALOG: &str = "--file-chooser";
/// What the names of the answers' pipes start with.
pub(in crate::platform) const PIPES: &str = r"\\.\pipe\Oxyde.Dialog.";
/// How the launcher ends when Oxyde does not answer.
const UNAVAILABLE: i32 = 2;
/// How long Oxyde may take to show its window, starting included.
const SHOWING: Duration = Duration::from_secs(15);
const LIMIT: u64 = 256 * 1024;
/// More than any selection names.
const ANSWER_LIMIT: usize = 64 * 1024 * 1024;

/// What Oxyde wrote of its answer so far, split when its window is shown.
fn shown(received: &[u8]) -> Option<&[u8]> {
    received.strip_prefix(super::SHOWN.as_bytes())
}

/// The answer the library reads, when Oxyde gave one whole.
fn answer(received: &[u8]) -> Option<&str> {
    let answer = std::str::from_utf8(shown(received)?).ok()?;
    (answer.starts_with("ok\n") || answer == "cancel\n").then_some(answer)
}

/// The Oxyde this launcher starts would inherit the pipes of the library, which waits
/// for them to close.
fn keep_handles() {
    for handle in [std::io::stdin().as_raw_handle(), std::io::stdout().as_raw_handle(), std::io::stderr().as_raw_handle()] {
        if !handle.is_null() {
            unsafe { let _ = SetHandleInformation(HANDLE(handle), HANDLE_FLAG_INHERIT.0, Default::default()); }
        }
    }
}

/// Hands the request to Oxyde, starting one when none runs.
fn hand_over(lines: &[String], until: Instant) -> bool {
    if super::super::windows_instance::hand_over_dialog(lines) {
        return true;
    }
    let Some(program) = super::super::windows_folder_launcher::program() else { return false };
    let started = std::process::Command::new(program)
        .arg(FOR_DIALOG)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    if started.is_err() {
        return false;
    }
    while Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
        if super::super::windows_instance::hand_over_dialog(lines) {
            return true;
        }
    }
    false
}

fn relay() -> Option<String> {
    keep_handles();
    let mut asked = String::new();
    std::io::stdin().take(LIMIT).read_to_string(&mut asked).ok()?;
    let name = format!("{PIPES}{}.{}", std::process::id(), uuid::Uuid::new_v4().simple());
    let pipe = super::super::windows_instance::create(&name)?;
    let until = Instant::now() + SHOWING;
    let mut lines = vec![name];
    lines.extend(asked.lines().filter(|line| !line.is_empty()).map(str::to_owned));
    if !hand_over(&lines, until) {
        return None;
    }
    collect(pipe, until)
}

/// What Oxyde answers on `pipe`, when it shows its window before `until` and then
/// gives a whole answer.
fn collect(pipe: super::super::windows_instance::Pipe, until: Instant) -> Option<String> {
    let (sender, receiver) = channel::<Vec<u8>>();
    std::thread::Builder::new().name("oxyde-dialog-answer".into()).spawn(move || {
        let pipe = pipe;
        // Oxyde connects once its window is shown, and closes the pipe after its answer.
        unsafe { let _ = ConnectNamedPipe(pipe.0, None); }
        let mut chunk = [0u8; 8192];
        loop {
            let mut read = 0u32;
            if unsafe { ReadFile(pipe.0, Some(&mut chunk), Some(&mut read), None) }.is_err() || read == 0 {
                return;
            }
            if sender.send(chunk[..read as usize].to_vec()).is_err() {
                return;
            }
        }
    }).ok()?;
    let mut received = Vec::new();
    while shown(&received).is_none() {
        match receiver.recv_timeout(until.saturating_duration_since(Instant::now())) {
            Ok(chunk) => received.extend(chunk),
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return None,
        }
    }
    while let Ok(chunk) = receiver.recv() {
        if received.len() + chunk.len() > ANSWER_LIMIT {
            return None;
        }
        received.extend(chunk);
    }
    answer(&received).map(str::to_owned)
}

/// Runs the dialog and returns how the launcher ends.
pub(in crate::platform) fn run() -> i32 {
    let Some(answer) = relay() else { return UNAVAILABLE };
    let mut output = std::io::stdout();
    if output.write_all(answer.as_bytes()).and_then(|()| output.flush()).is_err() {
        return UNAVAILABLE;
    }
    0
}

#[cfg(test)]
#[path = "tests/relay.rs"]
mod tests;
