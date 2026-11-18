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
        let field = &rest[space + 1..length - 1];
        let equal = field.iter().position(|byte| *byte == b'=').ok_or_else(failure)?;
        let key = std::str::from_utf8(&field[..equal]).map_err(|_| failure())?;
        // Values may be binary (extended attributes): only decode the used ones.
        let value = || std::str::from_utf8(&field[equal + 1..]).map_err(|_| failure());
        match key {
            "path" if path.is_none() => {
                let value = value()?;
                if value.len() > MAX_NAME {
                    return Err(failure());
                }
                path = Some(value.to_owned());
            }
            "size" if size.is_none() => size = Some(value()?.parse().map_err(|_| failure())?),
            "path" | "size" => return Err(failure()),
            key if key.starts_with("GNU.sparse.") => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "PAX sparse TAR members are not supported",
                ))
            }
            // Owners, times, comments and extended attributes change neither
            // the member's name, type, size nor content; the codecs ignore
            // them too (the date comes from the header).
            _ => {}
        }
        offset += length;
    }
    Ok((path, size))
}

/// GNU sparse map (`S`): four blocks in the header, then 21 per extension
/// header while the extension flag is set. Calls `visit` with each data
/// block's offset in the file and its length, after the same checks as the
/// tar crate. Returns the extension header count and the real size.
fn sparse_map(
    file: &mut File,
    header: &[u8; 512],
    start: u64,
    length: u64,
    stored: u64,
    visit: &mut dyn FnMut(&mut File, u64, u64) -> io::Result<()>,
) -> io::Result<(u64, u64)> {
    if &header[257..265] != b"ustar  \0" {
        return Err(failure());
    }
    let real = number(&header[483..495])?;
    let (mut current, mut remaining) = (0u64, stored);
    let mut block = |file: &mut File, entry: &[u8]| -> io::Result<()> {
        // An empty slot has a NUL first byte in either field.
        if entry[0] == 0 || entry[12] == 0 {
            return Ok(());
        }
        let (start, size) = (number(&entry[..12])?, number(&entry[12..24])?);
        if (size != 0 && (stored - remaining) % 512 != 0) || start < current {
            return Err(failure());
        }
        current = start.checked_add(size).ok_or_else(failure)?;
        remaining = remaining.checked_sub(size).ok_or_else(failure)?;
        visit(file, start, size)
    };
    for entry in header[386..482].chunks(24) {
        block(file, entry)?;
    }
    let mut extended = header[482] != 0;
    let mut extensions = 0u64;
    let mut buffer = [0u8; 512];
    while extended {
        let position = start.checked_add(extensions * 512).ok_or_else(failure)?;
        if length.saturating_sub(position) < 512 {
            return Err(failure());
        }
        file.seek(SeekFrom::Start(position))?;
        file.read_exact(&mut buffer)?;
        extensions += 1;
        for entry in buffer[..504].chunks(24) {
            block(file, entry)?;
        }
        extended = buffer[504] != 0;
    }
    if current != real || remaining != 0 {
        return Err(failure());
    }
    Ok((extensions, real))
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
    let mut long_link = None;
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
            if long_name.is_some()
                || long_link.is_some()
                || extended.is_some()
                || length.saturating_sub(offset) < 512
            {
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
        if kind == b'g' {
            // A PAX global header describes the archive, not a member.
            if canonical || long_name.is_some() || long_link.is_some() || extended.is_some() {
                return Err(failure());
            }
            offset = offset
                .checked_add(size)
                .and_then(|n| n.checked_add((512 - size % 512) % 512))
                .filter(|end| *end <= length)
                .ok_or_else(failure)?;
            continue;
        }
        if matches!(kind, b'L' | b'K' | b'x') {
            if canonical && kind != b'L' {
                return Err(failure());
            }
            // Extended attributes can make PAX records longer than a name.
            let limit = if kind == b'x' { 64 * 1024 } else { MAX_NAME * 2 + 1024 };
            let pending = match kind {
                b'L' => long_name.is_some() || extended.is_some(),
                b'K' => long_link.is_some(),
                _ => long_name.is_some() || extended.is_some(),
            };
            if size > limit as u64 || pending {
                return Err(failure());
            }
            let end = offset
                .checked_add(size)
                .and_then(|n| n.checked_add((512 - size % 512) % 512))
                .filter(|end| *end <= length)
                .ok_or_else(failure)?;
            let mut bytes = vec![0; size as usize];
            file.read_exact(&mut bytes)?;
            match kind {
                b'L' => long_name = Some(text(&bytes)?),
                // A long link target only matters to links, which are refused.
                b'K' => long_link = Some(()),
                _ => extended = Some(pax(&bytes)?),
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
        // Contiguous (`7`) and GNU sparse (`S`) members are files.
        if !matches!(kind, 0 | b'0' | b'5' | b'7' | b'S') || (kind == b'S' && canonical) {
            return Err(failure());
        }
        let directory = kind == b'5';
        let sparse = kind == b'S';
        long_link = None;
        let extended = extended.take();
        if sparse && extended.is_some() {
            return Err(failure());
        }
        let (path, override_size) = extended.unwrap_or((None, None));
        // The header name only counts without a long one, as in the tar crate:
        // GNU tar cuts it at 100 bytes, possibly inside a character.
        let name = match (long_name.take(), path) {
            (Some(name), _) | (None, Some(name)) => name,
            (None, None) => {
                let name = text(&header[..100])?;
                let prefix = if &header[257..263] == b"ustar\0" && &header[263..265] == b"00" {
                    text(&header[345..500])?
                } else {
                    String::new()
                };
                if prefix.is_empty() { name } else { format!("{prefix}/{name}") }
            }
        };
        if let Some(value) = override_size {
            size = value;
        }
        // Stored bytes in the archive; `size` becomes the member's real size.
        let mut stored = size;
        let headers = offset;
        if sparse {
            let (extensions, real) =
                sparse_map(file, &header, offset, length, size, &mut |_, _, _| Ok(()))?;
            offset = offset.checked_add(extensions * 512).ok_or_else(failure)?;
            (stored, size) = (size, real);
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
            .checked_add(stored)
            .and_then(|n| n.checked_add((512 - stored % 512) % 512))
            .filter(|end| *end <= length)
            .ok_or_else(failure)?;
        let mut hash = blake3::Hasher::new();
        if contents && chosen && sparse {
            let data = offset;
            let (mut expanded, mut read) = (0u64, 0u64);
            let fill = |hash: &mut blake3::Hasher, mut count: u64| -> io::Result<()> {
                let zeros = [0u8; 64 * 1024];
                while count != 0 {
                    budget.checkpoint(checkpoint)?;
                    let step = count.min(zeros.len() as u64) as usize;
                    hash.update(&zeros[..step]);
                    count -= step as u64;
                }
                Ok(())
            };
            sparse_map(file, &header, headers, length, stored, &mut |file, start, count| {
                fill(&mut hash, start - expanded)?;
                file.seek(SeekFrom::Start(data + read))?;
                let mut remaining = count;
                while remaining != 0 {
                    budget.checkpoint(checkpoint)?;
                    let step = remaining.min(buffer.len() as u64) as usize;
                    file.read_exact(&mut buffer[..step])?;
                    hash.update(&buffer[..step]);
                    remaining -= step as u64;
                }
                read += count;
                expanded = start + count;
                Ok(())
            })?;
            fill(&mut hash, size - expanded)?;
        } else if contents && chosen && !directory {
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
            let padding = (end - offset - stored) as usize;
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
