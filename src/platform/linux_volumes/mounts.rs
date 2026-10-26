//! The kernel mount table: mount points, so a drop can tell when `rename` crosses a
//! mount, and entries for volumes UDisks2 does not manage.

pub(super) struct MountEntry {
    pub point: String,
    pub fs_type: String,
    pub source: String,
}

/// Every mount of this process's namespace, in kernel order; empty when unreadable.
pub(super) fn table() -> Vec<MountEntry> {
    std::fs::read_to_string("/proc/self/mountinfo")
        .map(|table| entries(&table))
        .unwrap_or_default()
}

pub(super) fn mount_points() -> Vec<String> {
    points(&table())
}

/// `id parent dev root point options [optional…] - fs_type source super_options`
fn entries(table: &str) -> Vec<MountEntry> {
    table
        .lines()
        .filter_map(|line| {
            let (mount, filesystem) = line.split_once(" - ")?;
            let mut filesystem = filesystem.split(' ');
            Some(MountEntry {
                point: unescape(mount.split(' ').nth(4)?),
                fs_type: unescape(filesystem.next()?),
                source: unescape(filesystem.next()?),
            })
        })
        .filter(|entry| entry.point.starts_with('/'))
        .collect()
}

fn points(entries: &[MountEntry]) -> Vec<String> {
    let mut points: Vec<String> = entries.iter().map(|entry| entry.point.clone()).collect();
    points.sort();
    points.dedup();
    points
}

/// The kernel writes space, tab, newline and backslash as three octal digits.
fn unescape(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let octal = bytes.get(index + 1..index + 4).and_then(|digits| {
            std::str::from_utf8(digits).ok().and_then(|digits| u8::from_str_radix(digits, 8).ok())
        });
        match (bytes[index], octal) {
            (b'\\', Some(byte)) => {
                out.push(byte);
                index += 4;
            }
            (byte, _) => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
#[path = "tests/mounts.rs"]
mod tests;
