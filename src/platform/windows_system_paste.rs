//! Fallback paste through the shell's own Paste verb, for files Oxyde cannot copy:
//! virtual files (`FileGroupDescriptorW`) and paths a source has not written yet.
//! VMware Tools stages host folders only when the shell pastes them.
use log::warn;
use std::time::{Duration, Instant};
use windows::{
    core::{HSTRING, PCSTR},
    Win32::{
        System::{
            DataExchange::{IsClipboardFormatAvailable, RegisterClipboardFormatW},
            Ole::{OleInitialize, OleUninitialize, CF_HDROP},
        },
        UI::{
            Shell::{
                BHID_SFUIObject, IContextMenu, IShellItem, SHCreateItemFromParsingName,
                CMINVOKECOMMANDINFO,
            },
            WindowsAndMessaging::{
                DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE, SW_SHOWNORMAL,
            },
        },
    },
};

/// The shell may finish its transfer on this thread's messages after the verb returns.
const PUMP_AFTER_PASTE: Duration = Duration::from_secs(30);

fn virtual_files_offered() -> bool {
    let format = unsafe { RegisterClipboardFormatW(windows::core::w!("FileGroupDescriptorW")) };
    format != 0 && unsafe { IsClipboardFormatAvailable(format) }.is_ok()
}

/// Whether the clipboard offers only files the shell can paste.
pub(super) fn available() -> bool {
    virtual_files_offered()
}

fn invoke_paste(target: &str) -> windows::core::Result<()> {
    let target = HSTRING::from(target);
    let folder: IShellItem = unsafe { SHCreateItemFromParsingName(&target, None) }?;
    let menu: IContextMenu = unsafe { folder.BindToHandler(None, &BHID_SFUIObject) }?;
    let info = CMINVOKECOMMANDINFO {
        cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
        lpVerb: PCSTR(c"paste".as_ptr().cast()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe { menu.InvokeCommand(&info) }
}

fn pump_messages(duration: Duration) {
    let started = Instant::now();
    while started.elapsed() < duration {
        let mut message = MSG::default();
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Runs the shell's Paste into `target` on a dedicated OLE thread. Returns false when
/// the clipboard offers no files at all.
pub(super) fn paste_into(target: &str) -> Result<bool, String> {
    let hdrop = unsafe { IsClipboardFormatAvailable(CF_HDROP.0 as u32) }.is_ok();
    if !hdrop && !virtual_files_offered() {
        return Ok(false);
    }
    let target = target.to_owned();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("system-paste".into())
        .spawn(move || {
            if let Err(error) = unsafe { OleInitialize(None) } {
                let _ = sender.send(Err(format!("Cannot initialize OLE: {error}")));
                return;
            }
            let result = invoke_paste(&target);
            let succeeded = result.is_ok();
            let _ = sender.send(result.map_err(|error| error.to_string()));
            if succeeded {
                pump_messages(PUMP_AFTER_PASTE);
            }
            unsafe { OleUninitialize() };
        })
        .map_err(|error| error.to_string())?;
    let result = receiver
        .recv()
        .map_err(|_| "System paste stopped unexpectedly".to_string())?;
    if let Err(error) = &result {
        warn!("System paste failed: {error}");
    }
    result.map(|()| true)
}
