//! HTTPS GET through WinHTTP, which follows redirects and the system proxy.
use std::ffi::c_void;
use std::io;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Networking::WinHttp::*;

struct Handle(*mut c_void);

impl Handle {
    fn new(raw: *mut c_void) -> io::Result<Self> {
        if raw.is_null() { Err(io::Error::last_os_error()) } else { Ok(Self(raw)) }
    }
}

impl Drop for Handle {
    fn drop(&mut self) { unsafe { let _ = WinHttpCloseHandle(self.0); } }
}

/// Download `url` into memory, refusing bodies larger than `limit` bytes.
pub(super) fn download(url: &str, limit: usize) -> io::Result<Vec<u8>> {
    let rest = url.strip_prefix("https://")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Only HTTPS downloads are allowed"))?;
    let (host, path) = rest.split_once('/').map_or((rest, "/".to_owned()), |(host, path)| (host, format!("/{path}")));
    let other = |error: windows::core::Error| io::Error::other(error);
    unsafe {
        let session = Handle::new(WinHttpOpen(&HSTRING::from(concat!("Oxyde/", env!("CARGO_PKG_VERSION"))),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0))?;
        WinHttpSetTimeouts(session.0, 10_000, 10_000, 15_000, 30_000).map_err(other)?;
        let connection = Handle::new(WinHttpConnect(session.0, &HSTRING::from(host), INTERNET_DEFAULT_HTTPS_PORT, 0))?;
        let request = Handle::new(WinHttpOpenRequest(connection.0, &HSTRING::from("GET"), &HSTRING::from(path),
            PCWSTR::null(), PCWSTR::null(), std::ptr::null(), WINHTTP_FLAG_SECURE))?;
        WinHttpSendRequest(request.0, None, None, 0, 0, 0).map_err(other)?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut()).map_err(other)?;
        let mut status = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        WinHttpQueryHeaders(request.0, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER, PCWSTR::null(),
            Some(&mut status as *mut u32 as *mut c_void), &mut size, std::ptr::null_mut()).map_err(other)?;
        if status != 200 {
            return Err(io::Error::other(format!("HTTP status {status}")));
        }
        let mut body = Vec::new();
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            let mut read = 0u32;
            WinHttpReadData(request.0, buffer.as_mut_ptr().cast(), buffer.len() as u32, &mut read).map_err(other)?;
            if read == 0 { return Ok(body); }
            if body.len() + read as usize > limit {
                return Err(io::Error::new(io::ErrorKind::FileTooLarge, "Download exceeds its size limit"));
            }
            body.extend_from_slice(&buffer[..read as usize]);
        }
    }
}
