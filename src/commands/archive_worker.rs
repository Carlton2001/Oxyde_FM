//! Headless listing only: no extraction, caches, preferences, logging or GPUI.
use crate::utils::archive_listing_protocol;
use std::io::{Read, Write};

pub fn run_if_requested() -> bool {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--oxyde-archive-list-worker-v1")) {
        return false;
    }
    let result = (|| -> std::io::Result<()> {
        let mut gate = [0];
        std::io::stdin().read_exact(&mut gate)?;
        if gate != [0xA7] {
            return Err(std::io::Error::other("Invalid archive start gate"));
        }
        let archive = arguments
            .next()
            .and_then(|value| value.into_string().ok())
            .ok_or_else(|| std::io::Error::other("Missing archive source"))?;
        let internal = arguments
            .next()
            .and_then(|value| value.into_string().ok())
            .ok_or_else(|| std::io::Error::other("Missing archive directory"))?;
        if arguments.next().is_some() {
            return Err(std::io::Error::other("Unexpected archive argument"));
        }
        let result = super::archive::list_archive_contents_local(archive, internal);
        let mut output = std::io::stdout().lock();
        if let Err(error) = archive_listing_protocol::encode(result, &mut output) {
            // Encoding validates before emitting bytes; a write failure will
            // produce a malformed response rejected by the supervising parent.
            archive_listing_protocol::encode(
                Err(crate::models::CommandError::ArchiveError(error.to_string())),
                &mut output,
            )?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = writeln!(std::io::stderr(), "Archive worker failed: {error}");
    }
    // stdout was flushed. No application cleanup or user state exists here.
    let _ = crate::platform::platform().finish_process_shutdown();
    true
}
