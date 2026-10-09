//! Compression plans hold directory selection and verify sources on encoder handles.
use super::{
    path_parents::HeldParents,
    source_selection::{SelectedEntry, SourceWalk},
};
use std::{
    fs::{File, Metadata},
    io,
    path::{Path, PathBuf},
};
pub(super) struct Entry {
    pub source: SelectedEntry,
    pub name: String,
}
impl Entry {
    pub fn open(
        &self,
        retained: Option<&std::collections::HashMap<PathBuf, File>>,
    ) -> io::Result<(HeldParents, File)> {
        if let Some(file) = retained.and_then(|files| files.get(&self.source.resolved_path())) {
            return self.source.open_retained_archive(file);
        }
        self.source.open_archive()
    }
}
pub(super) fn plan(
    sources: &[PathBuf],
    checkpoint: &mut dyn FnMut() -> io::Result<()>,
) -> io::Result<Vec<Entry>> {
    let mut roots = Vec::with_capacity(sources.len());
    for path in sources {
        checkpoint()?;
        roots.push(SelectedEntry::select(path)?);
    }
    let mut entries = Vec::new();
    for root in roots {
        let source = root.path().to_path_buf();
        let base = source
            .file_name()
            .ok_or_else(|| io::Error::other("Invalid compression source name"))?;
        let mut walk = SourceWalk::new(root);
        while let Some(source_entry) = walk.next(checkpoint)? {
            let relative = source_entry
                .path()
                .strip_prefix(&source)
                .map_err(io::Error::other)?;
            // Empty relative paths must not add a separator to single-file names.
            let archive_path = if relative.as_os_str().is_empty() {
                PathBuf::from(base)
            } else {
                Path::new(base).join(relative)
            };
            entries.push(Entry {
                source: source_entry,
                name: archive_path.to_string_lossy().replace('\\', "/"),
            });
        }
    }
    Ok(entries)
}
/// Match the dependency's from_path timestamps without resolving a pathname again.
pub(super) fn seven_entry(name: String, metadata: &Metadata) -> sevenz_rust2::ArchiveEntry {
    let mut entry = if metadata.is_dir() {
        sevenz_rust2::ArchiveEntry::new_directory(&name)
    } else {
        sevenz_rust2::ArchiveEntry::new_file(&name)
    };
    if let Ok(time) = metadata.modified() {
        if let Ok(date) = time.try_into() {
            entry.last_modified_date = date;
            entry.has_last_modified_date = u64::from(entry.last_modified_date) > 0;
        }
    }
    if let Ok(time) = metadata.created() {
        if let Ok(date) = time.try_into() {
            entry.creation_date = date;
            entry.has_creation_date = u64::from(entry.creation_date) > 0;
        }
    }
    if let Ok(time) = metadata.accessed() {
        if let Ok(date) = time.try_into() {
            entry.access_date = date;
            entry.has_access_date = u64::from(entry.access_date) > 0;
        }
    }
    entry
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    #[test]
    fn compression_rejects_retained_reader_from_another_object_with_identical_bytes() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let foreign = root.path().join("foreign");
        std::fs::write(&source, b"same bytes").unwrap();
        std::fs::write(&foreign, b"same bytes").unwrap();
        let entries = plan(&[source.clone()], &mut || Ok(())).unwrap();
        let retained = std::collections::HashMap::from([(
            entries[0].source.resolved_path(),
            File::open(&foreign).unwrap(),
        )]);
        let error = entries[0].open(Some(&retained)).err().unwrap();
        assert_eq!(
            crate::models::error::SourceSelectionFailure::from_message(&error.to_string()),
            Some(true)
        );
        assert_eq!(std::fs::read(source).unwrap(), b"same bytes");
        assert_eq!(std::fs::read(foreign).unwrap(), b"same bytes");
    }
}
