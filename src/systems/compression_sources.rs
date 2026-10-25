//! Compression plans hold directory selection and verify sources on encoder handles.
use super::{
    path_parents::HeldParents,
    source_selection::{SelectedEntry, SourceWalk},
};
#[cfg(any(not(target_os = "windows"), test))]
use std::fs::Metadata;
use std::{
    fs::File,
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

#[cfg(target_os = "windows")]
pub(super) fn remap(
    entries: &mut [Entry],
    source: &Path,
    destination: &Path,
    limits: crate::utils::archive_limits::ArchiveLimits,
) -> io::Result<()> {
    let budget = crate::utils::archive_limits::ArchiveBudget::new(limits);
    let mut names = std::collections::HashSet::new();
    let mut bytes = 0usize;
    for entry in entries {
        let path = Path::new(&entry.name);
        if let Ok(relative) = path.strip_prefix(source) {
            entry.name = if relative.as_os_str().is_empty() {
                destination.to_path_buf()
            } else {
                destination.join(relative)
            }
            .to_string_lossy()
            .replace('\\', "/");
        }
        crate::utils::archive_security::relative_archive_path(&entry.name)?;
        budget.entry(&entry.name, entry.source.size())?;
        bytes = bytes
            .checked_add(entry.name.len())
            .ok_or_else(|| io::Error::other("Archive names exceeded"))?;
        if entry.name.len() > crate::utils::archive_io_protocol::MAX_NAME
            || bytes > 64 * 1024 * 1024
            || !names.insert(entry.name.to_lowercase())
        {
            return Err(io::Error::other(
                "Archive rename creates invalid or duplicate names",
            ));
        }
    }
    Ok(())
}
#[cfg(any(not(target_os = "windows"), test))]
pub(super) fn plan(
    sources: &[PathBuf],
    checkpoint: &mut dyn FnMut() -> io::Result<()>,
) -> io::Result<Vec<Entry>> {
    plan_controlled(sources, checkpoint, None)
}
pub(super) fn plan_controlled(
    sources: &[PathBuf],
    checkpoint: &mut dyn FnMut() -> io::Result<()>,
    budget: Option<&crate::utils::archive_limits::ArchiveBudget>,
) -> io::Result<Vec<Entry>> {
    if let Some(budget) = budget {
        budget.check_entry_count(sources.len())?;
    }
    let mut check = || {
        if let Some(budget) = budget {
            budget.checkpoint_mut(checkpoint)
        } else {
            checkpoint()
        }
    };
    let mut roots = Vec::with_capacity(sources.len());
    for path in sources {
        check()?;
        roots.push(SelectedEntry::select(path)?);
    }
    let mut entries = Vec::new();
    let mut names = std::collections::HashSet::new();
    let mut name_bytes = 0usize;
    for root in roots {
        let source = root.path().to_path_buf();
        let base = source
            .file_name()
            .ok_or_else(|| io::Error::other("Invalid compression source name"))?;
        let mut walk = SourceWalk::new(root);
        loop {
            if let Some(budget) = budget {
                walk.bound_pending(
                    budget.limits().max_entries.saturating_sub(entries.len()),
                    budget.limits().max_entries,
                );
            }
            let Some(source_entry) = walk.next(&mut check)? else {
                break;
            };
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
            let name = archive_path.to_string_lossy().replace('\\', "/");
            if let Some(budget) = budget {
                budget.entry(&name, source_entry.size())?;
                if name.len() > crate::utils::archive_io_protocol::MAX_NAME {
                    return Err(crate::utils::archive_io_protocol::invalid());
                }
                name_bytes = name_bytes
                    .checked_add(name.len())
                    .filter(|n| *n <= crate::utils::archive_io_state::MAX_MANIFEST_NAMES)
                    .ok_or_else(crate::utils::archive_io_protocol::invalid)?;
                if !names.insert(
                    crate::utils::archive_security::relative_archive_path(&name)?
                        .to_string_lossy()
                        .to_lowercase(),
                ) {
                    return Err(io::Error::other("Ambiguous archive source names"));
                }
            }
            entries.push(Entry {
                source: source_entry,
                name,
            });
        }
    }
    Ok(entries)
}
/// Match the dependency's from_path timestamps without resolving a pathname again.
#[cfg(any(not(target_os = "windows"), test))]
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
        let entries = plan(std::slice::from_ref(&source), &mut || Ok(())).unwrap();
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
    #[test]
    fn bounded_plan_limits_pending_children_before_accumulation_and_validates_names() {
        use crate::models::error::{ArchiveLimitFailure, ArchiveLimitKind};
        use crate::utils::archive_limits::{ArchiveBudget, ArchiveLimits};
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("tree");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("a"), b"1234").unwrap();
        std::fs::write(folder.join("b"), b"1234").unwrap();
        let value = ArchiveLimits {
            max_entries: 2,
            max_bytes: 8,
            max_depth: 2,
            ..ArchiveLimits::EXTRACTION
        };
        let error = plan_controlled(
            std::slice::from_ref(&folder),
            &mut || Ok(()),
            Some(&ArchiveBudget::new(value)),
        )
        .err()
        .unwrap();
        assert_eq!(
            ArchiveLimitFailure::from_message(&error.to_string()),
            Some(ArchiveLimitFailure {
                kind: ArchiveLimitKind::Entries,
                limit: 2
            })
        );
        let value = ArchiveLimits {
            max_entries: 3,
            ..value
        };
        assert_eq!(
            plan_controlled(
                std::slice::from_ref(&folder),
                &mut || Ok(()),
                Some(&ArchiveBudget::new(value))
            )
            .unwrap()
            .len(),
            3
        );
        let error = plan_controlled(
            &[folder],
            &mut || Ok(()),
            Some(&ArchiveBudget::new(ArchiveLimits {
                max_depth: 1,
                ..value
            })),
        )
        .err()
        .unwrap();
        assert_eq!(
            ArchiveLimitFailure::from_message(&error.to_string())
                .unwrap()
                .kind,
            ArchiveLimitKind::Depth
        );
        let left = temp.path().join("left");
        let right = temp.path().join("right");
        std::fs::create_dir(&left).unwrap();
        std::fs::create_dir(&right).unwrap();
        for parent in [&left, &right] {
            std::fs::write(parent.join("same"), b"safe").unwrap();
        }
        assert!(plan_controlled(
            &[left.join("same"), right.join("same")],
            &mut || Ok(()),
            Some(&ArchiveBudget::new(value))
        )
        .is_err());
    }
    #[cfg(target_os = "windows")]
    #[test]
    fn logical_archive_rename_keeps_open_sources_and_revalidates_names() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("folder");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("file"), b"safe").unwrap();
        let mut entries = plan(std::slice::from_ref(&folder), &mut || Ok(())).unwrap();
        let original = entries[1].source.resolved_path();
        remap(
            &mut entries,
            Path::new("folder"),
            Path::new("école"),
            crate::utils::archive_limits::ArchiveLimits::WORKSPACE,
        )
        .unwrap();
        assert_eq!(entries[0].name, "école");
        assert_eq!(entries[1].name, "école/file");
        assert_eq!(entries[1].source.resolved_path(), original);
        let (_parents, file) = entries[1].open(None).unwrap();
        assert_eq!(file.metadata().unwrap().len(), 4);
        assert!(remap(
            &mut entries,
            Path::new("école"),
            Path::new("../escape"),
            crate::utils::archive_limits::ArchiveLimits::WORKSPACE
        )
        .is_err());
        let mut entries = plan(&[folder], &mut || Ok(())).unwrap();
        assert!(remap(
            &mut entries,
            Path::new("folder"),
            Path::new("a/b"),
            crate::utils::archive_limits::ArchiveLimits {
                max_depth: 2,
                ..crate::utils::archive_limits::ArchiveLimits::WORKSPACE
            }
        )
        .is_err());
    }
}
