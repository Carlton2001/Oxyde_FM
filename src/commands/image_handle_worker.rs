//! A read-only input handle is the image worker's only document authority.
use crate::utils::{
    image_preview_decode::{self, POLICY},
    preview_protocol::{self, Reply},
};
use std::{
    fs::File,
    io::{self, Read, Write},
    os::windows::io::FromRawHandle,
};

pub const FLAG: &str = "--oxyde-image-handle-worker-v1";
pub fn run_if_requested(
    svg: &dyn Fn(
        &[u8],
        image_preview_decode::Policy,
    ) -> Result<Vec<image::Frame>, image_preview_decode::PreviewError>,
) -> bool {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new(FLAG)) {
        return false;
    }
    let result = (|| -> io::Result<()> {
        let mut gate = [0];
        io::stdin().read_exact(&mut gate)?;
        if gate != [0xA7] {
            return Err(io::Error::other("Invalid image start gate"));
        }
        if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--source-handle")) {
            return Err(io::Error::other("Missing image source handle"));
        }
        let handle = arguments
            .next()
            .and_then(|arg| arg.into_string().ok())
            .and_then(|arg| arg.parse::<usize>().ok())
            .filter(|value| *value != 0 && *value != usize::MAX)
            .ok_or_else(|| io::Error::other("Invalid image source handle"))?;
        if arguments.next().is_some() {
            return Err(io::Error::other("Unexpected image argument"));
        }
        let file = unsafe { File::from_raw_handle(handle as *mut std::ffi::c_void) };
        let decoded = image_preview_decode::read_bounded(file, POLICY.encoded_bytes)
            .and_then(|data| image_preview_decode::decode(&data, POLICY, svg));
        let reply = match decoded {
            Ok(decoded) => Reply::Images {
                frames: decoded.frames,
                limited: decoded.limited,
            },
            Err(error) => Reply::Error {
                limit: matches!(error, image_preview_decode::PreviewError::Limit(_)),
                detail: error.to_string(),
            },
        };
        preview_protocol::encode(reply, &mut io::stdout().lock())
    })();
    if let Err(error) = result {
        let _ = writeln!(io::stderr(), "Image worker failed: {error}");
    }
    let _ = crate::platform::finish_archive_worker_shutdown();
    true
}
