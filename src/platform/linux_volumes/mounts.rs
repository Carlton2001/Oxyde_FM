//! Mount points from the kernel table, so a drop can tell when `rename` crosses a mount.

/// Every mount point of this process's namespace; empty when the table is unreadable.
pub(super) fn mount_points() -> Vec<String> {
    std::fs::read_to_string("/proc/self/mountinfo")
        .map(|table| parse(&table))
        .unwrap_or_default()
}

fn parse(table: &str) -> Vec<String> {
    let mut points: Vec<String> = table
        .lines()
        .filter_map(|line| line.split(' ').nth(4).map(unescape))
        .filter(|point| point.starts_with('/'))
        .collect();
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
