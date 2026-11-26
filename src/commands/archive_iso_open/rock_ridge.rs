//! Rock Ridge (RRIP over SUSP), which Linux tools write for long, case-sensitive names.
//! As the kernel does when it mounts an image, its names come before Joliet's.
use super::{both_u32, invalid, record, Extent, Image, Record, SECTOR};
use std::io::{self, Read, Seek};

/// Continuation areas followed for one record, against a loop of CE entries.
const MAX_CONTINUATIONS: usize = 16;

/// What a record's system use area says.
#[derive(Default)]
pub(super) struct Fields {
    /// `None` keeps the ISO 9660 identifier: no NM entry, or a name that is not UTF-8.
    pub(super) name: Option<String>,
    /// A directory moved under `rr_moved` for depth, listed where its CL entry points.
    pub(super) relocated: bool,
    /// The block of the relocated directory this placeholder stands for.
    pub(super) child: Option<u64>,
}

/// The bytes skipped before each system use area, when the root's `.` record opens
/// with the SP entry that announces SUSP.
pub(super) fn detect<R: Read + Seek>(image: &mut Image<'_, R>, root: &Record, block_size: u64) -> Option<usize> {
    let dot = first_record(image, root.extent.offset, block_size).ok()?;
    let sp = dot.system_use.get(..7)?;
    (sp[..4] == *b"SP\x07\x01" && sp[4..6] == [0xBE, 0xEF]).then_some(sp[6] as usize)
}

pub(super) fn fields<R: Read + Seek>(
    image: &mut Image<'_, R>,
    entry: &Record,
    skip: usize,
    block_size: u64,
) -> io::Result<Fields> {
    let mut fields = Fields::default();
    let mut name = Vec::new();
    let mut area = entry.system_use.get(skip..).unwrap_or_default().to_vec();
    for _ in 0..=MAX_CONTINUATIONS {
        let mut next = None;
        let mut rest = area.as_slice();
        while rest.len() >= 4 {
            let length = rest[2] as usize;
            if length < 4 || length > rest.len() {
                break;
            }
            let data = &rest[4..length];
            match &rest[..2] {
                // Flags 2 and 4 name the directory itself or its parent.
                b"NM" if data.first().is_some_and(|flags| flags & 0b110 == 0) => name.extend_from_slice(&data[1..]),
                b"CE" if data.len() >= 24 => {
                    next = Some((both_u32(&data[0..8])?, both_u32(&data[8..16])?, both_u32(&data[16..24])?));
                }
                b"RE" => fields.relocated = true,
                b"CL" if data.len() >= 8 => fields.child = Some(both_u32(&data[0..8])? as u64),
                b"ST" => break,
                _ => {}
            }
            rest = &rest[length..];
        }
        let Some((block, offset, length)) = next else {
            fields.name = (!name.is_empty()).then(|| String::from_utf8(name).ok()).flatten();
            return Ok(fields);
        };
        if u64::from(offset) + u64::from(length) > SECTOR {
            return Err(invalid("Invalid Rock Ridge continuation area"));
        }
        let start = u64::from(block)
            .checked_mul(block_size)
            .and_then(|start| start.checked_add(offset.into()))
            .ok_or_else(|| invalid("Invalid Rock Ridge continuation area"))?;
        area = vec![0; length as usize];
        image.read_at(start, &mut area)?;
    }
    Err(invalid("Too many Rock Ridge continuation areas"))
}

/// The directory a CL entry points to, from its own `.` record.
pub(super) fn child_extent<R: Read + Seek>(image: &mut Image<'_, R>, block: u64, block_size: u64) -> io::Result<Extent> {
    let offset = block.checked_mul(block_size).ok_or_else(|| invalid("Invalid Rock Ridge child link"))?;
    let dot = first_record(image, offset, block_size)?;
    if !dot.directory || !dot.final_extent || dot.raw_name != [0] {
        return Err(invalid("Invalid Rock Ridge child link"));
    }
    Ok(dot.extent)
}

fn first_record<R: Read + Seek>(image: &mut Image<'_, R>, offset: u64, block_size: u64) -> io::Result<Record> {
    let mut bytes = [0u8; 255];
    image.read_at(offset, &mut bytes[..1])?;
    let length = bytes[0] as usize;
    if length < 34 {
        return Err(invalid("Invalid ISO directory record length"));
    }
    image.read_at(offset + 1, &mut bytes[1..length])?;
    record(&bytes[..length], block_size)
}
