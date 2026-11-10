//! Makes Oxyde's library the user's Open and Save dialogs, or gives them back to
//! Windows. The system's dialogs are two COM classes of `comdlg32.dll`, registered for
//! the machine; the same classes registered for the user, under
//! `HKCU\Software\Classes\CLSID`, are found first by every application that runs with
//! the user's rights. Removing the user's keys restores the system's. An application
//! run as administrator ignores them, and 32-bit ones read another branch: both keep
//! the system's dialogs.
//!
//! A registration pointing to a library that is gone would leave applications without
//! any dialog: only a copy that stays in place is offered the option (not a portable
//! one), the uninstaller withdraws it, and an Oxyde that finds its registration
//! pointing nowhere removes it.
use super::super::windows_folder_handler::{class_text, remove_empty, write, Value};

/// The library, next to the main executable.
pub(in crate::platform) const LIBRARY: &str = "oxyde_file_dialog.dll";
/// `CLSID_FileOpenDialog` and `CLSID_FileSaveDialog`.
const DIALOGS: [&str; 2] = [r"CLSID\{DC1C5A9C-E88A-4DDE-A5A1-60F82A20AEF7}", r"CLSID\{C0B4E2F3-BA21-4773-8DBA-335EC946EB8B}"];
const SERVER: &str = "InprocServer32";

/// The values that register `library`, or none of them when it is None.
fn values(library: Option<&str>) -> Vec<Value> {
    DIALOGS.iter().flat_map(|dialog| {
        let key = format!(r"{dialog}\{SERVER}");
        [
            Value { key: key.clone(), name: String::new(), data: library.map(str::to_owned), expand: false },
            // As the system's: the dialog lives in the thread that creates it.
            Value { key, name: "ThreadingModel".into(), data: library.map(|_| "Apartment".to_owned()), expand: false },
        ]
    }).collect()
}

/// Whether `registered`, what a dialog's class names for the user, is `library`.
fn names(registered: Option<&str>, library: &str) -> bool {
    registered.is_some_and(|registered| registered.eq_ignore_ascii_case(library))
}

/// Whether `registered` is a copy of Oxyde's library, wherever it is.
fn ours(registered: &str) -> bool {
    std::path::Path::new(registered).file_name().is_some_and(|name| name.eq_ignore_ascii_case(LIBRARY))
}

/// The library of this copy of Oxyde, when it was installed with it.
fn library() -> Option<String> {
    let library = std::env::current_exe().ok()?.with_file_name(LIBRARY);
    let text = library.to_str()?;
    let text = text.strip_prefix(r"\\?\").unwrap_or(text).to_owned();
    library.is_file().then_some(text)
}

fn registered(dialog: &str) -> Option<String> {
    class_text(&format!(r"{dialog}\{SERVER}"), "")
}

fn apply(values: &[Value]) -> Result<(), String> {
    let mut outcome = Ok(());
    for value in values {
        if let Err(error) = write(value) {
            outcome = Err(error);
        }
    }
    values.iter().filter(|value| value.data.is_none()).for_each(|value| remove_empty(&value.key));
    outcome
}

/// Whether applications choose their files with this copy of Oxyde; None when it has
/// no library, or is a portable copy that does not already answer them.
pub(in crate::platform) fn enabled() -> Option<bool> {
    let library = library()?;
    let enabled = DIALOGS.iter().all(|dialog| names(registered(dialog).as_deref(), &library));
    (enabled || !crate::app_paths::is_portable()).then_some(enabled)
}

pub(in crate::platform) fn set(enabled: bool) -> Result<(), String> {
    if !enabled {
        return apply(&values(None));
    }
    if crate::app_paths::is_portable() {
        return Err("A portable copy does not answer the dialogs of other applications".into());
    }
    let library = library().ok_or("Oxyde's dialog library is missing")?;
    let written = apply(&values(Some(&library)));
    if written.is_err() {
        // Half a registration would leave one of the two dialogs to Oxyde.
        let _ = apply(&values(None));
    }
    written
}

/// Gives the dialogs back when they are registered to a copy of Oxyde's library that
/// is gone, or to any copy when Oxyde is uninstalled (`gone` None).
fn withdraw_where(gone: Option<&dyn Fn(&str) -> bool>) {
    let stale = DIALOGS.iter().filter_map(|dialog| registered(dialog)).any(|registered| {
        ours(&registered) && gone.is_none_or(|gone| gone(&registered))
    });
    if stale {
        if let Err(error) = apply(&values(None)) {
            log::warn!("Cannot give the dialogs back to Windows: {error}");
        }
    }
}

/// What the uninstaller asks for.
pub(in crate::platform) fn withdraw() {
    withdraw_where(None);
}

/// Removes a registration whose library is gone. Reads the disk: off the UI thread.
pub(in crate::platform) fn repair() {
    withdraw_where(Some(&|library| !std::path::Path::new(library).is_file()));
}

#[cfg(test)]
#[path = "tests/registration.rs"]
mod tests;
