//! Restore the selected payload itself; never overwrite or retry a guessed name.
use crate::{
    models::error::{TrashRestoreFailure, TrashRestoreIssue},
    systems::path_parents::HeldParents,
};
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};
fn failure(issue: TrashRestoreIssue, path: &Path, detail: impl ToString) -> io::Error {
    io::Error::other(TrashRestoreFailure {
        issue,
        path: path.to_string_lossy().into_owned(),
        detail: detail.to_string(),
    })
}
fn normalized(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_start_matches("\\\\?\\")
        .to_lowercase()
}
struct Selection {
    payload: File,
    metadata: File,
    source: PathBuf,
    metadata_path: PathBuf,
    original: PathBuf,
    destination: HeldParents,
    _source_parents: HeldParents,
}
impl Selection {
    fn new(item: &trash::TrashItem) -> io::Result<Self> {
        use TrashRestoreIssue::{Changed, Unavailable};
        let source = PathBuf::from(&item.id);
        let original = item.original_path();
        let leaf = source
            .file_name()
            .ok_or_else(|| failure(Unavailable, &source, "Missing payload name"))?
            .to_string_lossy();
        if !leaf
            .get(..2)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("$R"))
            || leaf.len() <= 2
            || !original.is_absolute()
            || !source.is_absolute()
            || !source
                .parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("$Recycle.Bin"))
        {
            return Err(failure(Unavailable, &source, "Invalid trash selection"));
        }
        let parents =
            HeldParents::new(&source).map_err(|error| failure(Unavailable, &source, error))?;
        let metadata_path = source.with_file_name(format!("$I{}", &leaf[2..]));
        let mut metadata = super::platform()
            .open_copied_file(parents.directory(), &parents.resolved_path(&metadata_path)?)
            .map_err(|error| failure(Unavailable, &metadata_path, error))?;
        let bytes = super::windows_trash::metadata_bytes(&mut metadata)
            .map_err(|error| failure(Unavailable, &metadata_path, error))?;
        let mut captured_original = super::windows_trash::parse_original_path(&bytes)
            .ok_or_else(|| failure(Unavailable, &metadata_path, "Invalid original path"))?;
        // Preserve the existing shortcut correction, before any native move.
        if let Some(extension) = source.extension().filter(|extension| {
            ["lnk", "url"]
                .iter()
                .any(|value| extension.to_string_lossy().eq_ignore_ascii_case(value))
        }) {
            if !captured_original.extension().is_some_and(|current| {
                current
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&extension.to_string_lossy())
            }) {
                let mut name = captured_original
                    .file_name()
                    .ok_or_else(|| failure(Unavailable, &metadata_path, "Missing original name"))?
                    .to_os_string();
                name.push(".");
                name.push(extension);
                captured_original.set_file_name(name);
            }
        }
        if normalized(&captured_original) != normalized(&original) {
            return Err(failure(
                Changed,
                &source,
                "Original trash path changed since selection",
            ));
        }
        let payload = super::platform()
            .open_rename_item(parents.directory(), &parents.resolved_path(&source)?)
            .map_err(|error| failure(Unavailable, &source, error))?;
        let destination =
            HeldParents::new(&original).map_err(|error| failure(Unavailable, &original, error))?;
        Ok(Self {
            payload,
            metadata,
            source: parents.resolved_path(&source)?,
            metadata_path: parents.resolved_path(&metadata_path)?,
            original,
            destination,
            _source_parents: parents,
        })
    }
    fn restore(self, checkpoint: &mut dyn FnMut() -> io::Result<()>) -> io::Result<()> {
        checkpoint()?;
        self.destination
            .rename(&self.payload, &self.source, &self.original, false)
            .map_err(|error| {
                failure(
                    if error.kind() == io::ErrorKind::AlreadyExists {
                        TrashRestoreIssue::Conflict
                    } else {
                        TrashRestoreIssue::Unavailable
                    },
                    &self.original,
                    error,
                )
            })?;
        // Restoration has succeeded. Failure of sidecar cleanup must not report
        // that the payload is still in the bin or trigger another name-based move.
        if let Err(error) =
            super::platform().remove_copied_file(&self.metadata, &self.metadata_path)
        {
            log::warn!(
                "Restored {}, but trash metadata remains: {error}",
                self.original.display()
            );
        }
        Ok(())
    }
}
pub(super) fn restore(items: Vec<trash::TrashItem>) -> Result<(), String> {
    // Preflight every selected pair before starting a batch. Each native rename
    // remains exclusive if a destination appears after this preparation.
    let selected = items
        .iter()
        .map(Selection::new)
        .collect::<io::Result<Vec<_>>>()
        .map_err(|error| error.to_string())?;
    for item in selected {
        item.restore(&mut || Ok(()))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn metadata(original: &Path, version: u64) -> Vec<u8> {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<_> = original.as_os_str().encode_wide().chain([0]).collect();
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
    struct Fixture {
        _root: tempfile::TempDir,
        source: PathBuf,
        sidecar: PathBuf,
        original: PathBuf,
        item: trash::TrashItem,
    }
    impl Fixture {
        fn new(name: &str, version: u64) -> Self {
            let root = tempfile::tempdir().unwrap();
            let bin = root.path().join("$Recycle.Bin/test-user");
            let destination = root.path().join("original");
            fs::create_dir_all(&bin).unwrap();
            fs::create_dir(&destination).unwrap();
            let source = bin.join(format!(
                "$Rfixture{}",
                Path::new(name)
                    .extension()
                    .map(|s| format!(".{}", s.to_string_lossy()))
                    .unwrap_or_default()
            ));
            let sidecar = source.with_file_name(
                source
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .replacen("$R", "$I", 1),
            );
            let original = destination.join(name);
            fs::write(&source, b"selected contents").unwrap();
            fs::write(&sidecar, metadata(&original, version)).unwrap();
            let item = trash::TrashItem {
                id: source.clone().into_os_string(),
                name: original.file_name().unwrap().to_owned(),
                original_parent: destination,
                time_deleted: 0,
            };
            Self {
                _root: root,
                source,
                sidecar,
                original,
                item,
            }
        }
        fn select(&self) -> Selection {
            Selection::new(&self.item).unwrap()
        }
        fn preserved(&self) {
            assert!(self.source.exists());
            assert!(self.sidecar.exists());
        }
    }
    #[test]
    fn native_restore_preserves_object_streams_and_shortcut_names_in_both_formats() {
        use crate::models::transaction::HistoryFileIdentity;
        for version in [1, 2] {
            for name in ["école.txt", "école.lnk", "école.url"] {
                let fixture = Fixture::new(name, version);
                let ads = format!("{}:fixture", fixture.source.display());
                fs::write(&ads, b"alternate stream").unwrap();
                let proof =
                    HistoryFileIdentity::from_file(&File::open(&fixture.source).unwrap()).unwrap();
                // Simulate a Shell display name with the shortcut extension hidden.
                if name.ends_with(".lnk") || name.ends_with(".url") {
                    fs::write(
                        &fixture.sidecar,
                        metadata(&fixture.original.with_extension(""), version),
                    )
                    .unwrap();
                }
                restore(vec![fixture.item.clone()]).unwrap();
                assert!(!fixture.source.exists());
                assert!(!fixture.sidecar.exists());
                assert_eq!(fs::read(&fixture.original).unwrap(), b"selected contents");
                assert_eq!(
                    fs::read(format!("{}:fixture", fixture.original.display())).unwrap(),
                    b"alternate stream"
                );
                assert!(
                    proof
                        .matches(&File::open(&fixture.original).unwrap())
                        .unwrap()
                );
            }
        }
    }
    #[test]
    fn native_restore_keeps_existing_and_concurrently_created_destinations() {
        for directory in [false, true] {
            for concurrent in [false, true] {
                let fixture = Fixture::new("selected.txt", 2);
                let make_destination = || {
                    if directory {
                        fs::create_dir(&fixture.original).unwrap();
                    }
                    let witness = if directory {
                        fixture.original.join("witness.txt")
                    } else {
                        fixture.original.clone()
                    };
                    fs::write(witness, b"independent contents").unwrap();
                };
                if !concurrent {
                    make_destination();
                }
                let selected = fixture.select();
                let error = selected
                    .restore(&mut || {
                        if concurrent {
                            make_destination();
                        }
                        Ok(())
                    })
                    .unwrap_err();
                assert_eq!(
                    TrashRestoreFailure::from_message(&error.to_string()),
                    Some(TrashRestoreIssue::Conflict)
                );
                fixture.preserved();
                let witness = if directory {
                    fixture.original.join("witness.txt")
                } else {
                    fixture.original.clone()
                };
                assert_eq!(fs::read(witness).unwrap(), b"independent contents");
            }
        }
    }
    #[test]
    fn native_restore_retains_payload_and_metadata_until_publication() {
        let fixture = Fixture::new("selected.txt", 2);
        let selected = fixture.select();
        selected
            .restore(&mut || {
                for source in [&fixture.source, &fixture.sidecar] {
                    assert!(fs::rename(source, source.with_extension("displaced")).is_err());
                    assert!(fs::write(source, b"replacement").is_err());
                    assert!(fs::remove_file(source).is_err());
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(fs::read(&fixture.original).unwrap(), b"selected contents");
        assert!(!fixture.sidecar.exists());
    }
    #[test]
    fn native_restore_rejects_changed_malformed_oversized_or_foreign_metadata() {
        for mode in 0..4 {
            let fixture = Fixture::new("selected.txt", 2);
            let mut item = fixture.item.clone();
            let bytes = match mode {
                0 => metadata(&fixture.original.with_file_name("independent.txt"), 2),
                1 => vec![0; 24],
                2 => vec![0; 128 * 1024 + 1],
                _ => {
                    item.id = fixture._root.path().join("$Rforeign.txt").into_os_string();
                    fs::write(Path::new(&item.id), b"independent contents").unwrap();
                    fs::write(
                        fixture._root.path().join("$Iforeign.txt"),
                        metadata(&fixture.original, 2),
                    )
                    .unwrap();
                    metadata(&fixture.original, 2)
                }
            };
            fs::write(&fixture.sidecar, &bytes).unwrap();
            assert!(restore(vec![item]).is_err());
            fixture.preserved();
            assert!(!fixture.original.exists());
            assert_eq!(fs::read(&fixture.sidecar).unwrap(), bytes);
        }
    }
    #[test]
    fn native_restore_moves_a_directory_or_junction_entry_without_traversal() {
        use super::super::windows_reparse_tests::junction;
        for reparse in [false, true] {
            let fixture = Fixture::new("selected", 2);
            fs::remove_file(&fixture.source).unwrap();
            let external = fixture._root.path().join("external");
            fs::create_dir(&external).unwrap();
            let witness = external.join("witness.txt");
            fs::write(&witness, b"independent contents").unwrap();
            if reparse {
                junction(&fixture.source, &external);
            } else {
                fs::create_dir(&fixture.source).unwrap();
                fs::write(fixture.source.join("child"), b"child").unwrap();
            }
            restore(vec![fixture.item.clone()]).unwrap();
            assert!(!fixture.source.exists());
            assert!(!fixture.sidecar.exists());
            if reparse {
                fs::remove_dir(&fixture.original).unwrap();
            } else {
                assert_eq!(fs::read(fixture.original.join("child")).unwrap(), b"child");
            }
            assert_eq!(fs::read(witness).unwrap(), b"independent contents");
        }
    }
    #[test]
    fn native_restore_uses_the_retained_destination_despite_junction_retargeting() {
        use super::super::windows_reparse_tests::{Retarget, junction};
        let fixture = Fixture::new("selected.txt", 2);
        let alias = fixture._root.path().join("alias");
        let foreign = fixture._root.path().join("foreign");
        let template = fixture._root.path().join("template");
        fs::create_dir(&foreign).unwrap();
        junction(&alias, fixture.original.parent().unwrap());
        junction(&template, &foreign);
        let retarget = Retarget::new(&alias, &template);
        let mut item = fixture.item.clone();
        item.original_parent = alias.clone();
        fs::write(&fixture.sidecar, metadata(&item.original_path(), 2)).unwrap();
        Selection::new(&item)
            .unwrap()
            .restore(&mut || {
                retarget.apply();
                Ok(())
            })
            .unwrap();
        assert_eq!(fs::read(&fixture.original).unwrap(), b"selected contents");
        assert!(!foreign.join("selected.txt").exists());
        drop(retarget);
        fs::remove_dir(alias).unwrap();
        fs::remove_dir(template).unwrap();
    }
    #[test]
    fn native_restore_preserves_the_pair_if_the_original_parent_is_missing() {
        let fixture = Fixture::new("selected.txt", 2);
        fs::remove_dir(fixture.original.parent().unwrap()).unwrap();
        let error = restore(vec![fixture.item.clone()]).unwrap_err();
        assert_eq!(
            TrashRestoreFailure::from_message(&error),
            Some(TrashRestoreIssue::Unavailable)
        );
        fixture.preserved();
        assert!(!fixture.original.parent().unwrap().exists());
    }
    #[test]
    fn native_restore_reports_success_when_only_metadata_cleanup_is_refused() {
        let fixture = Fixture::new("selected.txt", 2);
        let linked = fixture._root.path().join("independent-metadata-link");
        fs::hard_link(&fixture.sidecar, &linked).unwrap();
        let original_bytes = fs::read(&linked).unwrap();
        let mut permissions = fs::metadata(&linked).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&linked, permissions.clone()).unwrap();
        let result = restore(vec![fixture.item.clone()]);
        let readonly = fs::metadata(&linked).unwrap().permissions().readonly();
        permissions.set_readonly(false);
        fs::set_permissions(&linked, permissions).unwrap();
        result.unwrap();
        assert!(readonly);
        assert!(!fixture.source.exists());
        assert!(fixture.sidecar.exists());
        assert_eq!(fs::read(&fixture.original).unwrap(), b"selected contents");
        assert_eq!(fs::read(linked).unwrap(), original_bytes);
    }
    #[test]
    fn native_restore_preflights_the_whole_batch_without_touching_the_real_bin() {
        let first = Fixture::new("first.txt", 2);
        let second = Fixture::new("second.txt", 2);
        fs::write(&second.sidecar, b"invalid").unwrap();
        assert!(restore(vec![first.item.clone(), second.item.clone()]).is_err());
        first.preserved();
        second.preserved();
        assert!(!first.original.exists());
    }
}
