//! Images, Office documents and audio tags decoded in a confined worker, as glycin
//! loads images for Nautilus: a hostile file only reaches that process. One worker per
//! request, so a compromised decoder never sees the next document.
use super::linux_worker::Worker;
use super::thumbnails;
use crate::models::CommandError;
use crate::utils::office_document::search;
use crate::utils::{audio_metadata, office_preview};
use crate::utils::preview_protocol::{self, Reply};
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::{Mutex, TryLockError};
use std::time::{Duration, Instant};

const MEMORY_BYTES: u64 = 512 * 1024 * 1024;
const VIEWER_MEMORY_BYTES: u64 = 1024 * 1024 * 1024;
const COVER_MEMORY_BYTES: u64 = 256 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(15);
const VIEWER_DEADLINE: Duration = Duration::from_secs(30);
/// Content search does not wait behind itself for longer than one document may take.
static SEARCH_SLOT: Mutex<()> = Mutex::new(());

pub(super) fn run(mode: &str, path: &Path) -> io::Result<Vec<u8>> {
    run_in(&std::env::current_exe()?, mode, path)
}

fn run_in(executable: &Path, mode: &str, path: &Path) -> io::Result<Vec<u8>> {
    let (memory, deadline, limit) = match mode {
        "image-viewer" => (VIEWER_MEMORY_BYTES, VIEWER_DEADLINE, preview_protocol::VIEWER_MAX_REPLY),
        "audio-cover" | "audio-cover-thumbnail" => (COVER_MEMORY_BYTES, DEADLINE, preview_protocol::MAX_REPLY),
        "audio-metadata" => (MEMORY_BYTES, DEADLINE, audio_metadata::MAX_REPLY),
        _ => (MEMORY_BYTES, DEADLINE, preview_protocol::MAX_REPLY),
    };
    let mut worker = Worker::start(executable, mode, Some(path), memory)?;
    worker.read_to_end(limit, Instant::now() + deadline).map_err(|error| match error.kind() {
        // The interface recognizes this prefix to tell a timeout from a failure.
        io::ErrorKind::TimedOut => io::Error::new(io::ErrorKind::TimedOut, "PreviewWorkerTimeout: Preview exceeded its deadline"),
        _ => error,
    })
}

pub(super) fn office_search(path: &Path, checkpoint: &dyn Fn() -> io::Result<()>) -> io::Result<String> {
    let started = Instant::now();
    let _slot = loop {
        checkpoint()?;
        if started.elapsed() >= DEADLINE {
            return Err(io::ErrorKind::TimedOut.into());
        }
        match SEARCH_SLOT.try_lock() {
            Ok(slot) => break slot,
            Err(TryLockError::Poisoned(_)) => return Err(io::Error::other("Office search admission unavailable")),
            Err(TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(10)),
        }
    };
    office_search_in(&std::env::current_exe()?, path, DEADLINE.saturating_sub(started.elapsed()), checkpoint)
}

fn office_search_in(executable: &Path, path: &Path, deadline: Duration, checkpoint: &dyn Fn() -> io::Result<()>) -> io::Result<String> {
    checkpoint()?;
    if std::fs::metadata(path)?.len() > search::MAX_SOURCE {
        return Err(io::ErrorKind::FileTooLarge.into());
    }
    let mut worker = Worker::start(executable, "office-search", Some(path), MEMORY_BYTES)?;
    let bytes = worker.read_to_end_checked(search::MAX_TEXT + 4096, Instant::now() + deadline, checkpoint)?;
    checkpoint()?;
    match preview_protocol::decode(&bytes)? {
        Reply::Text(text) if text.len() <= search::MAX_TEXT => Ok(text),
        Reply::Error { limit, detail } => Err(io::Error::new(
            if limit { io::ErrorKind::FileTooLarge } else { io::ErrorKind::InvalidData },
            detail,
        )),
        _ => Err(io::Error::other("Unexpected Office search reply")),
    }
}

/// Worker side. Every mode reads only the document it was given.
pub(super) fn reply(mode: &str, path: &Path) -> Reply {
    let text = |result: Result<String, CommandError>| result.map_or_else(failure, Reply::Text);
    let image = |result: Result<image::RgbaImage, CommandError>| result.map_or_else(failure, one_frame);
    let extension = path.extension().and_then(|extension| extension.to_str()).unwrap_or_default();
    let name = || path.to_str().map(str::to_owned).ok_or_else(|| CommandError::Other("Preview path is not Unicode".into()));
    match mode {
        "thumbnail" => image(name().and_then(thumbnails::get_thumbnail_uncached)),
        "office" => image(name().and_then(thumbnails::get_office_thumbnail_uncached)),
        "office-text" => text(name().and_then(thumbnails::get_office_text_preview_local)),
        "office-document" => text(File::open(path).map_err(Into::into).and_then(|file| office_preview::document(file, extension))),
        "office-search" => office_search_reply(path, extension),
        "audio-cover" | "audio-cover-thumbnail" => cover_reply(path, mode == "audio-cover-thumbnail"),
        _ => super::linux_pdf::decode(mode, path),
    }
}

fn failure(error: CommandError) -> Reply {
    Reply::Error { limit: false, detail: error.to_string() }
}

fn one_frame(image: image::RgbaImage) -> Reply {
    Reply::Images { frames: vec![image::Frame::new(image)], limited: false, natural_size: None }
}

fn office_search_reply(path: &Path, extension: &str) -> Reply {
    let started = Instant::now();
    let parsed = File::open(path).and_then(|file| search::extract(file, extension, &|| {
        if started.elapsed() >= DEADLINE {
            return Err(io::ErrorKind::TimedOut.into());
        }
        Ok(())
    }));
    match parsed {
        Ok(text) => Reply::Text(text),
        Err(error) => Reply::Error { limit: error.kind() == io::ErrorKind::FileTooLarge, detail: error.to_string() },
    }
}

/// An empty text says that a valid file has no cover.
fn cover_reply(path: &Path, thumbnail: bool) -> Reply {
    let cover = File::open(path).and_then(|file| audio_metadata::read_cover(file, thumbnail));
    match cover {
        Ok(None) => Reply::Text(String::new()),
        Ok(Some(cover)) => match image::RgbaImage::from_raw(cover.width, cover.height, cover.pixels) {
            Some(image) => one_frame(image),
            None => Reply::Error { limit: false, detail: "Invalid audio cover pixels".into() },
        },
        Err(error) => Reply::Error { limit: false, detail: error.to_string() },
    }
}

#[cfg(test)]
#[path = "linux_preview/tests/workers.rs"]
mod tests;
