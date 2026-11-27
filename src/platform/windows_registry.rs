//! Read-only registry helpers.
use windows::Win32::System::Registry::{
    HKEY, KEY_ENUMERATE_SUB_KEYS, KEY_QUERY_VALUE, REG_SZ, REG_VALUE_TYPE, RegCloseKey,
    RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW,
};
use windows::core::{PCWSTR, PWSTR};

pub(super) struct Key(pub(super) HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}
pub(super) fn open(parent: HKEY, name: &str) -> Option<Key> {
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let mut key = HKEY::default();
    unsafe {
        (RegOpenKeyExW(
            parent,
            PCWSTR(wide.as_ptr()),
            Some(0),
            KEY_QUERY_VALUE | KEY_ENUMERATE_SUB_KEYS,
            &mut key,
        )
        .0 == 0)
            .then_some(Key(key))
    }
}
pub(super) fn subkey_names(key: HKEY) -> Vec<String> {
    let mut names = Vec::new();
    for index in 0.. {
        let mut name = [0u16; 256];
        let mut len = name.len() as u32;
        unsafe {
            if RegEnumKeyExW(
                key,
                index,
                Some(PWSTR(name.as_mut_ptr())),
                &mut len,
                None,
                None,
                None,
                None,
            )
            .0 != 0
            {
                break;
            }
        }
        if let Ok(name) = String::from_utf16(&name[..len as usize]) {
            names.push(name);
        }
    }
    names
}
pub(super) fn string(key: HKEY, name: PCWSTR) -> Option<String> {
    let mut data = [0u16; 32768];
    let mut bytes = (data.len() * 2) as u32;
    let mut kind = REG_VALUE_TYPE::default();
    unsafe {
        if RegQueryValueExW(
            key,
            name,
            None,
            Some(&mut kind),
            Some(data.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
        .0 != 0
        {
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
