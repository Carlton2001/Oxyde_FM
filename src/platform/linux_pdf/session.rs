//! Parent side: one worker per thumbnail, and one per Viewer document that keeps it
//! open so that a page far into a long document renders without reloading it.
use super::super::linux_worker::Worker;
use super::{CLOSE_DOCUMENT, DOCUMENT_CLOSED, GRID_SIDE, HOVER_SIDE};
use crate::models::CommandError;
use crate::utils::preview_protocol::{self, Limits, Reply};
use image::RgbaImage;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Private memory of a rendering worker, the Windows job's ceiling.
const MEMORY_BYTES: u64 = 1024 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(15);
const CLOSE_DEADLINE: Duration = Duration::from_secs(2);

fn executable() -> Result<PathBuf, CommandError> {
    Ok(std::env::current_exe()?)
}

fn single_image(reply: Reply) -> Result<RgbaImage, CommandError> {
    match reply {
        Reply::Images { mut frames, .. } if frames.len() == 1 => Ok(frames.remove(0).into_buffer()),
        Reply::Error { detail, .. } => Err(CommandError::Other(detail)),
        _ => Err(CommandError::Other("Unexpected PDF worker reply".into())),
    }
}

/// The first page, 128 px for the grid or 1024 px for hover previews.
pub(in crate::platform) fn thumbnail(path: &Path, grid: bool) -> Result<RgbaImage, CommandError> {
    thumbnail_in(&executable()?, path, grid)
}

fn thumbnail_in(executable: &Path, path: &Path, grid: bool) -> Result<RgbaImage, CommandError> {
    let side = if grid { GRID_SIDE } else { HOVER_SIDE };
    let mut worker = Worker::start(executable, if grid { "pdf-grid" } else { "pdf" }, Some(path), MEMORY_BYTES)?;
    let bytes = worker.read_to_end(preview_protocol::MAX_REPLY, Instant::now() + DEADLINE)?;
    single_image(preview_protocol::decode_with(&bytes, Limits { pixels: (side * side * 4) as usize, side })?)
}

pub(in crate::platform) struct Session {
    worker: Option<Worker>,
    sizes: Vec<(f32, f32)>,
}

impl Session {
    pub(in crate::platform) fn open(path: &Path) -> Result<Self, CommandError> {
        Self::open_in(&executable()?, path)
    }

    fn open_in(executable: &Path, path: &Path) -> Result<Self, CommandError> {
        let worker = Worker::start(executable, "pdf-session", Some(path), MEMORY_BYTES)?;
        let mut session = Self { worker: Some(worker), sizes: Vec::new() };
        let text = match session.reply(DEADLINE)? {
            Reply::Text(text) => text,
            Reply::Error { detail, .. } => return Err(CommandError::Other(detail)),
            _ => return Err(CommandError::Other("Unexpected PDF session reply".into())),
        };
        session.sizes = super::super::thumbnails::parse_pdf_sizes(&text)?;
        Ok(session)
    }

    fn worker(&mut self) -> Result<&mut Worker, CommandError> {
        self.worker.as_mut().ok_or_else(|| CommandError::Other("PDF session is closed".into()))
    }

    /// One length-prefixed frame, giving up when the worker stalls or exits.
    fn reply(&mut self, timeout: Duration) -> Result<Reply, CommandError> {
        let deadline = Instant::now() + timeout;
        let worker = self.worker()?;
        let mut header = [0; 4];
        worker.read_exact(&mut header, deadline)?;
        let length = u32::from_le_bytes(header) as usize;
        if length == 0 || length > preview_protocol::MAX_REPLY {
            return Err(CommandError::Other("Invalid PDF session reply".into()));
        }
        let mut bytes = vec![0; length];
        worker.read_exact(&mut bytes, deadline)?;
        Ok(preview_protocol::decode(&bytes)?)
    }
}

impl super::super::PdfSession for Session {
    fn close(&mut self) -> Result<(), CommandError> {
        if self.worker.is_none() {
            return Ok(());
        }
        let result = (|| {
            self.worker()?.send(&CLOSE_DOCUMENT.to_le_bytes())?;
            match self.reply(CLOSE_DEADLINE)? {
                Reply::Text(text) if text == DOCUMENT_CLOSED => Ok(()),
                _ => Err(CommandError::Other("Missing PDF close receipt".into())),
            }
        })();
        if let Some(worker) = self.worker.take() {
            // A normal exit after the receipt; otherwise the worker is killed.
            if result.is_ok() {
                worker.finish(Duration::from_secs(1));
            }
        }
        result
    }

    fn sizes(&self) -> &[(f32, f32)] {
        &self.sizes
    }

    fn page(&mut self, index: u32) -> Result<RgbaImage, CommandError> {
        if index == CLOSE_DOCUMENT {
            return Err(CommandError::Other("PDF page is out of range".into()));
        }
        let reply = match self.worker()?.send(&index.to_le_bytes()) {
            Ok(()) => self.reply(DEADLINE),
            Err(error) => Err(error.into()),
        };
        if reply.is_err() {
            // A stalled or broken stream cannot be resynchronized; a page error can.
            self.worker = None;
        }
        single_image(reply?)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Also covers an opening result discarded before the Viewer shows it.
        let _ = super::super::PdfSession::close(self);
    }
}

#[cfg(test)]
#[path = "tests/session.rs"]
mod tests;
