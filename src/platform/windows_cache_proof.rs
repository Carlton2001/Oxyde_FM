//! User-bound DPAPI integrity for the trusted parent's persistent cache ledger.
//! A process with the user's full credentials is outside this proof contract.
use std::io;
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{LocalFree, HLOCAL},
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    },
};
const MAX_BYTES: usize = 4 * 1024 * 1024;
pub(super) mod key;
pub(super) fn transform(bytes: &[u8], key: &[u8; 32], protect: bool) -> io::Result<Vec<u8>> {
    const MAGIC: &[u8; 8] = b"OXCPMAC1";
    if protect {
        let encrypted = dpapi(bytes, true)?;
        let mut result = Vec::with_capacity(40 + encrypted.len());
        result.extend_from_slice(MAGIC);
        result.extend_from_slice(blake3::keyed_hash(key, &encrypted).as_bytes());
        result.extend_from_slice(&encrypted);
        Ok(result)
    } else {
        if bytes.len() < 40 || bytes.len() > MAX_BYTES || &bytes[..8] != MAGIC { return Err(io::Error::other("Invalid cache proof envelope")); }
        let expected = blake3::Hash::from_bytes(bytes[8..40].try_into().unwrap());
        if blake3::keyed_hash(key, &bytes[40..]) != expected { return Err(io::Error::other("Cache proof authentication failed")); }
        dpapi(&bytes[40..], false)
    }
}
fn dpapi(bytes: &[u8], protect: bool) -> io::Result<Vec<u8>> {
    if bytes.len() > MAX_BYTES {
        return Err(io::Error::other("Cache proof exceeds its size limit"));
    }
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let entropy_bytes = b"Oxyde archive cache ownership v4";
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: entropy_bytes.len() as u32,
        pbData: entropy_bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        if protect {
            CryptProtectData(
                &input,
                PCWSTR::null(),
                Some(&entropy),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                None,
                Some(&entropy),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    }
    .map_err(|error| io::Error::other(error.to_string()))?;
    struct Allocation(*mut u8);
    impl Drop for Allocation {
        fn drop(&mut self) {
            unsafe {
                LocalFree(Some(HLOCAL(self.0.cast())));
            }
        }
    }
    let _allocation = Allocation(output.pbData);
    if output.cbData as usize > MAX_BYTES || (output.cbData != 0 && output.pbData.is_null()) {
        return Err(io::Error::other("Invalid native cache proof"));
    }
    if output.cbData == 0 {
        return Ok(Vec::new());
    }
    Ok(unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec())
}

#[cfg(test)]
#[path = "windows_cache_proof/tests/authentication.rs"]
mod tests;
