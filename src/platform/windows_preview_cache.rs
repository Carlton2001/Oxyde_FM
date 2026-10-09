//! Bind optional preview-cache publication to native directory and file handles.
use std::fs::File;
use std::io;
use std::os::windows::fs::MetadataExt;
use std::path::Path;
use windows::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE};
use windows::Win32::Storage::FileSystem::{DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE};

fn ordinary_directory(directory: &File) -> io::Result<()> {
    let metadata = directory.metadata()?;
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
        return Err(io::Error::other(
            "Preview cache parent is not an ordinary directory",
        ));
    }
    Ok(())
}

fn ensure_parents(parent: &Path) -> io::Result<Vec<File>> {
    let mut held: Vec<File> = Vec::new();
    for ancestor in parent.ancestors().collect::<Vec<_>>().into_iter().rev() {
        let directory = if let Some(previous) = held.last() {
            ordinary_directory(previous)?;
            let leaf = ancestor
                .file_name()
                .ok_or_else(|| io::Error::other("Cache directory has no name"))?;
            let (directory, _) =
                super::windows_no_reparse::ensure_directory_relative(previous, leaf)?;
            ordinary_directory(previous)?;
            directory
        } else {
            super::windows_no_reparse::open(
                ancestor,
                true,
                GENERIC_READ.0,
                FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0,
                1,
            )?
        };
        ordinary_directory(&directory)?;
        held.push(directory);
    }
    Ok(held)
}

struct StagedFile {
    file: File,
    published: bool,
}
impl Drop for StagedFile {
    fn drop(&mut self) {
        if !self.published {
            if let Err(error) = super::windows_file_moves::remove(&self.file) {
                log::warn!("Could not clean held preview cache staging file: {error}");
            }
        }
    }
}

pub(super) fn write(
    path: &Path,
    temporary: &Path,
    write: &mut dyn FnMut(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let path = std::path::absolute(path)?;
    let temporary = std::path::absolute(temporary)?;
    if path == temporary || path.parent() != temporary.parent() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Cache staging requires a separate sibling path",
        ));
    }
    let leaf = temporary
        .file_name()
        .ok_or_else(|| io::Error::other("Cache staging has no name"))?;
    // Validate the destination too, before creating directories or staging.
    super::windows_no_reparse::leaf_name(
        path.file_name()
            .ok_or_else(|| io::Error::other("Cache has no name"))?,
    )?;
    super::windows_no_reparse::leaf_name(leaf)?;
    let held = ensure_parents(
        path.parent()
            .ok_or_else(|| io::Error::other("Cache has no parent"))?,
    )?;
    let parent = held
        .last()
        .ok_or_else(|| io::Error::other("Cache has no parent handle"))?;
    let mut staged = StagedFile {
        file: super::windows_no_reparse::create_relative(
            parent,
            leaf,
            false,
            GENERIC_READ.0 | GENERIC_WRITE.0 | DELETE.0,
            FILE_SHARE_READ.0,
            0x80,
        )?,
        published: false,
    };
    // The created child pins the final directory against reparse installation.
    // All ancestors remain held; publication and cleanup never reopen its name.
    for directory in &held {
        ordinary_directory(directory)?;
    }
    write(&mut staged.file)?;
    staged.file.sync_all()?;
    for directory in &held {
        ordinary_directory(directory)?;
    }
    super::windows_file_moves::rename_relative(&staged.file, parent, &path, true)?;
    staged.published = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn creates_missing_unicode_parents_and_publishes_complete_bytes() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("cache-é").join("nested");
        let destination = directory.join("entry.png");
        let temporary = directory.join("entry.tmp");
        write(&destination, &temporary, &mut |file| {
            file.write_all(b"first complete output")
        })
        .unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"first complete output");
        write(&destination, &temporary, &mut |file| {
            assert_eq!(fs::read(&destination).unwrap(), b"first complete output");
            file.write_all(b"second complete output")
        })
        .unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"second complete output");
        assert!(!temporary.exists());
        assert_eq!(fs::read_dir(directory).unwrap().count(), 1);
    }

    #[test]
    fn writer_error_cleans_the_created_file_and_keeps_the_previous_cache() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("entry.png");
        let temporary = root.path().join("entry.tmp");
        fs::write(&destination, b"previous cache").unwrap();
        let error = write(&destination, &temporary, &mut |file| {
            file.write_all(b"partial bytes")?;
            Err(io::Error::other("injected writer error"))
        })
        .unwrap_err();
        assert!(error.to_string().contains("injected writer error"));
        assert_eq!(fs::read(&destination).unwrap(), b"previous cache");
        assert!(!temporary.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn unwinding_cleans_the_created_file_by_handle() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("entry.png");
        let temporary = root.path().join("entry.tmp");
        fs::write(&destination, b"previous cache").unwrap();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = write(&destination, &temporary, &mut |file| {
                file.write_all(b"partial bytes")?;
                panic!("injected writer panic");
            });
        }));
        assert!(panic.is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"previous cache");
        assert!(!temporary.exists());
    }

    #[test]
    fn held_staging_and_parents_block_path_substitution_during_the_write() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("cache");
        let destination = directory.join("entry.png");
        let temporary = directory.join("entry.tmp");
        write(&destination, &temporary, &mut |file| {
            assert!(fs::rename(&temporary, directory.join("displaced.tmp")).is_err());
            assert!(fs::remove_file(&temporary).is_err());
            assert!(fs::write(&temporary, b"substituted bytes").is_err());
            assert!(fs::rename(&directory, root.path().join("displaced_cache")).is_err());
            file.write_all(b"original held output")
        })
        .unwrap();
        assert_eq!(fs::read(destination).unwrap(), b"original held output");
        assert_eq!(fs::read_dir(directory).unwrap().count(), 1);
    }

    #[test]
    fn staging_child_blocks_a_preopened_reparse_writer() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("cache");
        let outside = root.path().join("outside");
        fs::create_dir(&directory).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("witness"), b"outside witness").unwrap();
        let template = root.path().join("template");
        super::super::windows_reparse_tests::junction(&template, &outside);
        let retarget = super::super::windows_reparse_tests::Retarget::new(&directory, &template);
        write(
            &directory.join("entry.png"),
            &directory.join("entry.tmp"),
            &mut |file| {
                assert!(
                    retarget.try_apply().is_err(),
                    "Nonempty cache directory became a junction"
                );
                file.write_all(b"original held output")
            },
        )
        .unwrap();
        drop(retarget);
        fs::remove_dir(template).unwrap();
        assert_eq!(
            fs::read(directory.join("entry.png")).unwrap(),
            b"original held output"
        );
        assert_eq!(
            fs::read(outside.join("witness")).unwrap(),
            b"outside witness"
        );
        assert_eq!(fs::read_dir(outside).unwrap().count(), 1);
    }

    #[test]
    fn replacement_does_not_write_through_an_existing_hard_link() {
        let root = tempfile::tempdir().unwrap();
        let outside = root.path().join("outside");
        let destination = root.path().join("entry.png");
        fs::write(&outside, b"outside witness").unwrap();
        fs::hard_link(&outside, &destination).unwrap();
        write(&destination, &root.path().join("entry.tmp"), &mut |file| {
            file.write_all(b"new cache bytes")
        })
        .unwrap();
        assert_eq!(fs::read(destination).unwrap(), b"new cache bytes");
        assert_eq!(fs::read(outside).unwrap(), b"outside witness");
    }

    #[test]
    fn existing_junction_is_refused_before_the_writer_is_called() {
        let root = tempfile::tempdir().unwrap();
        let outside = root.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("entry.png"), b"outside witness").unwrap();
        let directory = root.path().join("cache");
        super::super::windows_reparse_tests::junction(&directory, &outside);
        let calls = std::cell::Cell::new(0);
        let result = write(
            &directory.join("entry.png"),
            &directory.join("entry.tmp"),
            &mut |_| {
                calls.set(calls.get() + 1);
                Ok(())
            },
        );
        fs::remove_dir(directory).unwrap();
        assert!(result.is_err());
        assert_eq!(calls.get(), 0);
        assert_eq!(
            fs::read(outside.join("entry.png")).unwrap(),
            b"outside witness"
        );
        assert_eq!(fs::read_dir(outside).unwrap().count(), 1);
    }

    #[test]
    fn invalid_staging_paths_are_refused_before_any_creation() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("missing").join("entry.png");
        for temporary in [
            destination.clone(),
            root.path().join("other").join("entry.tmp"),
        ] {
            let result = write(&destination, &temporary, &mut |_| {
                panic!("Invalid staging called the writer")
            });
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidInput);
        }
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }
}
