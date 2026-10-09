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
    write_with_policy(
        path,
        temporary,
        write,
        super::windows_preview_quota::Policy::default(),
    )
}

fn write_with_policy(
    path: &Path,
    temporary: &Path,
    write: &mut dyn FnMut(&mut File) -> io::Result<()>,
    policy: super::windows_preview_quota::Policy,
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
    let destination = path
        .file_name()
        .ok_or_else(|| io::Error::other("Cache has no name"))?;
    let _lease = if super::windows_preview_quota::recognized(destination) {
        Some(super::windows_preview_quota::admit(
            parent,
            destination,
            leaf,
            &staged.file,
            policy,
        )?)
    } else {
        None
    };
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

    #[test]
    fn persistent_cache_count_remains_at_most_4096() {
        let root = tempfile::tempdir().unwrap();
        for index in 0..4096 {
            fs::write(root.path().join(format!("{index:064x}_small.png")), b"x").unwrap();
        }
        let destination = root.path().join(format!("{:064x}_small.png", 4096));
        let temporary = root.path().join("incoming.tmp");
        write(&destination, &temporary, &mut |file| {
            file.write_all(b"incoming bytes")
        })
        .unwrap();
        let entries = fs::read_dir(root.path())
            .unwrap()
            .filter(|entry| {
                entry
                    .as_ref()
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "png")
            })
            .count();
        assert!(entries <= 4096, "Preview disk cache kept {entries} entries");
        assert_eq!(fs::read(destination).unwrap(), b"incoming bytes");
    }

    fn cache_path(root: &Path, index: u32) -> std::path::PathBuf {
        root.join(format!("{index:064x}_small.png"))
    }
    fn quota(bytes: u64, entries: usize) -> super::super::windows_preview_quota::Policy {
        super::super::windows_preview_quota::Policy {
            bytes,
            entries,
            entry_bytes: bytes,
            maintenance_time: std::time::Duration::from_secs(5),
            ..Default::default()
        }
    }
    fn seed(root: &Path, index: u32, bytes: &[u8], age: u64) {
        let path = cache_path(root, index);
        fs::write(&path, bytes).unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(
                fs::FileTimes::new()
                    .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(age)),
            )
            .unwrap();
    }

    #[test]
    fn disk_quota_evicts_oldest_closed_entries_on_count_or_byte_pressure() {
        for policy in [quota(100, 2), quota(8, 10)] {
            let root = tempfile::tempdir().unwrap();
            seed(root.path(), 1, b"1111", 100);
            seed(root.path(), 2, b"2222", 200);
            let destination = cache_path(root.path(), 3);
            write_with_policy(
                &destination,
                &root.path().join("entry.tmp"),
                &mut |file| file.write_all(b"3333"),
                policy,
            )
            .unwrap();
            assert!(!cache_path(root.path(), 1).exists());
            assert_eq!(fs::read(cache_path(root.path(), 2)).unwrap(), b"2222");
            assert_eq!(fs::read(destination).unwrap(), b"3333");
        }
    }

    #[test]
    fn disk_quota_skips_open_oldest_entry_and_evicts_the_next() {
        let root = tempfile::tempdir().unwrap();
        seed(root.path(), 1, b"1111", 100);
        seed(root.path(), 2, b"2222", 200);
        let reader = File::open(cache_path(root.path(), 1)).unwrap();
        write_with_policy(
            &cache_path(root.path(), 3),
            &root.path().join("entry.tmp"),
            &mut |file| file.write_all(b"3333"),
            quota(8, 2),
        )
        .unwrap();
        assert_eq!(fs::read(cache_path(root.path(), 1)).unwrap(), b"1111");
        assert!(!cache_path(root.path(), 2).exists());
        assert_eq!(fs::read(cache_path(root.path(), 3)).unwrap(), b"3333");
        drop(reader);
    }

    #[test]
    fn disk_quota_preserves_unknown_and_readonly_entries_when_full() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("user.png"), b"user witness").unwrap();
        seed(root.path(), 1, b"1111", 100);
        let mut permissions = fs::metadata(cache_path(root.path(), 1))
            .unwrap()
            .permissions();
        permissions.set_readonly(true);
        fs::set_permissions(cache_path(root.path(), 1), permissions.clone()).unwrap();
        let result = write_with_policy(
            &cache_path(root.path(), 2),
            &root.path().join("entry.tmp"),
            &mut |file| file.write_all(b"new"),
            quota(100, 2),
        );
        assert!(result.is_err());
        assert_eq!(
            fs::read(root.path().join("user.png")).unwrap(),
            b"user witness"
        );
        assert_eq!(fs::read(cache_path(root.path(), 1)).unwrap(), b"1111");
        assert!(!cache_path(root.path(), 2).exists());
        assert!(!root.path().join("entry.tmp").exists());
        permissions.set_readonly(false);
        fs::set_permissions(cache_path(root.path(), 1), permissions).unwrap();
    }

    #[test]
    fn disk_quota_replacement_counts_the_destination_only_once() {
        let root = tempfile::tempdir().unwrap();
        seed(root.path(), 1, b"1111", 100);
        seed(root.path(), 2, b"2222", 200);
        write_with_policy(
            &cache_path(root.path(), 1),
            &root.path().join("entry.tmp"),
            &mut |file| file.write_all(b"new1"),
            quota(8, 2),
        )
        .unwrap();
        assert_eq!(fs::read(cache_path(root.path(), 1)).unwrap(), b"new1");
        assert_eq!(fs::read(cache_path(root.path(), 2)).unwrap(), b"2222");
    }

    #[test]
    fn disk_quota_eviction_unlinks_cache_without_altering_an_external_hard_link() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("cache");
        fs::create_dir(&cache).unwrap();
        let outside = root.path().join("outside");
        fs::write(&outside, b"outside witness").unwrap();
        fs::hard_link(&outside, cache_path(&cache, 1)).unwrap();
        write_with_policy(
            &cache_path(&cache, 2),
            &cache.join("entry.tmp"),
            &mut |file| file.write_all(b"new"),
            quota(100, 1),
        )
        .unwrap();
        assert!(!cache_path(&cache, 1).exists());
        assert_eq!(fs::read(outside).unwrap(), b"outside witness");
    }

    #[test]
    fn disk_quota_lock_contention_leaves_existing_cache_and_cleans_staging() {
        let root = tempfile::tempdir().unwrap();
        seed(root.path(), 1, b"1111", 100);
        let lock = root.path().join(".preview-cache-lock");
        fs::write(&lock, b"lease witness").unwrap();
        let reader = File::open(&lock).unwrap();
        let error = write_with_policy(
            &cache_path(root.path(), 2),
            &root.path().join("entry.tmp"),
            &mut |file| file.write_all(b"new"),
            quota(100, 2),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(fs::read(&lock).unwrap(), b"lease witness");
        assert_eq!(fs::read(cache_path(root.path(), 1)).unwrap(), b"1111");
        assert!(!root.path().join("entry.tmp").exists());
        drop(reader);
    }

    #[test]
    fn disk_quota_scan_and_time_limits_abort_without_eviction() {
        for policy in [
            super::super::windows_preview_quota::Policy {
                scan_entries: 1,
                ..quota(100, 2)
            },
            super::super::windows_preview_quota::Policy {
                maintenance_time: std::time::Duration::ZERO,
                ..quota(100, 2)
            },
        ] {
            let root = tempfile::tempdir().unwrap();
            seed(root.path(), 1, b"1111", 100);
            let result = write_with_policy(
                &cache_path(root.path(), 2),
                &root.path().join("entry.tmp"),
                &mut |file| file.write_all(b"new"),
                policy,
            );
            assert!(result.is_err());
            assert_eq!(fs::read(cache_path(root.path(), 1)).unwrap(), b"1111");
            assert!(!cache_path(root.path(), 2).exists());
            assert!(!root.path().join("entry.tmp").exists());
        }
    }

    #[test]
    fn disk_quota_rejects_an_oversized_entry_without_evicting() {
        let root = tempfile::tempdir().unwrap();
        seed(root.path(), 1, b"1111", 100);
        let mut policy = quota(100, 1);
        policy.entry_bytes = 4;
        let result = write_with_policy(
            &cache_path(root.path(), 2),
            &root.path().join("entry.tmp"),
            &mut |file| file.write_all(b"12345"),
            policy,
        );
        assert!(result.is_err());
        assert_eq!(fs::read(cache_path(root.path(), 1)).unwrap(), b"1111");
        assert!(!cache_path(root.path(), 2).exists());
        assert!(!root.path().join("entry.tmp").exists());
    }

    #[test]
    fn disk_quota_prefers_closed_recognized_staging_over_completed_previews() {
        let root = tempfile::tempdir().unwrap();
        seed(root.path(), 1, b"1111", 100);
        let abandoned = root.path().join(format!("{:064x}_small.123.7.tmp", 2));
        fs::write(&abandoned, b"abandoned bytes").unwrap();
        write_with_policy(
            &cache_path(root.path(), 3),
            &root.path().join("entry.tmp"),
            &mut |file| file.write_all(b"new"),
            quota(100, 2),
        )
        .unwrap();
        assert!(!abandoned.exists());
        assert_eq!(fs::read(cache_path(root.path(), 1)).unwrap(), b"1111");
        assert_eq!(fs::read(cache_path(root.path(), 3)).unwrap(), b"new");
    }

    #[test]
    fn disk_quota_preserves_active_recognized_staging() {
        let root = tempfile::tempdir().unwrap();
        seed(root.path(), 1, b"1111", 100);
        let active = root.path().join(format!("{:064x}.123.7.tmp", 2));
        let mut writer = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&active)
            .unwrap();
        writer.write_all(b"active bytes").unwrap();
        write_with_policy(
            &cache_path(root.path(), 3),
            &root.path().join("entry.tmp"),
            &mut |file| file.write_all(b"new"),
            quota(100, 2),
        )
        .unwrap();
        assert_eq!(fs::read(&active).unwrap(), b"active bytes");
        assert!(!cache_path(root.path(), 1).exists());
        assert_eq!(fs::read(cache_path(root.path(), 3)).unwrap(), b"new");
        drop(writer);
    }
}
