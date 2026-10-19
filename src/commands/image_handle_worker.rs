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
pub const THUMBNAIL_FLAG: &str = "--oxyde-thumbnail-handle-worker-v1";
/// Full-resolution Viewer image in its own, larger resource job.
pub const VIEWER_FLAG: &str = "--oxyde-viewer-image-handle-worker-v1";
pub const SESSION_FLAG: &str = "--oxyde-image-session-worker-v1";
pub const SESSION_REQUESTS: u64 = 32;
mod session;
pub fn run_if_requested(
    svg: &dyn Fn(
        &[u8],
        image_preview_decode::Policy,
    ) -> Result<Vec<image::Frame>, image_preview_decode::PreviewError>,
) -> bool {
    let mut arguments = std::env::args_os().skip(1);
    let flag = arguments.next();
    if flag.as_deref() == Some(std::ffi::OsStr::new(SESSION_FLAG)) {
        let result = if arguments.next().is_none() { session::run(svg) } else {
            Err(io::Error::other("Unexpected image session argument"))
        };
        if let Err(error) = result { let _ = writeln!(io::stderr(), "Image session failed: {error}"); }
        let _ = crate::platform::finish_archive_worker_shutdown();
        return true;
    }
    let is = |name: &str| flag.as_deref() == Some(std::ffi::OsStr::new(name));
    let policy = if is(THUMBNAIL_FLAG) {
        image_preview_decode::Policy { output_side: 128, frame_count: 1, ..POLICY }
    } else if is(VIEWER_FLAG) {
        image_preview_decode::VIEWER_POLICY
    } else if is(FLAG) {
        POLICY
    } else {
        return false;
    };
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
        let decoded = image_preview_decode::read_bounded(file, policy.encoded_bytes)
            .and_then(|data| image_preview_decode::decode(&data, policy, svg));
        let reply = match decoded {
            Ok(decoded) => Reply::Images {
                frames: decoded.frames,
                limited: decoded.limited,
                natural_size: decoded.natural_size,
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
