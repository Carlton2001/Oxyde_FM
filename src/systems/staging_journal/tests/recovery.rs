use super::{recover_in, write_file, write_folder, Entry};
use crate::platform::platform;
use crate::systems::path_parents::HeldParents;
use std::io::Write;
use std::path::{Path, PathBuf};

/// A partial file and its note, as a copy leaves them.
fn partial(root: &Path, notes: &Path, name: &str) -> (PathBuf, Entry) {
    let path = root.join(name);
    let parents = HeldParents::new(&path).unwrap();
    let mut output = platform().create_operation_output(parents.directory(), &path).unwrap();
    output.write_all(b"partial copy").unwrap();
    let identity = platform().file_identity(&output).unwrap();
    let created = output.metadata().unwrap().created().ok();
    let entry = write_file(notes, &path, identity, created).unwrap();
    (path, entry)
}

/// A staging folder with a partial output and its note, as earlier versions left them.
fn stage(root: &Path, notes: &Path, name: &str) -> (PathBuf, Entry) {
    let parents = HeldParents::new(&root.join("destination")).unwrap();
    let path = root.join(name);
    let (directory, pin) = platform().create_copy_staging(parents.directory(), &path).unwrap();
    let mut output = platform().create_operation_output(&directory, &path.join("output")).unwrap();
    output.write_all(b"partial copy").unwrap();
    let entry = write_folder(notes, &path, &directory, &pin).unwrap();
    (path, entry)
}

/// The process ends: its lock goes, its note stays. A process another test forks meanwhile
/// shares the lock until it executes, so the note counts as ended once it can be locked.
fn end(mut entry: Entry) {
    drop(entry.lock.take());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while platform().lock_cache_file(&entry.path).is_err() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    std::mem::forget(entry);
}

#[test]
fn an_ended_process_leaves_nothing_after_the_next_start() {
    let root = tempfile::tempdir().unwrap();
    let notes = root.path().join("notes");
    let (path, entry) = stage(root.path(), &notes, ".oxyde-copy-ended");
    end(entry);
    let recovered = recover_in(&notes);
    assert_eq!(recovered.removed, std::slice::from_ref(&path));
    assert!(recovered.kept.is_empty());
    assert!(!path.exists());
    assert_eq!(std::fs::read_dir(&notes).unwrap().count(), 0);
}

#[test]
fn a_running_operation_keeps_its_staging() {
    let root = tempfile::tempdir().unwrap();
    let notes = root.path().join("notes");
    let (path, entry) = stage(root.path(), &notes, ".oxyde-copy-running");
    let recovered = recover_in(&notes);
    assert!(recovered.removed.is_empty() && recovered.kept.is_empty());
    assert!(path.join("output").exists());
    drop(entry);
    assert_eq!(std::fs::read_dir(&notes).unwrap().count(), 0);
}

#[test]
fn unknown_content_or_a_replaced_folder_is_left_in_place() {
    let root = tempfile::tempdir().unwrap();
    let notes = root.path().join("notes");
    let (foreign, entry) = stage(root.path(), &notes, ".oxyde-copy-foreign");
    std::fs::write(foreign.join("witness"), b"unowned").unwrap();
    end(entry);
    let (replaced, entry) = stage(root.path(), &notes, ".oxyde-copy-replaced");
    end(entry);
    // Moved away, not removed: a filesystem may reuse freed inodes for the new folder and pin.
    std::fs::rename(&replaced, root.path().join("original")).unwrap();
    std::fs::create_dir(&replaced).unwrap();
    std::fs::write(replaced.join(".pin"), b"").unwrap();
    std::fs::write(replaced.join("output"), b"someone else's").unwrap();
    let mut recovered = recover_in(&notes);
    recovered.kept.sort();
    assert!(recovered.removed.is_empty());
    assert_eq!(recovered.kept, [foreign.clone(), replaced.clone()]);
    assert_eq!(std::fs::read(foreign.join("witness")).unwrap(), b"unowned");
    assert_eq!(std::fs::read(replaced.join("output")).unwrap(), b"someone else's");
    // Reported once: the notes are gone.
    assert_eq!(std::fs::read_dir(&notes).unwrap().count(), 0);
}

#[test]
fn a_staging_on_an_unplugged_volume_is_cleaned_once_it_is_back() {
    let root = tempfile::tempdir().unwrap();
    let notes = root.path().join("notes");
    let volume = root.path().join("volume");
    std::fs::create_dir(&volume).unwrap();
    let (path, entry) = stage(&volume, &notes, ".oxyde-copy-unplugged");
    end(entry);
    let away = root.path().join("away");
    std::fs::rename(&volume, &away).unwrap();
    let recovered = recover_in(&notes);
    assert!(recovered.removed.is_empty() && recovered.kept.is_empty());
    assert_eq!(std::fs::read_dir(&notes).unwrap().count(), 1);
    std::fs::rename(&away, &volume).unwrap();
    let recovered = recover_in(&notes);
    assert_eq!(recovered.removed, std::slice::from_ref(&path));
    assert!(!path.exists());
    assert_eq!(std::fs::read_dir(&notes).unwrap().count(), 0);
}

#[test]
fn a_partial_file_of_an_ended_process_is_removed() {
    let root = tempfile::tempdir().unwrap();
    let notes = root.path().join("notes");
    let (path, entry) = partial(root.path(), &notes, "copy.bin");
    end(entry);
    std::fs::write(root.path().join("witness"), b"unowned").unwrap();
    let recovered = recover_in(&notes);
    assert_eq!(recovered.removed, std::slice::from_ref(&path));
    assert!(recovered.kept.is_empty());
    assert!(!path.exists());
    assert_eq!(std::fs::read(root.path().join("witness")).unwrap(), b"unowned");
    assert_eq!(std::fs::read_dir(&notes).unwrap().count(), 0);
}

/// A native copy applies the source's creation date once its bytes are written.
#[cfg(target_os = "windows")]
fn apply_source_date(path: &Path) -> Option<std::time::SystemTime> {
    use std::os::windows::fs::FileTimesExt;
    let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    let source = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
    file.set_times(std::fs::FileTimes::new().set_created(source)).unwrap();
    file.metadata().unwrap().created().ok()
}

#[cfg(target_os = "windows")]
#[test]
fn a_finished_copy_with_its_source_date_noted_is_removed() {
    let root = tempfile::tempdir().unwrap();
    let notes = root.path().join("notes");
    let (path, mut entry) = partial(root.path(), &notes, "copy.bin");
    let created = apply_source_date(&path);
    entry.update_created(created);
    end(entry);
    let recovered = recover_in(&notes);
    assert_eq!(recovered.removed, std::slice::from_ref(&path));
    assert!(!path.exists());
}

#[cfg(target_os = "windows")]
#[test]
fn a_finished_copy_whose_date_was_not_noted_is_kept() {
    let root = tempfile::tempdir().unwrap();
    let notes = root.path().join("notes");
    let (path, entry) = partial(root.path(), &notes, "copy.bin");
    apply_source_date(&path);
    end(entry);
    let recovered = recover_in(&notes);
    assert_eq!(recovered.kept, std::slice::from_ref(&path));
    assert!(path.exists());
}

#[test]
fn a_running_copy_keeps_its_partial_file() {
    let root = tempfile::tempdir().unwrap();
    let notes = root.path().join("notes");
    let (path, entry) = partial(root.path(), &notes, "copy.bin");
    let recovered = recover_in(&notes);
    assert!(recovered.removed.is_empty() && recovered.kept.is_empty());
    assert!(path.exists());
    drop(entry);
    assert_eq!(std::fs::read_dir(&notes).unwrap().count(), 0);
}

#[test]
fn a_file_that_took_the_partial_files_name_is_kept() {
    let root = tempfile::tempdir().unwrap();
    let notes = root.path().join("notes");
    let (path, entry) = partial(root.path(), &notes, "copy.bin");
    end(entry);
    // Moved away, not removed: a filesystem may reuse a freed identifier.
    std::fs::rename(&path, root.path().join("original")).unwrap();
    std::fs::write(&path, b"the user's file").unwrap();
    let recovered = recover_in(&notes);
    assert!(recovered.removed.is_empty());
    assert_eq!(recovered.kept, std::slice::from_ref(&path));
    assert_eq!(std::fs::read(&path).unwrap(), b"the user's file");
    assert_eq!(std::fs::read_dir(&notes).unwrap().count(), 0);
}

#[test]
fn a_partial_file_without_a_creation_date_is_kept() {
    let root = tempfile::tempdir().unwrap();
    let notes = root.path().join("notes");
    let path = root.path().join("copy.bin");
    std::fs::write(&path, b"partial copy").unwrap();
    let identity = platform().file_identity(&std::fs::File::open(&path).unwrap()).unwrap();
    end(write_file(&notes, &path, identity, None).unwrap());
    let recovered = recover_in(&notes);
    assert_eq!(recovered.kept, std::slice::from_ref(&path));
    assert!(path.exists());
}

#[test]
fn a_partial_file_whose_folder_went_away_waits_for_it() {
    let root = tempfile::tempdir().unwrap();
    let notes = root.path().join("notes");
    let volume = root.path().join("volume");
    std::fs::create_dir(&volume).unwrap();
    let (path, entry) = partial(&volume, &notes, "copy.bin");
    end(entry);
    let away = root.path().join("away");
    std::fs::rename(&volume, &away).unwrap();
    let recovered = recover_in(&notes);
    assert!(recovered.removed.is_empty() && recovered.kept.is_empty());
    assert_eq!(std::fs::read_dir(&notes).unwrap().count(), 1);
    std::fs::rename(&away, &volume).unwrap();
    let recovered = recover_in(&notes);
    assert_eq!(recovered.removed, std::slice::from_ref(&path));
    assert!(!path.exists());
}
