//! Asks Oxyde through its launcher, which is run with the request on its standard
//! input and writes the answer on its standard output. Meanwhile the application's
//! window is disabled, as under any modal dialog, and its messages keep being
//! dispatched: it repaints, and its timers run.
use crate::request::{answer, Answer};
use std::cell::Cell;
use windows::core::{HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, HWND, WAIT_OBJECT_0};
use windows::Win32::Security::SECURITY_ATTRIBUTES;
use windows::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess, InitializeProcThreadAttributeList, TerminateProcess,
    UpdateProcThreadAttribute, WaitForSingleObject, CREATE_NO_WINDOW, EXTENDED_STARTUPINFO_PRESENT, INFINITE,
    LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_HANDLE_LIST, STARTF_USESTDHANDLES, STARTUPINFOEXW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, DispatchMessageW, GetAncestor, IsWindow, MsgWaitForMultipleObjects, PeekMessageW, PostQuitMessage,
    SetForegroundWindow, TranslateMessage, ASFW_ANY, GA_ROOT, MSG, PM_REMOVE, QS_ALLINPUT, WM_QUIT,
};

/// The launcher, next to this library.
const LAUNCHER: &str = "oxyde-folder-launcher.exe";

pub(crate) enum Outcome {
    /// Oxyde cannot be reached, or shows nothing: the system's dialog is to be shown.
    Unavailable,
    Cancelled,
    /// The application closed its dialog, or its thread is ending.
    Closed,
    Chosen(Answer),
}

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_invalid() && !self.0 .0.is_null() {
            unsafe { let _ = CloseHandle(self.0); }
        }
    }
}

/// The launcher running, with what it reads and writes.
struct Launched {
    process: Handle,
    input: Handle,
    output: Handle,
}

/// A pipe whose `inherited` end (the reading one, or the writing one) is the launcher's.
fn pipe(inherited_reads: bool) -> Option<(Handle, Handle)> {
    let attributes = SECURITY_ATTRIBUTES { nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32, lpSecurityDescriptor: std::ptr::null_mut(), bInheritHandle: true.into() };
    let (mut read, mut write) = (HANDLE::default(), HANDLE::default());
    unsafe { CreatePipe(&mut read, &mut write, Some(&attributes), 64 * 1024) }.ok()?;
    let (read, write) = (Handle(read), Handle(write));
    let kept = if inherited_reads { write.0 } else { read.0 };
    unsafe { SetHandleInformation(kept, HANDLE_FLAG_INHERIT.0, Default::default()) }.ok()?;
    Some((read, write))
}

fn launch() -> Option<Launched> {
    let launcher = crate::module_path()?.with_file_name(LAUNCHER);
    if !launcher.is_file() {
        return None;
    }
    let (its_input, input) = pipe(true)?;
    let (output, its_output) = pipe(false)?;
    let program = HSTRING::from(launcher.as_os_str());
    let mut command: Vec<u16> = format!("\"{}\" --choose", launcher.display()).encode_utf16().chain([0]).collect();
    unsafe {
        // The launcher inherits its two pipes and nothing else of the application.
        let mut size = 0usize;
        let _ = InitializeProcThreadAttributeList(None, 1, None, &mut size);
        let mut memory = vec![0usize; size.div_ceil(std::mem::size_of::<usize>()).max(1)];
        let list = LPPROC_THREAD_ATTRIBUTE_LIST(memory.as_mut_ptr().cast());
        InitializeProcThreadAttributeList(Some(list), 1, None, &mut size).ok()?;
        let handles = [its_input.0, its_output.0];
        let listed = UpdateProcThreadAttribute(list, 0, PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize, Some(handles.as_ptr().cast()), std::mem::size_of_val(&handles), None, None);
        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = its_input.0;
        startup.StartupInfo.hStdOutput = its_output.0;
        startup.lpAttributeList = list;
        let mut started = PROCESS_INFORMATION::default();
        let created = listed.and_then(|()| CreateProcessW(
            &program,
            Some(PWSTR(command.as_mut_ptr())),
            None,
            None,
            true,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW,
            None,
            PCWSTR::null(),
            &startup.StartupInfo,
            &mut started,
        ));
        DeleteProcThreadAttributeList(list);
        created.ok()?;
        drop(Handle(started.hThread));
        Some(Launched { process: Handle(started.hProcess), input, output })
    }
}

fn write_all(handle: HANDLE, mut bytes: &[u8]) -> bool {
    while !bytes.is_empty() {
        let mut written = 0u32;
        if unsafe { WriteFile(handle, Some(bytes), Some(&mut written), None) }.is_err() || written == 0 {
            return false;
        }
        bytes = &bytes[written as usize..];
    }
    true
}

/// Everything the launcher writes, until it closes its end.
fn read_all(handle: Handle) -> Vec<u8> {
    let mut read = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let mut count = 0u32;
        if unsafe { ReadFile(handle.0, Some(&mut chunk), Some(&mut count), None) }.is_err() || count == 0 {
            return read;
        }
        read.extend_from_slice(&chunk[..count as usize]);
    }
}

// The handle goes to the thread that reads it and nowhere else.
unsafe impl Send for Handle {}

/// Dispatches the thread's messages until the launcher ends; false when the thread is
/// told to quit meanwhile.
fn wait(process: HANDLE) -> bool {
    unsafe {
        loop {
            let waited = MsgWaitForMultipleObjects(Some(&[process]), false, INFINITE, QS_ALLINPUT);
            if waited == WAIT_OBJECT_0 {
                return true;
            }
            if waited.0 != WAIT_OBJECT_0.0 + 1 {
                return false;
            }
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                if message.message == WM_QUIT {
                    // The application's own loop is the one to end with it.
                    PostQuitMessage(message.wParam.0 as i32);
                    return false;
                }
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}

/// The top-level window a dialog over `owner` disables.
pub(crate) fn top_level(owner: HWND) -> HWND {
    unsafe {
        if owner.0.is_null() || !IsWindow(Some(owner)).as_bool() {
            return HWND::default();
        }
        let root = GetAncestor(owner, GA_ROOT);
        if root.0.is_null() { owner } else { root }
    }
}

/// Shows Oxyde for `request` over `owner` and waits for the user. `showing` holds the
/// launcher's process meanwhile, which `Close` ends.
pub(crate) fn ask(request: &str, owner: HWND, showing: &Cell<isize>) -> Outcome {
    // This process has the foreground: the window Oxyde opens may take it.
    unsafe { let _ = AllowSetForegroundWindow(ASFW_ANY); }
    let Some(Launched { process, input, output }) = launch() else { return Outcome::Unavailable };
    let asked = write_all(input.0, request.as_bytes());
    drop(input);
    let Ok(reader) = std::thread::Builder::new().name("oxyde-file-dialog".into()).spawn(move || read_all(output)) else {
        unsafe { let _ = TerminateProcess(process.0, 1); }
        return Outcome::Unavailable;
    };
    let disabled = !owner.0.is_null() && unsafe { IsWindowEnabled(owner).as_bool() };
    if disabled {
        unsafe { let _ = EnableWindow(owner, false); }
    }
    showing.set(process.0 .0 as isize);
    let waited = asked && wait(process.0);
    let closed = showing.replace(0) == 0;
    if !waited {
        unsafe { let _ = TerminateProcess(process.0, 1); }
    }
    let mut code = 1u32;
    unsafe {
        WaitForSingleObject(process.0, INFINITE);
        let _ = GetExitCodeProcess(process.0, &mut code);
    }
    let written = reader.join().unwrap_or_default();
    let outcome = if closed || !waited {
        Outcome::Closed
    } else if code != 0 {
        Outcome::Unavailable
    } else {
        match std::str::from_utf8(&written).map_err(|_| ()).and_then(answer) {
            Ok(Some(answer)) => Outcome::Chosen(answer),
            Ok(None) => Outcome::Cancelled,
            Err(()) => Outcome::Unavailable,
        }
    };
    if disabled {
        unsafe {
            let _ = EnableWindow(owner, true);
            if !matches!(outcome, Outcome::Unavailable) {
                let _ = SetForegroundWindow(owner);
            }
        }
    }
    outcome
}

/// Ends the launcher of a dialog being shown, which closes Oxyde's window; false when
/// none is shown.
pub(crate) fn close(showing: &Cell<isize>) -> bool {
    let process = showing.replace(0);
    if process == 0 {
        return false;
    }
    unsafe { let _ = TerminateProcess(HANDLE(process as _), 1); }
    true
}
