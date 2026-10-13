//! Office text fallback without Shell/COM, paths, caches, or extra capabilities.
use crate::utils::{
    office_preview,
    preview_protocol::{self, Reply},
};
use std::{
    fs::File,
    io::{self, Read, Write},
    os::windows::io::FromRawHandle,
};

pub const FLAG: &str = "--oxyde-office-text-handle-worker-v1";
pub fn run_if_requested() -> bool {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new(FLAG)) {
        return false;
    }
    let result = (|| -> io::Result<()> {
        let mut gate = [0];
        io::stdin().read_exact(&mut gate)?;
        if gate != [0xA7] {
            return Err(io::Error::other("Invalid Office start gate"));
        }
        let extension = arguments
            .next()
            .and_then(|arg| arg.into_string().ok())
            .ok_or_else(|| io::Error::other("Missing Office extension hint"))?;
        if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--source-handle")) {
            return Err(io::Error::other("Missing Office source handle"));
        }
        let handle = arguments
            .next()
            .and_then(|arg| arg.into_string().ok())
            .and_then(|arg| arg.parse::<usize>().ok())
            .filter(|value| *value != 0 && *value != usize::MAX)
            .ok_or_else(|| io::Error::other("Invalid Office source handle"))?;
        if arguments.next().is_some() {
            return Err(io::Error::other("Unexpected Office argument"));
        }
        let file = unsafe { File::from_raw_handle(handle as *mut std::ffi::c_void) };
        let reply = match office_preview::text(file, &extension) {
            Ok(text) => Reply::Text(text),
            Err(error) => Reply::Error {
                limit: false,
                detail: error.to_string(),
            },
        };
        preview_protocol::encode(reply, &mut io::stdout().lock())
    })();
    if let Err(error) = result {
        let _ = writeln!(io::stderr(), "Office text worker failed: {error}");
    }
    let _ = crate::platform::finish_archive_worker_shutdown();
    true
}
