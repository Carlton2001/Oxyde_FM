use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};

/// The Shell may expose an opaque `$R…` name. Its `$I…` sidecar records
/// the original path, including extensions hidden by the Shell.
pub(super) fn original_path(id: &Path) -> Option<PathBuf> {
    let name = id.file_name()?.to_string_lossy();
    if !name.get(..2)?.eq_ignore_ascii_case("$R") || name.len() <= 2 {
        return None;
    }
    let metadata = id.with_file_name(format!("$I{}", name.get(2..)?));
    parse_original_path(&std::fs::read(metadata).ok()?)
}

fn parse_original_path(bytes: &[u8]) -> Option<PathBuf> {
    let version = u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?);
    let path_bytes = match version {
        // v1: a fixed MAX_PATH buffer immediately after the 24-byte header.
        1 => bytes.get(24..544)?,
        // v2: a UTF-16 code-unit count precedes the variable-length buffer.
        2 => {
            let count = u32::from_le_bytes(bytes.get(24..28)?.try_into().ok()?) as usize;
            bytes.get(28..28usize.checked_add(count.checked_mul(2)?)?)?
        }
        _ => return None,
    };
    // Both formats include a NUL terminator; it must not become part of a
    // Rust filename or cause the shortcut extension correction to append twice.
    let wide: Vec<u16> = path_bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    let path = PathBuf::from(OsString::from_wide(&wide));
    path.is_absolute().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(version: u64, path: &str) -> Vec<u8> {
        let wide: Vec<_> = path.encode_utf16().chain([0]).collect();
        let mut bytes = version.to_le_bytes().to_vec();
        bytes.resize(24, 0);
        if version == 2 {
            bytes.extend_from_slice(&(wide.len() as u32).to_le_bytes());
        }
        for unit in wide {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        if version == 1 {
            bytes.resize(544, 0);
        }
        bytes
    }

    #[test]
    fn original_names_exclude_terminator_and_padding_in_both_formats() {
        for version in [1, 2] {
            for path in [
                r"C:\Documents\Dossier école",
                r"C:\Documents\Rapport école 🦀.txt",
                r"C:\Documents\Raccourci école.lnk",
                r"\\serveur\partage\Lien école.url",
            ] {
                assert_eq!(
                    parse_original_path(&metadata(version, path)),
                    Some(path.into())
                );
            }
        }
    }

    #[test]
    fn truncated_or_unknown_metadata_has_no_original_path() {
        for version in [1, 2] {
            let bytes = metadata(version, r"C:\Documents\Rapport.txt");
            for end in 0..bytes.len() {
                assert_eq!(parse_original_path(&bytes[..end]), None);
            }
        }
        assert_eq!(
            parse_original_path(&metadata(3, r"C:\Documents\Rapport.txt")),
            None
        );
        let mut bytes = metadata(2, r"C:\Documents\Rapport.txt");
        bytes[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(parse_original_path(&bytes), None);
    }

    #[test]
    fn empty_or_relative_original_paths_are_rejected() {
        for version in [1, 2] {
            for path in ["", "Rapport.txt", r"C:Rapport.txt"] {
                assert_eq!(parse_original_path(&metadata(version, path)), None);
            }
        }
    }
}
