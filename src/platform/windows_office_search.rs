//! Search-only LPAC admission, independent of the interactive Office previews.
use crate::utils::{
    office_document::search,
    preview_protocol::{self, Reply},
};
use std::{
    io,
    path::Path,
    process::Command,
    sync::{Mutex, TryLockError},
    time::{Duration, Instant},
};

static SLOT: Mutex<()> = Mutex::new(());
const DEADLINE: Duration = Duration::from_secs(15);

pub(super) fn run(path: &Path, checkpoint: &dyn Fn() -> io::Result<()>) -> io::Result<String> {
    let started = Instant::now();
    let _slot = loop {
        checkpoint()?;
        if started.elapsed() >= DEADLINE {
            return Err(io::ErrorKind::TimedOut.into());
        }
        match SLOT.try_lock() {
            Ok(slot) => break slot,
            Err(TryLockError::Poisoned(_)) => {
                return Err(io::Error::other("Office search admission unavailable"))
            }
            Err(TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(10)),
        }
    };
    run_in(
        &std::env::current_exe()?.with_file_name("oxyde-image-worker.exe"),
        path,
        DEADLINE.saturating_sub(started.elapsed()),
        checkpoint,
    )
}

pub(super) fn run_in(
    executable: &Path,
    path: &Path,
    deadline: Duration,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> io::Result<String> {
    checkpoint()?;
    let source = super::platform().retain_archive_source(path)?;
    if source.file.metadata()?.len() > search::MAX_SOURCE {
        return Err(io::ErrorKind::FileTooLarge.into());
    }
    let mut command = Command::new(executable);
    command
        .arg(crate::commands::office_handle_worker::SEARCH_FLAG)
        .arg(
            path.extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or_default(),
        );
    let bytes = super::windows_preview_worker::supervise_source(
        command,
        &source.file,
        deadline,
        512 * 1024 * 1024,
        search::MAX_TEXT + 4096,
        checkpoint,
    )?;
    checkpoint()?;
    match preview_protocol::decode(&bytes)? {
        Reply::Text(text) if text.len() <= search::MAX_TEXT => Ok(text),
        Reply::Error { limit, detail } => Err(io::Error::new(
            if limit {
                io::ErrorKind::FileTooLarge
            } else {
                io::ErrorKind::InvalidData
            },
            detail,
        )),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid Office search reply",
        )),
    }
}
