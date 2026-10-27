use super::*;
use std::time::{Duration, SystemTime};

fn copy_all(source: &Path, destination: &Path, held: Option<&File>) -> io::Result<Vec<u64>> {
    let mut steps = Vec::new();
    copy(source, destination, held, &mut |_| Ok(()), &mut |copied| {
        steps.push(copied);
        Ok(())
    })?;
    Ok(steps)
}

fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|index| (index % 251) as u8).collect()
}

#[test]
fn the_held_descriptor_is_read_even_at_its_end_and_after_a_substitution() {
    use std::io::Read;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    std::fs::write(&source, b"verified").unwrap();
    let mut held = File::open(&source).unwrap();
    held.read_to_end(&mut Vec::new()).unwrap();
    std::fs::remove_file(&source).unwrap();
    std::fs::write(&source, b"substituted").unwrap();
    let destination = root.path().join("copy");
    copy_all(&source, &destination, Some(&held)).unwrap();
    assert_eq!(std::fs::read(&destination).unwrap(), b"verified");
}

#[test]
fn mode_and_both_dates_are_kept() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    std::fs::write(&source, b"data").unwrap();
    std::fs::set_permissions(&source, Permissions::from_mode(0o4750)).unwrap();
    let accessed = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(1_100_000_000);
    File::options()
        .write(true)
        .open(&source)
        .unwrap()
        .set_times(FileTimes::new().set_accessed(accessed).set_modified(modified))
        .unwrap();
    let destination = root.path().join("copy");
    copy_all(&source, &destination, None).unwrap();
    let copied = std::fs::metadata(&destination).unwrap();
    // The copy has the same owner here: setuid stays.
    assert_eq!(copied.mode() & 0o7777, 0o4750);
    assert_eq!(copied.modified().unwrap(), modified);
    assert_eq!(copied.accessed().unwrap(), accessed);
}

#[test]
fn set_id_bits_are_dropped_for_another_owner() {
    let Ok(foreign) = std::fs::metadata("/usr/bin/passwd") else { return };
    let root = tempfile::tempdir().unwrap();
    let own = root.path().join("own");
    std::fs::write(&own, b"").unwrap();
    let own = std::fs::metadata(&own).unwrap();
    if foreign.uid() == own.uid() {
        return;
    }
    assert_eq!(mode(&foreign, &own), foreign.mode() & 0o7777 & !SET_ID);
}

#[test]
fn a_file_larger_than_one_kernel_request_is_copied_across_volumes() {
    let Ok(other) = tempfile::tempdir_in("/dev/shm") else { return };
    let root = tempfile::tempdir().unwrap();
    let bytes = pattern(KERNEL_CHUNK + BUFFER + 17);
    for (source, destination) in [
        (root.path().join("source"), root.path().join("copy")),
        (root.path().join("source"), other.path().join("copy")),
    ] {
        std::fs::write(&source, &bytes).unwrap();
        let steps = copy_all(&source, &destination, None).unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), bytes);
        assert!(steps.windows(2).all(|pair| pair[0] <= pair[1]), "{steps:?}");
        assert_eq!(steps.last(), Some(&(bytes.len() as u64)));
        assert!(steps.len() >= 2, "{steps:?}");
    }
}

#[test]
fn a_progress_refusal_stops_the_copy() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    std::fs::write(&source, pattern(BUFFER * 3)).unwrap();
    let destination = root.path().join("copy");
    let mut calls = 0;
    let error = copy(&source, &destination, None, &mut |_| Ok(()), &mut |_| {
        calls += 1;
        if calls > 1 { Err(io::ErrorKind::Interrupted.into()) } else { Ok(()) }
    })
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
}

#[test]
fn a_folder_is_refused_and_an_existing_output_is_kept() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("copy");
    assert!(copy_all(root.path(), &destination, None).is_err());
    assert!(!destination.exists());
    let source = root.path().join("source");
    std::fs::write(&source, b"new").unwrap();
    std::fs::write(&destination, b"kept").unwrap();
    assert!(copy_all(&source, &destination, None).is_err());
    assert_eq!(std::fs::read(&destination).unwrap(), b"kept");
}
