//! File creation entries registered for the Windows Shell's New menu.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::windows::ffi::OsStringExt;
use std::path::{Component, Path, PathBuf};
use windows::Win32::Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CLASSES_ROOT, KEY_READ, REG_ROUTINE_FLAGS, RRF_RT_ANY, RRF_RT_REG_BINARY,
    RRF_RT_REG_SZ, RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW,
};
use windows::Win32::UI::Shell::{
    ASSOCF_NONE, ASSOCSTR_FRIENDLYDOCNAME, AssocQueryStringW, FOLDERID_CommonTemplates,
    FOLDERID_Templates, KF_FLAG_DONT_VERIFY, SHGetKnownFolderPath, SHLoadIndirectString,
};
use windows::core::{GUID, PCWSTR, PWSTR, w};

mod icon_types;
mod package_types;
pub(super) use icon_types::icon_types;

pub struct ShellNewTemplate {
    pub label: String,
    pub extension: String,
    contents: Contents,
}

enum Contents {
    Empty,
    File(PathBuf),
    RegistryData(String),
}

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn open(root: HKEY, name: &str) -> io::Result<Key> {
    let mut key = HKEY::default();
    let result = unsafe {
        RegOpenKeyExW(
            root,
            PCWSTR(wide(name).as_ptr()),
            Some(0),
            KEY_READ,
            &mut key,
        )
    };
    registry_result(result)?;
    Ok(Key(key))
}

fn registry_result(result: WIN32_ERROR) -> io::Result<()> {
    if result == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(result.0 as i32))
    }
}

fn has_value(key: &Key, name: PCWSTR, flags: REG_ROUTINE_FLAGS) -> bool {
    let mut bytes = 0;
    unsafe {
        RegGetValueW(
            key.0,
            PCWSTR::null(),
            name,
            flags,
            None,
            None,
            Some(&mut bytes),
        ) == ERROR_SUCCESS
    }
}

fn string(key: &Key, name: PCWSTR) -> Option<String> {
    // Allow Windows to expand REG_EXPAND_SZ values, returning a REG_SZ string.
    let mut value = [0u16; 1024];
    let mut bytes = size_of_val(&value) as u32;
    let result = unsafe {
        RegGetValueW(
            key.0,
            PCWSTR::null(),
            name,
            RRF_RT_REG_SZ,
            None,
            Some(value.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    };
    (result == ERROR_SUCCESS)
        .then(|| read_string(&value))
        .flatten()
}

fn read_string(buffer: &[u16]) -> Option<String> {
    let length = buffer.iter().position(|&unit| unit == 0)?;
    let text = String::from_utf16(&buffer[..length]).ok()?;
    (!text.trim().is_empty()).then_some(text)
}

fn display_string(text: String) -> Option<String> {
    if !text.starts_with('@') {
        return Some(text);
    }
    let mut value = [0u16; 1024];
    unsafe {
        SHLoadIndirectString(PCWSTR(wide(&text).as_ptr()), &mut value, None).ok()?;
    }
    read_string(&value)
}

fn association_name(extension: &str) -> Option<String> {
    let mut value = [0u16; 1024];
    let mut length = value.len() as u32;
    unsafe {
        AssocQueryStringW(
            ASSOCF_NONE,
            ASSOCSTR_FRIENDLYDOCNAME,
            PCWSTR(wide(extension).as_ptr()),
            PCWSTR::null(),
            Some(PWSTR(value.as_mut_ptr())),
            &mut length,
        )
        .ok()
        .ok()?;
    }
    read_string(&value)
}

/// The built-in folder command has a localized label independent of Oxyde's language.
pub(super) fn folder_menu_label() -> Option<String> {
    let key = open(HKEY_CLASSES_ROOT, r"Folder\ShellNew").ok()?;
    string(&key, w!("MenuText")).and_then(display_string)
}

fn file_name_part(name: &str) -> bool {
    !name.is_empty()
        && !name.ends_with([' ', '.'])
        && !name
            .chars()
            .any(|ch| ch.is_control() || r#"\/:*?"<>|"#.contains(ch))
}

fn registered_extension(name: &str) -> bool {
    name.strip_prefix('.')
        .is_some_and(|suffix| file_name_part(suffix) && !suffix.contains('.'))
}

pub(super) fn templates() -> io::Result<Vec<ShellNewTemplate>> {
    let mut templates: Vec<_> = extensions(HKEY_CLASSES_ROOT)?
        .into_iter()
        .filter_map(|extension| find_template(HKEY_CLASSES_ROOT, extension))
        .collect();
    templates.sort_by_cached_key(|entry| (entry.label.to_lowercase(), entry.extension.clone()));
    Ok(templates)
}

fn extensions(root: HKEY) -> io::Result<Vec<String>> {
    let mut extensions = Vec::new();
    let mut name = [0u16; 256];
    for index in 0.. {
        let mut length = name.len() as u32;
        let result = unsafe {
            RegEnumKeyExW(
                root,
                index,
                Some(PWSTR(name.as_mut_ptr())),
                &mut length,
                None,
                None,
                None,
                None,
            )
        };
        if result == ERROR_NO_MORE_ITEMS {
            break;
        }
        registry_result(result)?;
        if let Ok(extension) = String::from_utf16(&name[..length as usize]) {
            if registered_extension(&extension) {
                extensions.push(extension);
            }
        }
    }
    Ok(extensions)
}

struct Registration {
    location: String,
    key: Key,
    prog_id: Option<String>,
}

fn registration(root: HKEY, extension: &str) -> Option<Registration> {
    let key = open(root, extension).ok()?;
    let prog_id = string(&key, PCWSTR::null()).filter(|name| file_name_part(name));
    let (location, key) = [
        Some(format!(r"{extension}\ShellNew")),
        prog_id
            .as_ref()
            .map(|id| format!(r"{extension}\{id}\ShellNew")),
        prog_id.as_ref().map(|id| format!(r"{id}\ShellNew")),
    ]
    .into_iter()
    .flatten()
    .find_map(|path| open(root, &path).ok().map(|key| (path, key)))?;
    Some(Registration {
        location,
        key,
        prog_id,
    })
}

fn registration_label(root: HKEY, registration: &Registration, extension: &str) -> Option<String> {
    [w!("MenuText"), w!("ItemName")]
        .into_iter()
        .find_map(|name| string(&registration.key, name).and_then(display_string))
        .or_else(|| {
            let key = open(root, registration.prog_id.as_deref()?).ok()?;
            string(&key, w!("FriendlyTypeName"))
                .and_then(display_string)
                .or_else(|| string(&key, PCWSTR::null()).and_then(display_string))
        })
        .or_else(|| association_name(extension))
}

fn find_template(root: HKEY, extension: String) -> Option<ShellNewTemplate> {
    let registration = registration(root, &extension)?;

    // Windows owns the UI and effects of these registrations, including its shortcut wizard.
    // They remain available in Oxyde's native Shell menu.
    if [w!("Command"), w!("Handler"), w!("Directory")]
        .into_iter()
        .any(|value| has_value(&registration.key, value, RRF_RT_ANY))
    {
        return None;
    }
    let contents = if has_value(&registration.key, w!("NullFile"), RRF_RT_ANY) {
        Contents::Empty
    } else if let Some(path) = string(&registration.key, w!("FileName")) {
        Contents::File(PathBuf::from(path))
    } else if has_value(&registration.key, w!("Data"), RRF_RT_REG_BINARY) {
        Contents::RegistryData(registration.location.clone())
    } else {
        return None;
    };
    let label =
        registration_label(root, &registration, &extension).unwrap_or_else(|| extension.clone());
    Some(ShellNewTemplate {
        label,
        extension,
        contents,
    })
}

pub(super) fn create_file(
    folder: &Path,
    template: &ShellNewTemplate,
    names: &mut dyn Iterator<Item = String>,
) -> io::Result<PathBuf> {
    if !folder.is_absolute() || !registered_extension(&template.extension) {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    // Stage populated files once in the destination directory. A failed copy, or a
    // name taken between clicks, must never expose a partial file or overwrite it.
    let mut staged = match &template.contents {
        Contents::Empty => None,
        contents => {
            let mut file = tempfile::NamedTempFile::new_in(folder)?;
            match contents {
                Contents::Empty => unreachable!(),
                Contents::File(path) => {
                    io::copy(&mut open_template(path)?, file.as_file_mut())?;
                }
                Contents::RegistryData(key) => {
                    file.write_all(&registry_data(HKEY_CLASSES_ROOT, key)?)?
                }
            }
            Some(file)
        }
    };
    for stem in names {
        if !file_name_part(&stem) {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        let path = folder.join(format!("{stem}{}", template.extension));
        if let Some(file) = staged.take() {
            match file.persist_noclobber(&path) {
                Ok(_) => return Ok(path),
                Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
                    staged = Some(error.file)
                }
                Err(error) => return Err(error.error),
            }
        } else {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
                Err(error) => return Err(error),
            }
        }
    }
    Err(io::Error::from(io::ErrorKind::AlreadyExists))
}

fn open_template(path: &Path) -> io::Result<File> {
    if path.is_absolute() {
        return File::open(path);
    }
    if !path
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    let definitions = [FOLDERID_Templates, FOLDERID_CommonTemplates];
    let directories = definitions
        .iter()
        .filter_map(known_folder)
        .chain(std::env::var_os("SystemRoot").map(|root| PathBuf::from(root).join("ShellNew")));
    for root in directories {
        match File::open(root.join(path)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            result => return result,
        }
    }
    Err(io::Error::from(io::ErrorKind::NotFound))
}

fn known_folder(id: &GUID) -> Option<PathBuf> {
    unsafe {
        let path = SHGetKnownFolderPath(id, KF_FLAG_DONT_VERIFY, None).ok()?;
        let _memory = super::windows_com::TaskMemory::new(path.0);
        Some(PathBuf::from(std::ffi::OsString::from_wide(path.as_wide())))
    }
}

fn registry_data(root: HKEY, path: &str) -> io::Result<Vec<u8>> {
    let key = open(root, path)?;
    let mut bytes = 0;
    registry_result(unsafe {
        RegGetValueW(
            key.0,
            PCWSTR::null(),
            w!("Data"),
            RRF_RT_REG_BINARY,
            None,
            None,
            Some(&mut bytes),
        )
    })?;
    let mut data = vec![0; bytes as usize];
    registry_result(unsafe {
        RegGetValueW(
            key.0,
            PCWSTR::null(),
            w!("Data"),
            RRF_RT_REG_BINARY,
            None,
            Some(data.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    })?;
    data.truncate(bytes as usize);
    Ok(data)
}

#[cfg(test)]
#[path = "windows_shell_new/tests/templates.rs"]
mod tests;
