//! Metadata parser with one inherited read-only file and three IPC handles.
use crate::models::{CommandError, FileEntry};
use crate::utils::{
    archive::ArchiveFormat, archive_limits::ArchiveLimits, archive_listing_protocol,
    archive_security::relative_archive_path,
};
use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

pub const FLAG: &str = "--oxyde-archive-handle-worker-v1";

fn indexed(path: &Path, file: File) -> Result<Vec<FileEntry>, CommandError> {
    match ArchiveFormat::from_path(path) {
        Some(
            ArchiveFormat::Zip
            | ArchiveFormat::Tar
            | ArchiveFormat::TarGz
            | ArchiveFormat::TarXz
            | ArchiveFormat::TarZst
            | ArchiveFormat::TarBz2,
        ) => super::archive_metadata::indexed_file(path, file, ArchiveLimits::LISTING, &|| Ok(())),
        Some(ArchiveFormat::SevenZip) => {
            super::archive_seven_open::indexed_file(path, file, ArchiveLimits::LISTING, &|| Ok(()))
        }
        Some(ArchiveFormat::Iso) => super::archive_iso_open::indexed_file(path, file, &|| Ok(())),
        _ => Err(CommandError::ArchiveError(
            "Unsupported archive metadata format".into(),
        )),
    }
}

fn directory(
    path: &Path,
    internal: &str,
    entries: Vec<FileEntry>,
) -> Result<Vec<FileEntry>, CommandError> {
    let prefix = relative_archive_path(internal)?;
    let mut seen = std::collections::HashSet::new();
    let mut files = Vec::new();
    for mut entry in entries {
        let relative = Path::new(&entry.path)
            .strip_prefix(path)
            .map_err(|_| io::Error::other("Invalid archive entry"))?;
        let Ok(relative) = relative.strip_prefix(&prefix) else {
            continue;
        };
        let mut parts = relative.components();
        let Some(first) = parts.next() else {
            continue;
        };
        let name = first.as_os_str().to_string_lossy().into_owned();
        if !seen.insert(name.clone()) {
            continue;
        }
        entry.is_dir |= parts.next().is_some();
        if entry.is_dir {
            entry.size = 0;
        }
        entry.path = path
            .join(&prefix)
            .join(&name)
            .to_string_lossy()
            .into_owned();
        entry.name = name;
        files.push(entry);
    }
    Ok(files)
}

#[cfg(target_os = "windows")]
pub fn run_if_requested() -> bool {
    use std::os::windows::io::FromRawHandle;
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new(FLAG)) {
        return false;
    }
    let result = (|| -> io::Result<()> {
        let mut gate = [0];
        io::stdin().read_exact(&mut gate)?;
        if gate != [0xA7] {
            return Err(io::Error::other("Invalid metadata start gate"));
        }
        let mode = arguments
            .next()
            .ok_or_else(|| io::Error::other("Missing metadata mode"))?;
        let archive = arguments
            .next()
            .ok_or_else(|| io::Error::other("Missing logical archive path"))?;
        let internal = arguments
            .next()
            .and_then(|arg| arg.into_string().ok())
            .ok_or_else(|| io::Error::other("Missing archive directory"))?;
        if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--source-handle")) {
            return Err(io::Error::other("Missing metadata source handle"));
        }
        let handle = arguments
            .next()
            .and_then(|arg| arg.into_string().ok())
            .and_then(|arg| arg.parse::<usize>().ok())
            .filter(|handle| *handle != 0 && *handle != usize::MAX)
            .ok_or_else(|| io::Error::other("Invalid metadata source handle"))?;
        if arguments.next().is_some() {
            return Err(io::Error::other("Unexpected metadata argument"));
        }
        let file = unsafe { File::from_raw_handle(handle as *mut std::ffi::c_void) };
        let path = Path::new(&archive);
        let entries = indexed(path, file);
        let mut output = io::stdout().lock();
        let encoded = if mode == "index" {
            archive_listing_protocol::encode_index(entries, path, &mut output)
        } else if mode == "directory" {
            archive_listing_protocol::encode(
                entries.and_then(|entries| directory(path, &internal, entries)),
                &mut output,
            )
        } else {
            return Err(io::Error::other("Invalid metadata mode"));
        };
        if let Err(error) = encoded {
            let failed = Err(CommandError::ArchiveError(error.to_string()));
            if mode == "index" {
                archive_listing_protocol::encode_index(failed, path, &mut output)?;
            } else {
                archive_listing_protocol::encode(failed, &mut output)?;
            }
        }
        output.flush()
    })();
    if let Err(error) = result {
        let _ = writeln!(io::stderr(), "Archive metadata worker failed: {error}");
    }
    let _ = crate::platform::finish_archive_worker_shutdown();
    true
}
