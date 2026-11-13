use super::{recover_in, write, Entry};
use crate::platform::platform;
use crate::systems::path_parents::HeldParents;
use std::io::Write;
use std::path::{Path, PathBuf};

/// A staging folder with a partial output and its note, as a copy leaves them.
fn stage(root: &Path, notes: &Path, name: &str) -> (PathBuf, Entry) {
    let parents = HeldParents::new(&root.join("destination")).unwrap();
    let path = root.join(name);
    let (directory, pin) = platform().create_copy_staging(parents.directory(), &path).unwrap();
    let mut output = platform().create_operation_output(&directory, &path.join("output")).unwrap();
    output.write_all(b"partial copy").unwrap();
    let entry = write(notes, &path, &directory, &pin).unwrap();
    (path, entry)
}

/// The process ends: its lock goes, its note stays.
fn end(mut entry: Entry) {
    drop(entry.lock.take());
    std::mem::forget(entry);
}

#[test]
fn an_ended_process_leaves_nothing_after_the_next_start() {
    let root = tempfile::tempdir().unwrap();
    let notes = root.path().join("notes");
    let (path, entry) = stage(root.path(), &notes, ".oxyde-copy-ended");
    end(entry);
    let recovered = recover_in(&notes);
    assert_eq!(recovered.removed, [path.clone()]);
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
    std::fs::remove_dir_all(&replaced).unwrap();
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
