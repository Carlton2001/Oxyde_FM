use std::fs::Metadata;

pub mod archive;
pub(crate) mod archive_limits;
pub mod archive_security;
pub mod disk_image;
pub mod path_security;
pub mod preview;

pub mod hardware {
    pub fn is_network_path(path: &std::path::Path) -> bool {
        crate::platform::platform().is_network_path(path)
    }

    pub fn get_physical_disk_id(path: &std::path::Path) -> u64 {
        crate::platform::platform().physical_disk_id(path)
    }

    pub fn is_ssd(path: &std::path::Path) -> bool {
        crate::platform::platform().is_ssd(path)
    }
}

use unicode_normalization::UnicodeNormalization;

/// Removes accents from a string
pub fn remove_accents(s: &str) -> String {
    s.nfd().filter(|c| !char_is_mark(*c)).collect()
}

fn char_is_mark(c: char) -> bool {
    // Unicode mark category starts from '\u{0300}' to '\u{036F}' for basic accents
    // We check if the character is in the combining diacritical marks block
    ('\u{0300}'..='\u{036F}').contains(&c)
        || ('\u{1AB0}'..='\u{1AFF}').contains(&c)
        || ('\u{1DC0}'..='\u{1DFF}').contains(&c)
        || ('\u{20D0}'..='\u{20FF}').contains(&c)
        || ('\u{FE20}'..='\u{FE2F}').contains(&c)
}

/// Natural sorting comparison for strings (handles numeric segments correctly).
/// Zero-allocation: uses byte-slice indices instead of temporary Strings.
pub fn compare_natural(a: &str, b: &str) -> std::cmp::Ordering {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let mut ai = 0;
    let mut bi = 0;

    loop {
        match (a_bytes.get(ai), b_bytes.get(bi)) {
            (Some(&ac), Some(&bc)) => {
                if ac.is_ascii_digit() && bc.is_ascii_digit() {
                    // Extract numeric segments by index (no allocation)
                    let a_start = ai;
                    while ai < a_bytes.len() && a_bytes[ai].is_ascii_digit() {
                        ai += 1;
                    }
                    let b_start = bi;
                    while bi < b_bytes.len() && b_bytes[bi].is_ascii_digit() {
                        bi += 1;
                    }

                    // Trim leading zeros by advancing start pointers
                    let mut a_trimmed = a_start;
                    while a_trimmed < ai - 1 && a_bytes[a_trimmed] == b'0' {
                        a_trimmed += 1;
                    }
                    let mut b_trimmed = b_start;
                    while b_trimmed < bi - 1 && b_bytes[b_trimmed] == b'0' {
                        b_trimmed += 1;
                    }

                    let a_len = ai - a_trimmed;
                    let b_len = bi - b_trimmed;

                    // Compare by length first (longer number = bigger)
                    if a_len != b_len {
                        return a_len.cmp(&b_len);
                    }
                    // Same length: compare digit-by-digit
                    let cmp = a_bytes[a_trimmed..ai].cmp(&b_bytes[b_trimmed..bi]);
                    if cmp != std::cmp::Ordering::Equal {
                        return cmp;
                    }
                    // Equal value: fewer leading zeros comes first
                    let a_total = ai - a_start;
                    let b_total = bi - b_start;
                    if a_total != b_total {
                        return a_total.cmp(&b_total);
                    }
                } else {
                    // Case-insensitive lexicographical comparison
                    let ac_low = (ac as char).to_lowercase().next().unwrap_or(ac as char);
                    let bc_low = (bc as char).to_lowercase().next().unwrap_or(bc as char);

                    if ac_low != bc_low {
                        return ac_low.cmp(&bc_low);
                    }
                    ai += 1;
                    bi += 1;
                }
            }
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
        }
    }
}

/// Returns (is_hidden, is_system, is_reparse_point) attributes from metadata
pub fn get_file_attributes(metadata: &Metadata, file_name: &str) -> (bool, bool, bool) {
    crate::platform::platform().file_attributes(metadata, file_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_get_file_attributes_basic() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "hello").unwrap();

        let metadata = fs::metadata(&file_path).unwrap();
        let (hidden, system, _) = get_file_attributes(&metadata, "test.txt");

        // On Windows, a new file is usually not hidden or system by default
        assert!(!hidden);
        assert!(!system);
    }
}
