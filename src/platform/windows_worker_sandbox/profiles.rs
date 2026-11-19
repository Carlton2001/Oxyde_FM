//! AppContainer profiles left behind when Oxyde was killed while preparing a worker:
//! each launch registers `Oxyde.Archive.<uuid>` and removes it about 1.4 s later.
use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{ERROR_SUCCESS, FILETIME};
use windows::Win32::Security::Isolation::DeleteAppContainerProfile;
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    RRF_RT_REG_SZ,
};

const MAPPINGS: PCWSTR = w!(
    r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppContainer\Mappings"
);
/// Windows stores the moniker in lower case.
const PREFIX: &str = "oxyde.archive.";
/// Far older than a launch in progress, in this Oxyde or another one.
const STALE_SECONDS: u64 = 10 * 60;

struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

/// Once per process, before the first worker profile is created.
pub(super) fn sweep_once() {
    static SWEPT: std::sync::Once = std::sync::Once::new();
    SWEPT.call_once(|| {
        for moniker in stale_monikers() {
            let wide: Vec<u16> = moniker.encode_utf16().chain(Some(0)).collect();
            match unsafe { DeleteAppContainerProfile(PCWSTR(wide.as_ptr())) } {
                Ok(()) => log::info!("Removed leftover worker profile {moniker}"),
                Err(error) => log::warn!("Cannot remove leftover worker profile {moniker}: {error}"),
            }
        }
    });
}

fn stale_monikers() -> Vec<String> {
    let mut raw = HKEY::default();
    if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, MAPPINGS, Some(0), KEY_READ, &mut raw) }
        != ERROR_SUCCESS
    {
        return Vec::new();
    }
    let key = Key(raw);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |time| time.as_secs());
    let mut monikers = Vec::new();
    for index in 0.. {
        let mut name = [0u16; 256];
        let mut len = name.len() as u32 - 1;
        let mut written = FILETIME::default();
        let status = unsafe {
            RegEnumKeyExW(
                key.0,
                index,
                Some(PWSTR(name.as_mut_ptr())),
                &mut len,
                None,
                None,
                None,
                Some(&mut written),
            )
        };
        if status != ERROR_SUCCESS {
            break;
        }
        if !stale(written, now) {
            continue;
        }
        if let Some(moniker) = moniker(&key, &name).filter(|moniker| ours(moniker)) {
            monikers.push(moniker);
        }
    }
    monikers
}

fn moniker(key: &Key, subkey: &[u16]) -> Option<String> {
    let mut value = [0u16; 256];
    let mut size = std::mem::size_of_val(&value) as u32;
    let status = unsafe {
        RegGetValueW(
            key.0,
            PCWSTR(subkey.as_ptr()),
            w!("Moniker"),
            RRF_RT_REG_SZ,
            None,
            Some(value.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let units = (size as usize / 2).min(value.len());
    let end = value[..units].iter().position(|&unit| unit == 0).unwrap_or(units);
    String::from_utf16(&value[..end]).ok()
}

fn ours(moniker: &str) -> bool {
    moniker
        .to_ascii_lowercase()
        .strip_prefix(PREFIX)
        .is_some_and(|rest| uuid::Uuid::parse_str(rest).is_ok())
}

fn stale(written: FILETIME, now: u64) -> bool {
    let ticks = (u64::from(written.dwHighDateTime) << 32) | u64::from(written.dwLowDateTime);
    let seconds = ticks.saturating_sub(116_444_736_000_000_000) / 10_000_000;
    now.saturating_sub(seconds) >= STALE_SECONDS
}

#[cfg(test)]
#[path = "tests/profiles.rs"]
mod tests;
