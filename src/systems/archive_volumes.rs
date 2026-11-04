//! The volumes of a split RAR, served to the confined broker as sources 0, 1, 2…, and the
//! set's listing, which the broker sends as a manifest: no worker sees more than its reads.
use super::path_parents::HeldParents;
use crate::models::{CommandError, FileEntry};
use crate::platform::platform;
use crate::utils::archive_io_protocol::{invalid, Control, Format, Frame, Message, Operation, MAX_BLOCK};
use crate::utils::archive_limits::ArchiveLimits;
use crate::utils::archive_security::relative_archive_path;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;

pub(crate) struct VolumeFiles {
    files: Vec<(File, u64)>,
}

impl VolumeFiles {
    /// `source` opened at `path`; a volume of a set brings every volume, opened under
    /// their held folder, the first one first.
    pub(crate) fn open(path: &Path, source: File) -> io::Result<Self> {
        let volumes = crate::utils::archive_volumes::rar_volumes(path);
        if volumes.len() < 2 {
            let size = source.metadata()?.len();
            return Ok(Self { files: vec![(source, size)] });
        }
        let parents = HeldParents::new(path)?;
        let files = volumes
            .iter()
            .map(|volume| {
                let file = platform().open_archive_source(&parents.resolved_path(volume)?, false)?;
                let size = file.metadata()?.len();
                if size == 0 {
                    return Err(invalid());
                }
                Ok((file, size))
            })
            .collect::<io::Result<Vec<_>>>()?;
        Ok(Self { files })
    }

    pub(crate) fn first(&self) -> &File {
        &self.files[0].0
    }

    pub(crate) fn first_size(&self) -> u64 {
        self.files[0].1
    }

    /// Sizes of the volumes after the first, as `Operation::Decode::volumes` carries them.
    pub(crate) fn later_sizes(&self) -> Vec<u64> {
        self.files[1..].iter().map(|(_, size)| *size).collect()
    }

    /// The bytes a `Read` frame asks for, inside the volume it names.
    pub(crate) fn serve(&mut self, source: u64, offset: u64, length: u32) -> io::Result<Frame> {
        let (file, size) = self.files.get_mut(usize::try_from(source).map_err(|_| invalid())?).ok_or_else(invalid)?;
        if length == 0 || length as usize > MAX_BLOCK || offset.checked_add(length as u64).is_none_or(|end| end > *size) {
            return Err(invalid());
        }
        file.seek(SeekFrom::Start(offset))?;
        let mut data = vec![0; length as usize];
        file.read_exact(&mut data)?;
        Ok(Frame::data(Message::Bytes { source, offset }, data))
    }
}

fn ack() -> Frame {
    Frame::new(Message::Ack { pause: false, turbo: false })
}

/// Every member of a RAR set, for navigation and search, decoded by no one: the broker
/// parses each volume's headers and sends the merged manifest.
pub(crate) fn index_rar_set(path: &Path, checkpoint: &dyn Fn() -> io::Result<()>) -> Result<Vec<FileEntry>, CommandError> {
    let parents = HeldParents::new(path)?;
    let source = platform().open_archive_source(&parents.resolved_path(path)?, false)?;
    let mut files = VolumeFiles::open(path, source)?;
    let limits = super::archive_io::limits(ArchiveLimits::LISTING);
    let operation = Operation::Decode {
        format: Format::Rar,
        source_size: files.first_size(),
        selected: None,
        stream_name: None,
        password: crate::utils::archive_password::for_archive(path),
        volumes: files.later_sizes(),
        manifest_only: true,
        limits,
    };
    let origin = Arc::new(path.to_path_buf());
    let mut entries = Vec::new();
    let controls = || Ok(Control { cancelled: checkpoint().is_err(), ..Control::default() });
    platform()
        .run_archive_io_worker(operation, &controls, &mut |frame| {
            frame.validate()?;
            match frame.message {
                Message::Read { source, offset, length } => files.serve(source, offset, length),
                Message::Entry { entry } => {
                    if entries.len() as u64 >= limits.entries {
                        return Err(invalid());
                    }
                    let relative = relative_archive_path(&entry.name).map_err(io::Error::other)?;
                    let name = relative.file_name().ok_or_else(invalid)?.to_string_lossy().into_owned();
                    let modified = match entry.modified {
                        Some(time) => time
                            .system()?
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_or(0, |time| time.as_millis() as u64),
                        None => 0,
                    };
                    entries.push(FileEntry {
                        archive_path: Some(origin.clone()),
                        name,
                        path: origin.join(&relative).to_string_lossy().into_owned(),
                        is_dir: entry.directory,
                        size: if entry.directory { 0 } else { entry.size },
                        modified,
                        ..FileEntry::default()
                    });
                    Ok(ack())
                }
                Message::ManifestEnd | Message::Finished => Ok(ack()),
                _ => Err(invalid()),
            }
        })
        .map_err(CommandError::from)?;
    Ok(entries)
}
