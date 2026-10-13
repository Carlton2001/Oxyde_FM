//! Independent bounded TAR record verification; no tar codec is called here.
//! Covers plain TAR payloads/names/types/mode/mtime, not compressed containers.
use super::{
    archive_io_protocol::{Entry, MAX_NAME},
    archive_limits::{ArchiveBudget, ArchiveLimits},
    archive_security::relative_archive_path,
};
use std::{
    collections::HashSet,
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::PathBuf,
};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Record {
    pub name: PathBuf,
    pub directory: bool,
    pub size: u64,
    pub mode: u32,
    pub modified: Option<i64>,
    pub hash: Option<blake3::Hash>,
}
pub(crate) fn failure() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, FidelityFailure)
}
#[derive(Debug)]
struct FidelityFailure;
impl std::fmt::Display for FidelityFailure {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output.write_str("Archive fidelity verification failed")
    }
}
impl std::error::Error for FidelityFailure {}
#[cfg(not(test))]
pub(crate) fn source_error(error: io::Error) -> io::Error {
    if error
        .get_ref()
        .is_some_and(|cause| cause.is::<FidelityFailure>())
    {
        io::Error::other(crate::models::error::ArchiveProcessingFailure)
    } else {
        error
    }
}
fn number(bytes: &[u8]) -> io::Result<u64> {
    if bytes[0] & 0x80 != 0 {
        if bytes[0] != 0x80 {
            return Err(failure());
        }
        return bytes[1..].iter().try_fold(0u64, |value, byte| {
            value
                .checked_mul(256)
                .and_then(|n| n.checked_add(u64::from(*byte)))
                .ok_or_else(failure)
        });
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| failure())?
        .trim_matches([' ', '\0']);
    if text.is_empty() {
        return Ok(0);
    }
    if !text.bytes().all(|byte| matches!(byte, b'0'..=b'7')) {
        return Err(failure());
    }
    u64::from_str_radix(text, 8).map_err(|_| failure())
}
fn text(bytes: &[u8]) -> io::Result<String> {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    std::str::from_utf8(&bytes[..end])
        .map(str::to_owned)
        .map_err(|_| failure())
}
fn pax(bytes: &[u8]) -> io::Result<(Option<String>, Option<u64>)> {
    let mut path = None;
    let mut size = None;
    let mut offset = 0;
    while offset < bytes.len() {
        let rest = &bytes[offset..];
        let space = rest
            .iter()
            .position(|byte| *byte == b' ')
            .ok_or_else(failure)?;
        let length = std::str::from_utf8(&rest[..space])
            .map_err(|_| failure())?
            .parse::<usize>()
            .map_err(|_| failure())?;
        if length <= space + 2 || length > rest.len() || rest[length - 1] != b'\n' {
            return Err(failure());
        }
        let field = std::str::from_utf8(&rest[space + 1..length - 1]).map_err(|_| failure())?;
        let (key, value) = field.split_once('=').ok_or_else(failure)?;
        match key {
            "path" if path.is_none() => {
                if value.len() > MAX_NAME {
                    return Err(failure());
                }
                path = Some(value.to_owned());
            }
            "size" if size.is_none() => size = Some(value.parse().map_err(|_| failure())?),
            "path" | "size" => return Err(failure()),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Unsupported TAR extension for independent fidelity verification",
                ))
            }
        }
        offset += length;
    }
    Ok((path, size))
}

pub(crate) fn scan(
    file: &mut File,
    limits: ArchiveLimits,
    selected: Option<&str>,
    contents: bool,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> io::Result<Vec<Record>> {
    scan_impl(file, limits, selected, contents, false, checkpoint)
}
pub(crate) fn scan_canonical(
    file: &mut File,
    limits: ArchiveLimits,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> io::Result<Vec<Record>> {
    scan_impl(file, limits, None, true, true, checkpoint)
}
fn scan_impl(
    file: &mut File,
    limits: ArchiveLimits,
    selected: Option<&str>,
    contents: bool,
    canonical: bool,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> io::Result<Vec<Record>> {
    let budget = ArchiveBudget::new(limits);
    let selected = selected.map(relative_archive_path).transpose()?;
    let length = file.metadata()?.len();
    let mut offset = 0u64;
    let mut records = Vec::new();
    let mut names = HashSet::new();
    let mut name_bytes = 0usize;
    let mut long_name = None;
    let mut extended = None;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        budget.checkpoint(checkpoint)?;
        if length.saturating_sub(offset) < 512 {
            return Err(failure());
        }
        file.seek(SeekFrom::Start(offset))?;
        let mut header = [0; 512];
        file.read_exact(&mut header)?;
        offset += 512;
        if header == [0; 512] {
            if long_name.is_some() || extended.is_some() || length.saturating_sub(offset) < 512 {
                return Err(failure());
            }
            file.read_exact(&mut header)?;
            offset += 512;
            if header != [0; 512] {
                return Err(failure());
            }
            while offset < length {
                budget.checkpoint(checkpoint)?;
                let count = (length - offset).min(buffer.len() as u64) as usize;
                file.read_exact(&mut buffer[..count])?;
                if buffer[..count].iter().any(|byte| *byte != 0) {
                    return Err(failure());
                }
                offset += count as u64;
            }
            if selected.is_some()
                && !records.iter().any(|record: &Record| {
                    !record.directory && selected.as_ref() == Some(&record.name)
                })
            {
                return Err(failure());
            }
            return Ok(records);
        }
        let checksum: u64 = header
            .iter()
            .enumerate()
            .map(|(index, byte)| {
                if (148..156).contains(&index) {
                    32
                } else {
                    u64::from(*byte)
                }
            })
            .sum();
        if number(&header[148..156])? != checksum {
            return Err(failure());
        }
        if canonical
            && (number(&header[108..116])? != 0
                || number(&header[116..124])? != 0
                || header[157..257].iter().any(|byte| *byte != 0)
                || header[265..329].iter().any(|byte| *byte != 0))
        {
            return Err(failure());
        }
        let mut size = number(&header[124..136])?;
        let kind = header[156];
        if matches!(kind, b'L' | b'x') {
            if canonical && kind == b'x' {
                return Err(failure());
            }
            if size > (MAX_NAME * 2 + 1024) as u64 || long_name.is_some() || extended.is_some() {
                return Err(failure());
            }
            let end = offset
                .checked_add(size)
                .and_then(|n| n.checked_add((512 - size % 512) % 512))
                .filter(|end| *end <= length)
                .ok_or_else(failure)?;
            let mut bytes = vec![0; size as usize];
            file.read_exact(&mut bytes)?;
            if kind == b'L' {
                long_name = Some(text(&bytes)?);
            } else {
                extended = Some(pax(&bytes)?);
            }
            if canonical {
                let padding = (end - offset - size) as usize;
                file.read_exact(&mut buffer[..padding])?;
                if buffer[..padding].iter().any(|byte| *byte != 0) {
                    return Err(failure());
                }
            }
            offset = end;
            continue;
        }
        if !matches!(kind, 0 | b'0' | b'5') {
            return Err(failure());
        }
        let directory = kind == b'5';
        let mut name = long_name.take().unwrap_or(text(&header[..100])?);
        if &header[257..263] == b"ustar\0" && &header[263..265] == b"00" {
            let prefix = text(&header[345..500])?;
            if !prefix.is_empty() {
                name = format!("{prefix}/{name}");
            }
        }
        if let Some((path, override_size)) = extended.take() {
            if let Some(path) = path {
                name = path;
            }
            if let Some(value) = override_size {
                size = value;
            }
        }
        let name = relative_archive_path(&name)?;
        name_bytes = name_bytes
            .checked_add(name.to_string_lossy().len())
            .filter(|bytes| *bytes <= super::archive_io_state::MAX_MANIFEST_NAMES)
            .ok_or_else(failure)?;
        if (!directory && name.as_os_str().is_empty())
            || name.to_string_lossy().len() > MAX_NAME
            || !names.insert(name.to_string_lossy().to_lowercase())
            || (directory && size != 0)
        {
            return Err(failure());
        }
        let chosen = selected.as_ref().is_none_or(|selected| *selected == name);
        budget.entry(&name.to_string_lossy(), if chosen { size } else { 0 })?;
        let end = offset
            .checked_add(size)
            .and_then(|n| n.checked_add((512 - size % 512) % 512))
            .filter(|end| *end <= length)
            .ok_or_else(failure)?;
        let mut hash = blake3::Hasher::new();
        if contents && chosen && !directory {
            let mut remaining = size;
            while remaining != 0 {
                budget.checkpoint(checkpoint)?;
                let count = remaining.min(buffer.len() as u64) as usize;
                file.read_exact(&mut buffer[..count])?;
                hash.update(&buffer[..count]);
                remaining -= count as u64;
            }
        }
        records.push(Record {
            name,
            directory,
            size,
            mode: (number(&header[100..108])? & 0o7777) as u32,
            modified: number(&header[136..148])
                .ok()
                .and_then(|value| value.try_into().ok()),
            hash: (contents && chosen).then(|| hash.finalize()),
        });
        if canonical {
            let padding = (end - offset - size) as usize;
            file.read_exact(&mut buffer[..padding])?;
            if buffer[..padding].iter().any(|byte| *byte != 0) {
                return Err(failure());
            }
        }
        offset = end;
    }
}

pub(crate) struct Verifier {
    records: Vec<Record>,
    seen: usize,
    delivered: HashSet<u64>,
    active: Option<(u64, blake3::Hasher)>,
}
impl Verifier {
    pub fn new(records: Vec<Record>) -> Self {
        Self {
            records,
            seen: 0,
            delivered: HashSet::new(),
            active: None,
        }
    }
    pub fn entry(&mut self, entry: &Entry) -> io::Result<()> {
        let expected = self.records.get(self.seen).ok_or_else(failure)?;
        if entry.id != self.seen as u64
            || relative_archive_path(&entry.name)? != expected.name
            || entry.directory != expected.directory
            || entry.size != expected.size
            || entry.mode != expected.mode
            || entry.modified.map(|time| (time.seconds, time.nanos))
                != expected.modified.map(|time| (time, 0))
            || entry.created.is_some()
            || entry.accessed.is_some()
        {
            return Err(failure());
        }
        self.seen += 1;
        Ok(())
    }
    pub fn begin(&mut self, id: u64) -> io::Result<()> {
        let record = self.records.get(id as usize).ok_or_else(failure)?;
        if self.active.is_some() || !self.delivered.insert(id) || record.hash.is_none() {
            return Err(failure());
        }
        self.active = Some((id, blake3::Hasher::new()));
        Ok(())
    }
    pub fn chunk(&mut self, id: u64, bytes: &[u8]) -> io::Result<()> {
        let (active, hash) = self.active.as_mut().ok_or_else(failure)?;
        if *active != id {
            return Err(failure());
        }
        hash.update(bytes);
        Ok(())
    }
    pub fn end(&mut self, id: u64) -> io::Result<()> {
        let (active, hash) = self.active.take().ok_or_else(failure)?;
        let record = self.records.get(id as usize).ok_or_else(failure)?;
        if active != id
            || record
                .hash
                .is_some_and(|expected| hash.finalize() != expected)
        {
            return Err(failure());
        }
        Ok(())
    }
    pub fn finish(&self) -> io::Result<()> {
        if self.active.is_some()
            || self.seen != self.records.len()
            || self.records.iter().enumerate().any(|(index, record)| {
                (record.hash.is_some()) && !self.delivered.contains(&(index as u64))
            })
        {
            return Err(failure());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tar_fidelity/tests/verification.rs"]
mod tests;
