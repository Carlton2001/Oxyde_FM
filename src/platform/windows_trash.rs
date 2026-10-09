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
    let mut file = super::platform()
        .open_archive_source(&metadata, false)
        .ok()?;
    parse_original_path(&metadata_bytes(&mut file).ok()?)
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
            let name = path
                .file_name()
                .ok_or("Invalid trash filename")?
                .to_string_lossy();
            let metadata_path = path.with_file_name(format!("$I{}", &name[2..]));
            let mut metadata_file = super::platform()
                .open_archive_source(&metadata_path, false)
                .map_err(|error| error.to_string())?;
            let bytes = metadata_bytes(&mut metadata_file).map_err(|error| error.to_string())?;
            let original_path = parse_original_path(&bytes)
                .ok_or_else(|| format!("Cannot read original trash name: {}", path.display()))?;
            let metadata_identity =
                crate::models::transaction::HistoryFileIdentity::from_file(&metadata_file)
                    .map_err(|error| error.to_string())?;
            let metadata_hash = blake3::hash(&bytes).to_hex().to_string();
            Ok(super::TrashTransferEntry {
                path: path.clone(),
                original_path,
                metadata_path,
                metadata_identity,
                metadata_hash,
            })
        })
        .collect()
}

pub(super) fn detach_metadata(
    entry: &super::TrashTransferEntry,
) -> Result<Option<super::DetachedTrashMetadata>, String> {
    detach_metadata_checked(entry).map_err(|error| error.to_string())
}

fn detach_metadata_checked(
    entry: &super::TrashTransferEntry,
) -> std::io::Result<Option<super::DetachedTrashMetadata>> {
    if entry.path.try_exists()? {
        return Ok(None);
    }
    let parents = crate::systems::path_parents::HeldParents::new(&entry.metadata_path)?;
    let source = parents.resolved_path(&entry.metadata_path)?;
    // This handle denies both writers and replacement until validation and rename
    // finish. No hard-link-by-name or subsequent unlink can adopt another object.
    let mut file = super::platform().open_copied_file(parents.directory(), &source)?;
    if !entry.metadata_identity.matches(&file)? {
        return Err(std::io::Error::other(
            "Trash metadata was replaced; preserved",
        ));
    }
    let bytes = metadata_bytes(&mut file)?;
    if parse_original_path(&bytes).as_ref() != Some(&entry.original_path)
        || blake3::hash(&bytes).to_hex().as_str() != entry.metadata_hash
    {
        return Err(std::io::Error::other(format!(
            "Trash metadata changed during transfer: {}",
            entry.path.display()
        )));
    }
    // Capture before mutation: a sidecar without a usable proof stays restorable.
    let identity = crate::models::transaction::HistoryMoveIdentity::from_file(&file)?;
    let hash = blake3::hash(&bytes).to_hex().to_string();
    let backup = entry
        .metadata_path
        .with_file_name(format!(".oxyde-trash-{}.tmp", uuid::Uuid::new_v4()));
    super::platform().rename_operation_file(
        &file,
        &source,
        parents.directory(),
        &parents.resolved_path(&backup)?,
        false,
    )?;
    // The backup already exists and must remain recoverable if cosmetic hiding
    // fails. Update the retained object, never a substituted path.
    if let Err(error) = hide_metadata(&file) {
        log::warn!(
            "Detached trash metadata remains recoverable at {}: {error}",
            backup.display()
        );
    }
    Ok(Some(super::DetachedTrashMetadata {
        source: entry.metadata_path.clone(),
        backup,
        identity,
        hash,
    }))
}

// Covers the maximal Windows UTF-16 path plus the metadata header, without
// trusting a malformed length or allocating an unbounded sidecar.
const MAX_METADATA_BYTES: u64 = 128 * 1024;
fn metadata_bytes(file: &mut std::fs::File) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    if file.metadata()?.len() > MAX_METADATA_BYTES {
        return Err(std::io::Error::other("Trash metadata is too large"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_METADATA_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        return Err(std::io::Error::other("Trash metadata is too large"));
    }
    Ok(bytes)
}

fn hide_metadata(file: &std::fs::File) -> std::io::Result<()> {
    use std::os::windows::{fs::MetadataExt, io::AsRawHandle};
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{FILE_BASIC_INFO, FileBasicInfo, SetFileInformationByHandle},
    };
    let attributes = file.metadata()?.file_attributes();
    let info = FILE_BASIC_INFO {
        FileAttributes: (attributes & !0x80) | 2,
        ..Default::default()
    };
    unsafe {
        SetFileInformationByHandle(
            HANDLE(file.as_raw_handle()),
            FileBasicInfo,
            (&info as *const FILE_BASIC_INFO).cast(),
            std::mem::size_of_val(&info) as u32,
        )
        .map_err(|error| std::io::Error::from_raw_os_error((error.code().0 as u32 & 0xffff) as i32))
    }
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

    fn selected_metadata(
        path: PathBuf,
        sidecar: &Path,
        original: PathBuf,
    ) -> super::super::TrashTransferEntry {
        super::super::TrashTransferEntry {
            path,
            metadata_path: sidecar.to_path_buf(),
            original_path: original,
            metadata_identity: crate::models::transaction::HistoryFileIdentity::from_file(
                &std::fs::File::open(sidecar).unwrap(),
            )
            .unwrap(),
            metadata_hash: blake3::hash(&std::fs::read(sidecar).unwrap())
                .to_hex()
                .to_string(),
        }
    }

    #[test]
    fn metadata_detach_preserves_same_content_replacements_and_in_place_edits_since_selection() {
        for replacement in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("$Rfixture.txt");
            let sidecar = root.path().join("$Ifixture.txt");
            let saved = root.path().join("saved-metadata");
            let original = root.path().join("selected.txt");
            let bytes = metadata(2, &original.to_string_lossy());
            std::fs::write(&sidecar, &bytes).unwrap();
            let entry = selected_metadata(path, &sidecar, original.clone());
            let expected = if replacement {
                std::fs::rename(&sidecar, &saved).unwrap();
                bytes.clone()
            } else {
                let mut changed = bytes.clone();
                changed[8] = 42; // Alter size metadata, retain the exact original path.
                changed
            };
            std::fs::write(&sidecar, &expected).unwrap();
            assert!(detach_metadata(&entry).is_err());
            assert_eq!(std::fs::read(&sidecar).unwrap(), expected);
            if replacement {
                assert_eq!(std::fs::read(&saved).unwrap(), bytes);
                std::fs::remove_file(&sidecar).unwrap();
                std::fs::rename(&saved, &sidecar).unwrap();
            } else {
                std::fs::write(&sidecar, &bytes).unwrap();
            }
            let detached = detach_metadata(&entry).unwrap().unwrap();
            assert_eq!(std::fs::read(&detached.backup).unwrap(), bytes);
        }
    }

    #[test]
    fn detached_metadata_is_renamed_by_handle_and_returns_a_durable_content_proof() {
        for version in [1, 2] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("$Rfixture.txt");
            let sidecar = root.path().join("$Ifixture.txt");
            let original = root.path().join("Documents/Rapport.txt");
            let bytes = metadata(version, &original.to_string_lossy());
            std::fs::write(&sidecar, &bytes).unwrap();
            let entry = selected_metadata(path, &sidecar, original);
            let detached = detach_metadata(&entry).unwrap().unwrap();
            assert_eq!(detached.source, sidecar);
            assert!(!sidecar.exists());
            assert_eq!(std::fs::read(&detached.backup).unwrap(), bytes);
            assert_eq!(detached.hash, blake3::hash(&bytes).to_hex().to_string());
            let file = std::fs::File::open(&detached.backup).unwrap();
            assert!(
                detached
                    .identity
                    .snapshot()
                    .unwrap()
                    .matches(&file)
                    .unwrap()
            );
            assert!(
                super::super::platform()
                    .file_attributes(&file.metadata().unwrap(), "")
                    .0
            );
            drop(file);
            let saved = root.path().join("saved-metadata");
            std::fs::rename(&detached.backup, &saved).unwrap();
            std::fs::rename(&saved, &detached.backup).unwrap();
            crate::systems::file_copy::remove_verified_output_identity(
                &detached.backup,
                &detached.hash,
                detached.identity.snapshot(),
            )
            .unwrap();
            assert!(!detached.backup.exists());
        }
    }

    #[test]
    fn metadata_detach_refuses_changed_or_oversized_source_before_any_effect() {
        for oversized in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("$Rfixture.txt");
            let sidecar = root.path().join("$Ifixture.txt");
            let original = root.path().join("expected.txt");
            let bytes = if oversized {
                vec![0; 128 * 1024 + 1]
            } else {
                metadata(2, &root.path().join("independent.txt").to_string_lossy())
            };
            std::fs::write(&sidecar, &bytes).unwrap();
            let entry = selected_metadata(path, &sidecar, original);
            assert!(detach_metadata(&entry).is_err());
            assert_eq!(std::fs::read(&sidecar).unwrap(), bytes);
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
        }
    }

    #[test]
    fn metadata_detach_preserves_a_payload_that_has_not_been_removed() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("$Rfixture.txt");
        let sidecar = root.path().join("$Ifixture.txt");
        std::fs::write(&path, b"remaining payload").unwrap();
        std::fs::write(&sidecar, b"remaining metadata").unwrap();
        let entry = selected_metadata(path.clone(), &sidecar, root.path().join("original.txt"));
        assert!(detach_metadata(&entry).unwrap().is_none());
        assert_eq!(std::fs::read(path).unwrap(), b"remaining payload");
        assert_eq!(std::fs::read(sidecar).unwrap(), b"remaining metadata");
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
