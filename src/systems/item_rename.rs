//! Non-replacing rename shared by direct actions and history.
use super::path_parents::HeldParents;
use crate::platform::platform;
use std::{io, path::Path};

pub(crate) fn rename(source: &Path, destination: &Path) -> io::Result<()> {
    rename_with_checkpoint(source, destination, &mut || Ok(()))
}

pub(crate) fn rename_with_checkpoint(
    source: &Path,
    destination: &Path,
    checkpoint: &mut dyn FnMut() -> io::Result<()>,
) -> io::Result<()> {
    if source == destination {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid rename destination",
        ));
    }
    let parents = HeldParents::new(source)?;
    // Direct UI renames stay in one directory. Existing history entries may
    // refer to distinct parents; retain both without weakening exclusive publication.
    let separate_destination = if source.parent() != destination.parent() {
        Some(HeldParents::new(destination)?)
    } else {
        None
    };
    let destination_parents = separate_destination.as_ref().unwrap_or(&parents);
    let source = parents.resolved_path(source)?;
    let destination = destination_parents.resolved_path(destination)?;
    match std::fs::symlink_metadata(&destination) {
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Rename target already exists",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let file = platform().open_rename_item(parents.directory(), &source)?;
    checkpoint()?;
    for parent in [parents.directory(), destination_parents.directory()] {
        let metadata = parent.metadata()?;
        if !metadata.is_dir() || platform().file_attributes(&metadata, "").2 {
            return Err(io::Error::other(
                "Rename parent changed during the operation",
            ));
        }
    }
    platform()
        .rename_operation_item(
            &file,
            &source,
            parents.directory(),
            destination_parents.directory(),
            &destination,
        )
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                // Keep the existing translated conflict message independent of OS locale.
                io::Error::new(io::ErrorKind::AlreadyExists, "Rename target already exists")
            } else {
                error
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(path: &Path) -> crate::platform::FileIdentity {
        let parents = HeldParents::new(path).unwrap();
        let file = platform()
            .open_rename_item(parents.directory(), &parents.resolved_path(path).unwrap())
            .unwrap();
        platform().file_identity(&file).unwrap()
    }

    #[test]
    fn late_rename_destination_preserves_both_files() {
        for separate_parent in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let source = temp.path().join("source.txt");
            let target_parent = if separate_parent {
                let parent = temp.path().join("another-directory");
                std::fs::create_dir(&parent).unwrap();
                parent
            } else {
                temp.path().to_path_buf()
            };
            let destination = target_parent.join("target.txt");
            std::fs::write(&source, b"selected source").unwrap();
            let result = rename_with_checkpoint(&source, &destination, &mut || {
                std::fs::write(&destination, b"late independent file")
            });
            let error = result.expect_err("late rename destination was overwritten");
            assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
            assert_eq!(error.to_string(), "Rename target already exists");
            assert_eq!(std::fs::read(&source).unwrap(), b"selected source");
            assert_eq!(
                std::fs::read(&destination).unwrap(),
                b"late independent file"
            );
        }
    }

    #[test]
    fn rename_preserves_nonempty_directories_and_rejects_late_directory_targets() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let destination = temp.path().join("renamed");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("child.txt"), b"selected child").unwrap();
        let selected_identity = identity(&source);
        let error = rename_with_checkpoint(&source, &destination, &mut || {
            std::fs::create_dir(&destination)?;
            std::fs::write(destination.join("witness.txt"), b"independent child")
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read(source.join("child.txt")).unwrap(),
            b"selected child"
        );
        assert_eq!(
            std::fs::read(destination.join("witness.txt")).unwrap(),
            b"independent child"
        );
        let destination = temp.path().join("available");
        rename(&source, &destination).unwrap();
        assert!(!source.exists());
        assert_eq!(
            std::fs::read(destination.join("child.txt")).unwrap(),
            b"selected child"
        );
        assert_eq!(identity(&destination), selected_identity);
    }

    #[test]
    fn cancelled_rename_leaves_source_in_place() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.txt");
        let destination = temp.path().join("target.txt");
        std::fs::write(&source, b"selected source").unwrap();
        let error = rename_with_checkpoint(&source, &destination, &mut || {
            Err(io::Error::new(io::ErrorKind::Interrupted, "Cancelled"))
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(std::fs::read(source).unwrap(), b"selected source");
        assert!(!destination.exists());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn rename_source_cannot_be_substituted_after_it_is_opened() {
        let temp = tempfile::tempdir().unwrap();
        for directory in [false, true] {
            let source = temp.path().join(if directory { "folder" } else { "file" });
            let destination = source.with_extension("renamed");
            let displaced = source.with_extension("displaced");
            if directory {
                std::fs::create_dir(&source).unwrap();
                std::fs::write(source.join("child"), b"selected").unwrap();
            } else {
                std::fs::write(&source, b"selected").unwrap();
            }
            let selected_identity = identity(&source);
            rename_with_checkpoint(&source, &destination, &mut || {
                assert!(std::fs::rename(&source, &displaced).is_err());
                Ok(())
            })
            .unwrap();
            assert!(!source.exists());
            assert!(!displaced.exists());
            assert_eq!(identity(&destination), selected_identity);
            assert_eq!(
                std::fs::read(if directory {
                    destination.join("child")
                } else {
                    destination
                })
                .unwrap(),
                b"selected"
            );
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn rename_stays_in_the_bound_parent_when_a_junction_is_retargeted() {
        use crate::platform::windows_reparse_tests::{Retarget, junction};
        let temp = tempfile::tempdir().unwrap();
        let selected = temp.path().join("selected");
        let outside = temp.path().join("outside");
        std::fs::create_dir(&selected).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(selected.join("old"), b"selected").unwrap();
        std::fs::write(outside.join("old"), b"outside source").unwrap();
        std::fs::write(outside.join("new"), b"outside destination").unwrap();
        let link = temp.path().join("link");
        let template = temp.path().join("template");
        junction(&link, &selected);
        junction(&template, &outside);
        let retarget = Retarget::new(&link, &template);
        rename_with_checkpoint(&link.join("old"), &link.join("new"), &mut || {
            retarget.apply();
            Ok(())
        })
        .unwrap();
        assert!(!selected.join("old").exists());
        assert_eq!(std::fs::read(selected.join("new")).unwrap(), b"selected");
        assert_eq!(
            std::fs::read(outside.join("old")).unwrap(),
            b"outside source"
        );
        assert_eq!(
            std::fs::read(outside.join("new")).unwrap(),
            b"outside destination"
        );
        drop(retarget);
        std::fs::remove_dir(link).unwrap();
        std::fs::remove_dir(template).unwrap();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn rename_changes_a_junction_entry_without_touching_its_target() {
        use crate::platform::windows_reparse_tests::junction;
        let temp = tempfile::tempdir().unwrap();
        let outside = temp.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("witness"), b"untouched").unwrap();
        let source = temp.path().join("link");
        let destination = temp.path().join("renamed-link");
        junction(&source, &outside);
        rename(&source, &destination).unwrap();
        assert!(!source.exists());
        assert!(
            platform()
                .file_attributes(&std::fs::symlink_metadata(&destination).unwrap(), "")
                .2
        );
        assert_eq!(
            std::fs::read(destination.join("witness")).unwrap(),
            b"untouched"
        );
        assert_eq!(
            std::fs::read(outside.join("witness")).unwrap(),
            b"untouched"
        );
        std::fs::remove_dir(destination).unwrap();
    }
}
