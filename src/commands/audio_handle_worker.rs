//! Audio metadata and cover decoding from a single inherited source handle.
use crate::utils::audio_metadata;
use std::{
    fs::File,
    io::{self, Read, Write},
    os::windows::io::FromRawHandle,
};

pub const FLAG: &str = "--oxyde-audio-metadata-handle-worker-v1";
pub fn run_if_requested() -> bool {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new(FLAG)) {
        return false;
    }
    let result = (|| -> io::Result<()> {
        let mut gate = [0];
        io::stdin().read_exact(&mut gate)?;
        if gate != [0xA7] {
            return Err(io::Error::other("Invalid Audio start gate"));
        }
        if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--source-handle")) {
            return Err(io::Error::other("Missing Audio source handle"));
        }
        let handle = arguments
            .next()
            .and_then(|arg| arg.into_string().ok())
            .and_then(|arg| arg.parse::<usize>().ok())
            .filter(|value| *value != 0 && *value != usize::MAX)
            .ok_or_else(|| io::Error::other("Invalid Audio source handle"))?;
        if arguments.next().is_some() {
            return Err(io::Error::other("Unexpected Audio argument"));
        }
        let file = unsafe { File::from_raw_handle(handle as *mut std::ffi::c_void) };
        let metadata = audio_metadata::read(file)?;
        let mut output = io::stdout().lock();
        serde_json::to_writer(&mut output, &metadata).map_err(io::Error::other)?;
        // Native worker shutdown bypasses destructors; publish the buffered tail first.
        output.flush()
    })();
    if let Err(error) = result {
        let _ = writeln!(io::stderr(), "Audio metadata worker failed: {error}");
    }
    let _ = crate::platform::finish_archive_worker_shutdown();
    true
}
