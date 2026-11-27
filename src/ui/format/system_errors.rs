//! System errors that reach a notice as raw text: `… (os error N)`. The common ones are
//! named in the interface language; the raw text stays in the log.
use crate::ui::i18n::Message;

#[cfg(test)]
#[path = "system_errors/tests/codes.rs"]
mod code_tests;

/// The system error code a raw message ends with, as std formats an `io::Error`.
fn code(detail: &str) -> Option<i32> {
    let start = detail.rfind("(os error ")? + "(os error ".len();
    let end = start + detail[start..].find(')')?;
    detail[start..end].parse().ok()
}

#[cfg(target_os = "windows")]
fn message_for(code: i32) -> Option<Message> {
    Some(match code {
        2 | 3 | 161 => Message::SystemErrorNotFound,
        5 | 1314 => Message::SystemErrorAccessDenied,
        32 | 33 | 1224 => Message::SystemErrorInUse,
        39 | 112 => Message::SystemErrorDiskFull,
        80 | 183 => Message::SystemErrorAlreadyExists,
        145 => Message::SystemErrorNotEmpty,
        206 => Message::SystemErrorNameTooLong,
        19 => Message::SystemErrorReadOnly,
        21 | 53 | 59 | 64 | 67 | 121 | 1231 | 1232 => Message::SystemErrorNetwork,
        _ => return None,
    })
}

#[cfg(not(target_os = "windows"))]
fn message_for(code: i32) -> Option<Message> {
    Some(match code {
        2 | 20 => Message::SystemErrorNotFound,
        1 | 13 => Message::SystemErrorAccessDenied,
        16 | 26 => Message::SystemErrorInUse,
        28 | 122 => Message::SystemErrorDiskFull,
        17 => Message::SystemErrorAlreadyExists,
        39 => Message::SystemErrorNotEmpty,
        36 => Message::SystemErrorNameTooLong,
        30 => Message::SystemErrorReadOnly,
        100 | 101 | 110 | 112 | 113 => Message::SystemErrorNetwork,
        _ => return None,
    })
}

/// The interface message for a raw system error, if it is a common one.
pub(super) fn message(detail: &str) -> Option<Message> {
    message_for(code(detail)?)
}
