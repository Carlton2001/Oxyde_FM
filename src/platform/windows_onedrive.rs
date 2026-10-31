use crate::models::QuickAccessItem;
use windows::core::{w, PCWSTR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_READ, REG_SZ, REG_VALUE_TYPE,
};

pub(super) struct Key(pub(super) HKEY);
impl Drop for Key {
    fn drop(&mut self) { unsafe { let _ = RegCloseKey(self.0); } }
}
pub(super) fn open(parent: HKEY, name: &str) -> Option<Key> {
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let mut key = HKEY::default();
    unsafe {
        (RegOpenKeyExW(parent, PCWSTR(wide.as_ptr()), Some(0), KEY_READ, &mut key).0 == 0)
            .then_some(Key(key))
    }
}
pub(super) fn subkey_names(key: HKEY) -> Vec<String> {
    let mut names = Vec::new();
    for index in 0..128 {
        let mut name = [0u16; 256];
        let mut len = name.len() as u32;
        unsafe {
            if RegEnumKeyExW(key, index, Some(windows::core::PWSTR(name.as_mut_ptr())),
                &mut len, None, None, None, None).0 != 0 {
                break;
            }
        }
        if let Ok(name) = String::from_utf16(&name[..len as usize]) { names.push(name); }
    }
    names
}
pub(super) fn string(key: HKEY, name: PCWSTR) -> Option<String> {
    let mut data = [0u16; 32768];
    let mut bytes = (data.len() * 2) as u32;
    let mut kind = REG_VALUE_TYPE::default();
    unsafe {
        if RegQueryValueExW(key, name, None, Some(&mut kind),
            Some(data.as_mut_ptr().cast()), Some(&mut bytes)).0 != 0 {
            return None;
        }
    }
    if kind != REG_SZ || bytes < 2 || bytes as usize > data.len() * 2 || !bytes.is_multiple_of(2) {
        return None;
    }
    let value = String::from_utf16(&data[..bytes as usize / 2]).ok()?;
    let value = value.trim_end_matches('\0');
    (!value.is_empty()).then(|| value.to_owned())
}

/// Read configured accounts, never guess a folder from its name or create one.
/// Registry and filesystem reads run in the quick-access background task.
pub(super) fn folders() -> Vec<QuickAccessItem> {
    if !installed() { return Vec::new(); }
    let Some(accounts) = open(HKEY_CURRENT_USER, r"Software\Microsoft\OneDrive\Accounts") else {
        return Vec::new();
    };
    let mut roots = Vec::new();
    for name in subkey_names(accounts.0) {
        let Some(account) = open(accounts.0, &name) else { continue };
        let Some(path) = string(account.0, w!("UserFolder")) else { continue };
        if !std::path::Path::new(&path).is_absolute() || !std::path::Path::new(&path).is_dir() {
            continue;
        }
        if roots.iter().any(|item: &QuickAccessItem| item.path.eq_ignore_ascii_case(&path)) {
            continue;
        }
        let label = string(account.0, w!("DisplayName"))
            .map(|name| format!("OneDrive — {name}"))
            .unwrap_or_else(|| "OneDrive".into());
        roots.push(QuickAccessItem { name: label, path });
    }
    roots.sort_by(|a, b| a.path.cmp(&b.path));
    roots
}


/// Detect the client independently of configured accounts or sidebar preferences.
pub(super) fn installed() -> bool {
    // Account registrations can survive an uninstall. Require the client too.
     ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"].iter().any(|key| {
        std::env::var_os(key).is_some_and(|base| {
            let directory = if *key == "LOCALAPPDATA" {
                std::path::PathBuf::from(base).join("Microsoft").join("OneDrive")
            } else {
                std::path::PathBuf::from(base).join("Microsoft OneDrive")
            };
            directory.join("OneDrive.exe").is_file()
        })
    })

}
