//! RAR (1.3 to 7), read only, through `rars`: listing, search index, password check and
//! decoding. Runs only in confined workers; links refused. A split archive is decoded
//! by the broker, which reads its volumes as sources 0, 1, 2…
use crate::models::error::ArchivePasswordFailure;
use crate::models::{CommandError, FileEntry};
use crate::utils::archive_io_protocol::{invalid, Entry, Frame, Message, Timestamp, MAX_BLOCK};
use crate::utils::archive_io_stream::{Connection, Exchange, Reader, WindowReader};
use crate::utils::archive_limits::{ArchiveBudget, ArchiveLimits};
use crate::utils::archive_password::{self, ArchiveSecret};
use crate::utils::archive_security::relative_archive_path;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Header bytes a listing may parse: names and extras, not payloads.
const MAX_HEADER_BYTES: u64 = 64 * 1024 * 1024;

fn options<'a>(secret: Option<&'a ArchiveSecret>, limits: ArchiveLimits) -> rars::ArchiveReadOptions<'a> {
    let mut options = rars::ArchiveReadOptions::with_optional_password(secret.map(|secret| secret.expose().as_bytes()))
        // Main, end and service headers come beside the members.
        .with_max_header_count((limits.max_entries as u64).saturating_mul(2).saturating_add(64))
        .with_max_header_bytes(MAX_HEADER_BYTES)
        .with_max_member_output_bytes(limits.max_bytes)
        .with_max_total_output_bytes(limits.max_bytes)
        .with_max_reader_workspace_bytes(512 * 1024 * 1024)
        // RAR 7 allows 64 GiB dictionaries; WinRAR's default is 32 MiB, its maximum on 32 bits 1 GiB.
        .with_rar50_dictionary_size_limit(1024 * 1024 * 1024);
    options.legacy_name_encoding = None;
    options
}

/// A `rars` error as Oxyde's: password refusals typed, I/O kept with its marker.
pub(super) fn failure(error: rars::Error, tried: bool) -> CommandError {
    match error.kind() {
        rars::ErrorKind::PasswordRequired => ArchivePasswordFailure::Required.into(),
        rars::ErrorKind::BadPassword if tried => ArchivePasswordFailure::Wrong.into(),
        rars::ErrorKind::BadPassword => ArchivePasswordFailure::Required.into(),
        _ => match error.root_cause() {
            rars::Error::Io(io) => io::Error::new(io.kind, io.message.clone()).into(),
            _ => CommandError::ArchiveError(format!("Invalid RAR archive: {error}")),
        },
    }
}

fn parse<R: Read + Seek + Send + 'static>(
    reader: R,
    secret: Option<&ArchiveSecret>,
    limits: ArchiveLimits,
) -> Result<rars::Archive, CommandError> {
    rars::ArchiveReader::read_reader_with_options(reader, options(secret, limits))
        .map_err(|error| failure(error, secret.is_some()))
}

/// The member's path inside the archive, with `/` separators, or a refusal.
/// Out of a set, a member continued in another volume cannot be read.
fn member_name(meta: &rars::ArchiveMemberMeta, set: bool) -> Result<String, CommandError> {
    if meta.is_redirection {
        return Err(CommandError::ArchiveError("Archive links and special files are not supported".into()));
    }
    if !set && (meta.is_split_before || meta.is_split_after) {
        return Err(CommandError::ArchiveError("Another volume of this RAR archive is missing".into()));
    }
    let relative = rars::entry_relative_path(meta.name_bytes())
        .map_err(|error| CommandError::ArchiveError(format!("Invalid RAR member name: {error}")))?;
    let name = relative.to_string_lossy().replace('\\', "/");
    relative_archive_path(&name)?;
    Ok(name)
}

/// Every member, for navigation and name search. Encrypted headers need the password.
pub(super) fn indexed_file(
    path: &Path,
    file: File,
    limits: ArchiveLimits,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<Vec<FileEntry>, CommandError> {
    let budget = ArchiveBudget::new(limits);
    budget.checkpoint(checkpoint)?;
    let secret = archive_password::for_archive(path);
    let archive = parse(BufReader::new(file), secret.as_ref(), limits)?;
    let origin = Arc::new(path.to_path_buf());
    let mut files = Vec::new();
    for member in archive.members() {
        budget.checkpoint(checkpoint)?;
        let meta = &member.meta;
        let name = member_name(meta, false)?;
        budget.entry(&name, 0)?;
        let relative = relative_archive_path(&name)?;
        let Some(file_name) = relative.file_name() else { continue };
        let modified = meta
            .modification_time()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |time| time.as_millis() as u64);
        files.push(FileEntry {
            archive_path: Some(origin.clone()),
            name: file_name.to_string_lossy().into_owned(),
            path: origin.join(&relative).to_string_lossy().into_owned(),
            is_dir: meta.is_directory,
            size: if meta.is_directory { 0 } else { meta.unpacked_size },
            modified,
            ..FileEntry::default()
        });
    }
    budget.check_entry_count(files.len())?;
    Ok(files)
}

/// Stops a sample once enough bytes decoded to judge the key.
#[derive(Debug)]
struct Enough;
impl std::fmt::Display for Enough {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Sample complete")
    }
}
impl std::error::Error for Enough {}

struct Sample {
    left: u64,
}
impl Write for Sample {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.left == 0 {
            return Err(io::Error::other(Enough));
        }
        let count = (bytes.len() as u64).min(self.left);
        self.left -= count;
        Ok(count as usize)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The remembered password against the smallest encrypted member, up to `sample`
/// bytes; `None` when it opens the archive or the archive is left to the action.
pub(super) fn verify(path: &Path, file: File, sample: u64) -> Option<ArchivePasswordFailure> {
    let secret = archive_password::for_archive(path);
    let refusal = |error: CommandError| ArchivePasswordFailure::from_message(&error.to_string());
    let archive = match parse(BufReader::new(file), secret.as_ref(), ArchiveLimits::LISTING) {
        Ok(archive) => archive,
        Err(error) => return refusal(error),
    };
    let chosen = archive
        .members()
        .enumerate()
        .filter(|(_, member)| member.meta.is_encrypted && !member.meta.is_directory && !member.meta.is_redirection)
        .min_by_key(|(_, member)| member.meta.unpacked_size)?
        .0;
    if secret.is_none() {
        return Some(ArchivePasswordFailure::Required);
    }
    let solid = archive.is_solid();
    let mut index = 0;
    let result = archive.extract_with_control(options(secret.as_ref(), ArchiveLimits::LISTING), |member| {
        let current = index;
        index += 1;
        Ok(if current == chosen {
            rars::ExtractionDecision::Extract(Box::new(Sample { left: sample }))
        } else if current > chosen {
            rars::ExtractionDecision::Stop
        } else if solid && !member.meta.is_directory {
            rars::ExtractionDecision::Extract(Box::new(io::sink()))
        } else {
            rars::ExtractionDecision::Skip
        })
    });
    match result {
        Ok(_) => None,
        Err(error) if error.to_string().contains("Sample complete") => None,
        Err(error) => refusal(failure(error, true)),
    }
}

/// Streams a member's bytes to the parent as `Chunk` frames.
struct Chunks<T> {
    connection: Connection<T>,
    id: u64,
    offset: u64,
    written: Arc<std::sync::atomic::AtomicU64>,
}

impl<T: Exchange> Write for Chunks<T> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len().min(MAX_BLOCK);
        if count == 0 {
            return Ok(0);
        }
        super::archive_io_codec::notify_frame(
            &self.connection,
            Frame::data(Message::Chunk { id: self.id, offset: self.offset }, bytes[..count].to_vec()),
        )?;
        self.offset = self.offset.checked_add(count as u64).ok_or_else(invalid)?;
        self.written.store(self.offset, std::sync::atomic::Ordering::Relaxed);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn entry(id: u64, name: String, meta: &rars::ArchiveMemberMeta) -> io::Result<Entry> {
    let directory = meta.is_directory;
    let entry = Entry {
        id,
        name,
        directory,
        size: if directory { 0 } else { meta.unpacked_size },
        modified: meta.modification_time().map(Timestamp::from_system).transpose()?,
        created: None,
        accessed: None,
        mode: if directory { 0o755 } else { 0o644 },
    };
    entry.validate()?;
    Ok(entry)
}

/// What the broker reads: the first volume's size, the later volumes' ones, and whether
/// the manifest alone is asked, to list a set.
pub(super) struct Sources<'a> {
    pub first: u64,
    pub later: &'a [u64],
    pub manifest_only: bool,
}

/// The broker's decode: the whole manifest, then the selected member or all of them.
pub(super) fn decode<T: Exchange + Send + 'static>(
    connection: &Connection<T>,
    sources: &Sources<'_>,
    requested: Option<&Path>,
    secret: Option<&ArchiveSecret>,
    budget: &ArchiveBudget,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<(), CommandError> {
    let limits = budget.limits();
    let set = !sources.later.is_empty();
    let mut archives = Vec::new();
    for (id, size) in std::iter::once(sources.first).chain(sources.later.iter().copied()).enumerate() {
        budget.checkpoint(checkpoint)?;
        let source = WindowReader::new(Reader::new(connection.clone(), id as u64, size), MAX_BLOCK);
        archives.push(parse(source, secret, limits)?);
    }
    // Continuations fold into the member they continue.
    let members = if set {
        rars::volume_members(&archives).map_err(|error| failure(error, secret.is_some()))?
    } else {
        archives[0].members().collect()
    };
    let mut entries = Vec::new();
    let mut selected = None;
    for (index, member) in members.iter().enumerate() {
        budget.checkpoint(checkpoint)?;
        let name = member_name(&member.meta, set)?;
        let chosen = requested.is_none_or(|requested| relative_archive_path(&name).is_ok_and(|path| path == requested));
        let meta = entry(index as u64, name, &member.meta)?;
        budget.entry(&meta.name, if chosen && !sources.manifest_only { meta.size } else { 0 })?;
        if requested.is_some() && chosen {
            if selected.is_some() || meta.directory {
                return Err(invalid().into());
            }
            selected = Some(index);
        }
        entries.push(meta.clone());
        super::archive_io_codec::notify_frame(connection, Frame::new(Message::Entry { entry: meta }))?;
    }
    budget.check_entry_count(entries.len())?;
    if requested.is_some() && selected.is_none() {
        return Err(invalid().into());
    }
    super::archive_io_codec::notify_frame(connection, Frame::new(Message::ManifestEnd))?;
    if sources.manifest_only {
        return Ok(());
    }
    let solid = archives[0].is_solid();
    // The member being written; its `End` follows once `rars` has verified it.
    let open: Arc<Mutex<Option<usize>>> = Arc::default();
    let written = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let finish = |index: usize| -> io::Result<()> {
        let entry = entries.get(index).ok_or_else(invalid)?;
        if !entry.directory && written.swap(0, std::sync::atomic::Ordering::Relaxed) != entry.size {
            return Err(invalid());
        }
        super::archive_io_codec::notify_frame(connection, Frame::new(Message::End { id: index as u64 }))
    };
    /// What happens to the member at an index, in archive order.
    enum Step {
        Stop,
        Discard,
        Skip,
        Write(Box<dyn Write>),
    }
    let mut current = 0usize;
    let mut decide = |directory: bool| -> io::Result<Step> {
        let index = current;
        current += 1;
        if let Some(previous) = open.lock().map_err(|_| invalid())?.take() {
            finish(previous)?;
        }
        budget.checkpoint(checkpoint)?;
        if let Some(chosen) = selected {
            if index > chosen {
                return Ok(Step::Stop);
            }
            if index != chosen {
                // A solid archive, or a set, decodes what precedes the selection.
                return Ok(if (solid || set) && !directory { Step::Discard } else { Step::Skip });
            }
        }
        let id = index as u64;
        super::archive_io_codec::notify_frame(connection, Frame::new(Message::Begin { id }))?;
        if directory {
            finish(index)?;
            return Ok(Step::Skip);
        }
        *open.lock().map_err(|_| invalid())? = Some(index);
        Ok(Step::Write(Box::new(Chunks { connection: connection.clone(), id, offset: 0, written: written.clone() })))
    };
    let mut failure_in_callback: Option<io::Error> = None;
    let mut stopped = false;
    let mut wrap = |error: io::Error| {
        let wrapped = rars::Error::from(io::Error::new(error.kind(), error.to_string()));
        failure_in_callback = Some(error);
        wrapped
    };
    let result = if set {
        rars::extract_volumes_to_with_options(&archives, options(secret, limits), |meta| match decide(meta.is_directory) {
            Ok(Step::Write(writer)) => Ok(writer),
            Ok(Step::Discard | Step::Skip) => Ok(Box::new(io::sink())),
            // The set has no stop: an error ends the traversal, then counts as done.
            Ok(Step::Stop) => {
                stopped = true;
                Err(rars::Error::from(io::Error::other("Selection complete")))
            }
            Err(error) => Err(wrap(error)),
        })
    } else {
        archives[0].extract_with_control(options(secret, limits), |member| match decide(member.meta.is_directory) {
            Ok(Step::Write(writer)) => Ok(rars::ExtractionDecision::Extract(writer)),
            Ok(Step::Discard) => Ok(rars::ExtractionDecision::Extract(Box::new(io::sink()))),
            Ok(Step::Skip) => Ok(rars::ExtractionDecision::Skip),
            Ok(Step::Stop) => Ok(rars::ExtractionDecision::Stop),
            Err(error) => Err(wrap(error)),
        })
        .map(drop)
    };
    if let Some(error) = failure_in_callback {
        return Err(error.into());
    }
    if !stopped {
        result.map_err(|error| failure(error, secret.is_some()))?;
    }
    if let Some(last) = open.lock().map_err(|_| invalid())?.take() {
        finish(last)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "archive_rar/tests/rar_tests.rs"]
mod tests;
