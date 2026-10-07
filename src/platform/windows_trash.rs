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

pub(super) fn transfer_entries(
    paths: &[PathBuf],
) -> Result<Vec<super::TrashTransferEntry>, String> {
    // Most transfers never touch the Recycle Bin: avoid a Shell enumeration.
    let candidates: Vec<_> = paths
        .iter()
        .filter(|path| {
            path.file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .to_ascii_lowercase()
                    .starts_with("$r")
            }) && path
                .parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("$Recycle.bin"))
        })
        .collect();
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let normalize = |path: &Path| {
        path.to_string_lossy()
            .replace('/', "\\")
            .trim_start_matches("\\\\?\\")
            .to_lowercase()
    };
    let items = trash::os_limited::list().map_err(|error| error.to_string())?;
    candidates
        .into_iter()
        .map(|path| {
            if !items
                .iter()
                .any(|item| normalize(Path::new(&item.id)) == normalize(path))
            {
                return Err(format!(
                    "Trash entry is no longer available: {}",
                    path.display()
                ));
            }
            let original_path = original_path(path)
                .ok_or_else(|| format!("Cannot read original trash name: {}", path.display()))?;
            let name = path
                .file_name()
                .ok_or("Invalid trash filename")?
                .to_string_lossy();
            Ok(super::TrashTransferEntry {
                path: path.clone(),
                original_path,
                metadata_path: path.with_file_name(format!("$I{}", &name[2..])),
            })
        })
        .collect()
}

pub(super) fn detach_metadata(
    entry: &super::TrashTransferEntry,
) -> Result<Option<(PathBuf, PathBuf)>, String> {
    if entry.path.try_exists().map_err(|error| error.to_string())? {
        return Ok(None);
    }
    if original_path(&entry.path).as_ref() != Some(&entry.original_path) {
        return Err(format!(
            "Trash metadata changed during transfer: {}",
            entry.path.display()
        ));
    }
    let backup = entry
        .metadata_path
        .with_file_name(format!(".oxyde-trash-{}.tmp", uuid::Uuid::new_v4()));
    // A hard link reserves a unique name on the source volume without overwriting.
    std::fs::hard_link(&entry.metadata_path, &backup).map_err(|error| error.to_string())?;
    let result = super::platform()
        .set_hidden_attribute(&backup, true)
        .and_then(|()| {
            std::fs::remove_file(&entry.metadata_path).map_err(|error| error.to_string())
        });
    if let Err(error) = result {
        let _ = std::fs::remove_file(&backup);
        return Err(error);
    }
    Ok(Some((entry.metadata_path.clone(), backup)))
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
