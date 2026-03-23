use windows::Win32::System::Registry::{RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, REG_DWORD};
use windows::core::PCWSTR;

fn main() {
    let mut hkey = windows::Win32::System::Registry::HKEY::default();
    let wide_key: Vec<u16> = "Software".encode_utf16().chain(std::iter::once(0)).collect();
    let res = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(wide_key.as_ptr()), 0, KEY_READ, &mut hkey) };
    if res.is_ok() {
        println!("Success");
    } else {
        println!("Fail");
    }
}
