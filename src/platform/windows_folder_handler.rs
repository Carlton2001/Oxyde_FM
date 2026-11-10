//! Oxyde as the program the Shell opens folders with: the `open` verb of `Directory`
//! and `Drive`, and the Win+E command, each naming the launcher next to this program
//! (`windows_folder_launcher`) and written for the user only under
//! `HKCU\Software\Classes`. `Folder` is left alone, so Control Panel, the Recycle Bin
//! and the other namespace folders stay Explorer's. The values replaced are returned to
//! the caller and written back when the choice is turned off; keys left empty are
//! removed, which gives the system's values back. A portable copy is not offered the
//! choice: deleted or moved, it would leave folders to a launcher that is gone, and
//! nothing left to give them back.
use serde::{Deserialize, Serialize};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteKeyW, RegDeleteValueW, RegOpenKeyExW, RegQueryInfoKeyW, RegQueryValueExW,
    RegSetValueExW, HKEY, HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_EXPAND_SZ, REG_OPTION_NON_VOLATILE,
    REG_SZ, REG_VALUE_TYPE,
};
use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};

const CLASSES: &str = r"Software\Classes";
/// The verb the Shell runs for Win+E and for a new Explorer window.
const NEW_WINDOW: &str = r"CLSID\{52205fd8-5dfb-447d-801a-d0b52f2e83e1}\shell\opennewwindow\command";
const DELEGATE: &str = "DelegateExecute";

/// A value under `HKCU\Software\Classes`; `data` None when it does not exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Value {
    key: String,
    name: String,
    data: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    expand: bool,
}

fn open_command(executable: &str) -> String {
    format!("\"{executable}\" \"%V\"")
}

/// What makes the launcher `executable` open folders. An empty `DelegateExecute` hides the system's,
/// which would otherwise run Explorer whatever the command says.
fn values(executable: &str) -> Vec<Value> {
    let value = |key: String, name: &str, data: String| Value { key, name: name.into(), data: Some(data), expand: false };
    let mut values = Vec::new();
    for class in ["Directory", "Drive"] {
        // The system's default there is `none`, which leaves the choice to `Folder`.
        values.push(value(format!(r"{class}\shell"), "", "open".into()));
        values.push(value(format!(r"{class}\shell\open\command"), "", open_command(executable)));
        values.push(value(format!(r"{class}\shell\open\command"), DELEGATE, String::new()));
    }
    values.push(value(NEW_WINDOW.into(), "", format!("\"{executable}\"")));
    values.push(value(NEW_WINDOW.into(), DELEGATE, String::new()));
    values
}

/// The values to write back: those kept at the last change when there are, as the ones
/// read now may be Oxyde's own from another folder; a value already Oxyde's stands for
/// none.
fn replaced(kept: Option<&str>, wanted: &[Value], read: impl Fn(&Value) -> Value) -> Vec<Value> {
    if let Some(kept) = kept.and_then(|kept| serde_json::from_str::<Vec<Value>>(kept).ok()) {
        if wanted.iter().all(|value| kept.iter().any(|kept| kept.key == value.key && kept.name == value.name)) {
            return kept;
        }
    }
    wanted.iter().map(|value| {
        let found = read(value);
        if found.data == value.data { Value { data: None, expand: false, ..found } } else { found }
    }).collect()
}

/// What turning the choice off writes: the kept values, or none of Oxyde's.
fn restored(kept: Option<&str>, wanted: &[Value]) -> Vec<Value> {
    replaced(kept, wanted, |value| Value { data: None, expand: false, ..value.clone() })
}

/// The program a command runs, when the command names it by its full path.
fn program(command: &str) -> Option<&str> {
    let command = command.trim_start();
    let program = match command.strip_prefix('"') {
        Some(quoted) => quoted.split('"').next()?,
        None => &command[..command.to_ascii_lowercase().find(".exe")? + 4],
    };
    (!program.contains('%') && std::path::Path::new(program).is_absolute()).then_some(program)
}

/// Whether the programs the kept commands run are still there. A file manager Oxyde
/// replaced may have been uninstalled since: its command written back would leave
/// folders to a program that is gone, where the system's values open them.
fn still_installed(kept: &str, exists: impl Fn(&str) -> bool) -> bool {
    let Ok(kept) = serde_json::from_str::<Vec<Value>>(kept) else { return true };
    kept.iter()
        .filter(|value| value.name.is_empty() && value.key.to_ascii_lowercase().ends_with(r"\command"))
        .filter_map(|value| value.data.as_deref().and_then(program))
        .all(exists)
}

struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) { unsafe { let _ = RegCloseKey(self.0); } }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn fail(error: WIN32_ERROR) -> String {
    windows::core::Error::from(error.to_hresult()).to_string()
}

fn open(root: HKEY, path: &str, access: windows::Win32::System::Registry::REG_SAM_FLAGS) -> Option<Key> {
    let mut key = HKEY::default();
    let found = unsafe { RegOpenKeyExW(root, PCWSTR(wide(path).as_ptr()), Some(0), access, &mut key) };
    (found == ERROR_SUCCESS).then_some(Key(key))
}

/// A string value and whether it expands environment variables.
fn text(key: &Key, name: &str) -> Option<(String, bool)> {
    let mut data = vec![0u16; 16384];
    let mut bytes = (data.len() * 2) as u32;
    let mut kind = REG_VALUE_TYPE::default();
    let found = unsafe {
        RegQueryValueExW(key.0, PCWSTR(wide(name).as_ptr()), None, Some(&mut kind), Some(data.as_mut_ptr().cast()), Some(&mut bytes))
    };
    if found != ERROR_SUCCESS || (kind != REG_SZ && kind != REG_EXPAND_SZ) {
        return None;
    }
    let text = String::from_utf16_lossy(&data[..bytes as usize / 2]);
    Some((text.trim_end_matches('\0').to_owned(), kind == REG_EXPAND_SZ))
}

fn user_path(key: &str) -> String {
    format!(r"{CLASSES}\{key}")
}

fn read(value: &Value) -> Value {
    let found = open(HKEY_CURRENT_USER, &user_path(&value.key), KEY_READ).and_then(|key| text(&key, &value.name));
    Value {
        key: value.key.clone(),
        name: value.name.clone(),
        expand: found.as_ref().is_some_and(|(_, expand)| *expand),
        data: found.map(|(data, _)| data),
    }
}

fn write(value: &Value) -> Result<(), String> {
    let path = user_path(&value.key);
    let name = wide(&value.name);
    let Some(data) = &value.data else {
        let Some(key) = open(HKEY_CURRENT_USER, &path, KEY_READ | KEY_WRITE) else { return Ok(()) };
        let removed = unsafe { RegDeleteValueW(key.0, PCWSTR(name.as_ptr())) };
        return if removed == ERROR_SUCCESS || removed == ERROR_FILE_NOT_FOUND { Ok(()) } else { Err(fail(removed)) };
    };
    let mut key = HKEY::default();
    let created = unsafe {
        RegCreateKeyExW(HKEY_CURRENT_USER, PCWSTR(wide(&path).as_ptr()), Some(0), None, REG_OPTION_NON_VOLATILE, KEY_READ | KEY_WRITE, None, &mut key, None)
    };
    if created != ERROR_SUCCESS {
        return Err(fail(created));
    }
    let key = Key(key);
    let data: Vec<u8> = wide(data).into_iter().flat_map(u16::to_le_bytes).collect();
    let kind = if value.expand { REG_EXPAND_SZ } else { REG_SZ };
    let set = unsafe { RegSetValueExW(key.0, PCWSTR(name.as_ptr()), Some(0), kind, Some(&data)) };
    if set == ERROR_SUCCESS { Ok(()) } else { Err(fail(set)) }
}

/// Removes `key` and its parents up to the class while they hold nothing, so that the
/// user's classes are left as they were found.
fn remove_empty(key: &str) {
    let mut key = key;
    loop {
        let path = user_path(key);
        let Some(handle) = open(HKEY_CURRENT_USER, &path, KEY_READ) else { return };
        let (mut subkeys, mut values) = (0u32, 0u32);
        let counted = unsafe {
            RegQueryInfoKeyW(handle.0, None, None, None, Some(&mut subkeys), None, None, Some(&mut values), None, None, None, None)
        };
        drop(handle);
        if counted != ERROR_SUCCESS || subkeys != 0 || values != 0 {
            return;
        }
        if unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, PCWSTR(wide(&path).as_ptr())) } != ERROR_SUCCESS {
            return;
        }
        match key.rsplit_once('\\') {
            Some((parent, _)) if !parent.eq_ignore_ascii_case("CLSID") => key = parent,
            _ => return,
        }
    }
}

/// The launcher shipped next to this program; None when it is missing, as in a build
/// of the main executable alone.
fn executable() -> Option<String> {
    let launcher = std::env::current_exe().ok()?.with_file_name(super::windows_folder_launcher::EXECUTABLE);
    if !launcher.is_file() {
        return None;
    }
    let path = launcher.into_os_string().into_string().ok()?;
    // A quote cannot be part of a Windows file name; `%` would be read as a parameter.
    (!path.contains(['"', '%'])).then(|| path.strip_prefix(r"\\?\").unwrap_or(&path).to_owned())
}

fn class_text(path: &str, name: &str) -> Option<String> {
    open(HKEY_CLASSES_ROOT, path, KEY_READ).and_then(|key| text(&key, name)).map(|(text, _)| text)
}

/// The choice as offered: a portable copy only has it to turn off, when an earlier
/// version or an installed stay of the same folder turned it on.
fn offered(enabled: bool, portable: bool) -> Option<bool> {
    (enabled || !portable).then_some(enabled)
}

/// Whether the Shell opens folders with this Oxyde, read from the merged view the Shell
/// itself reads. None when the choice is not offered: the launcher is missing, its path
/// cannot be written in a command, or this copy is portable.
pub(super) fn enabled() -> Option<bool> {
    let executable = executable()?;
    let enabled = class_text(r"Directory\shell", "").is_some_and(|verb| verb.eq_ignore_ascii_case("open"))
        && class_text(r"Directory\shell\open\command", "").is_some_and(|command| command.eq_ignore_ascii_case(&open_command(&executable)))
        && class_text(r"Directory\shell\open\command", DELEGATE).is_none_or(|delegate| delegate.is_empty());
    offered(enabled, crate::app_paths::is_portable())
}

/// Makes this Oxyde open folders and returns the values it replaced, or writes `kept`
/// back. The Shell is told, so the change applies to the next folder opened.
pub(super) fn set(enabled: bool, kept: Option<&str>) -> Result<Option<String>, String> {
    let executable = executable().ok_or("The folder launcher is missing or its path cannot be written in a command")?;
    if enabled && crate::app_paths::is_portable() {
        return Err("A portable copy does not become the default for folders".into());
    }
    let wanted = values(&executable);
    let result = if enabled {
        let before = replaced(kept, &wanted, read);
        let written = wanted.iter().try_for_each(write);
        if written.is_err() {
            // Half a change would leave folders to a verb without its command.
            before.iter().for_each(|value| { let _ = write(value); });
            before.iter().for_each(|value| remove_empty(&value.key));
        }
        written.and_then(|()| serde_json::to_string(&before).map(Some).map_err(|error| error.to_string()))
    } else {
        let kept = kept.filter(|kept| still_installed(kept, |program| std::path::Path::new(program).is_file()));
        let before = restored(kept, &wanted);
        let written = before.iter().fold(Ok(()), |result, value| result.and(write(value)));
        before.iter().for_each(|value| remove_empty(&value.key));
        written.map(|()| None)
    };
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
    result
}

#[cfg(test)]
#[path = "windows_folder_handler/tests/values.rs"]
mod tests;
