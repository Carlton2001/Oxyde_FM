//! Linux eviction: a write lease stands for the exclusive Windows sharing check.
use super::*;

#[test]
fn pressure_skips_an_entry_still_open_and_evicts_the_next_closed_one() {
    let base = tempfile::tempdir().unwrap();
    let now = seconds(SystemTime::now()).unwrap();
    let policy = Policy { max_entries: 2, ..policy() };
    let oldest = seed(base.path(), now);
    let entry = prepare(base.path(), policy, now + 10).unwrap();
    let second = entry.payload().join("second.txt");
    fs::write(&second, b"closed").unwrap();
    entry.finish(second.clone()).unwrap();
    let reader = File::open(&oldest).unwrap();
    drop(prepare(base.path(), policy, now + 80).unwrap());
    assert!(oldest.exists());
    assert!(!second.exists());
    drop(reader);
}

#[test]
fn an_expired_file_open_in_another_process_is_preserved_until_it_closes() {
    let base = tempfile::tempdir().unwrap();
    let now = seconds(SystemTime::now()).unwrap();
    let path = seed(base.path(), now);
    let mut reader = std::process::Command::new("sleep").arg("30").stdin(File::open(&path).unwrap()).spawn().unwrap();
    assert!(prepare(base.path(), policy(), now + 7200).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"safe cached document");
    reader.kill().unwrap();
    reader.wait().unwrap();
    drop(prepare(base.path(), policy(), now + 7200).unwrap());
    assert!(!path.exists());
    // The whole entry goes: payload, marker and folder; only the lease file stays.
    let names: Vec<_> = fs::read_dir(base.path().join("oxyde-archive-open/v1")).unwrap().map(|item| item.unwrap().file_name()).collect();
    assert_eq!(names, [".cache-lock"]);
}

#[test]
fn cleanup_and_quota_scan_never_follow_a_cache_link() {
    let base = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let witness = outside.path().join("witness");
    fs::write(&witness, b"outside").unwrap();
    let now = seconds(SystemTime::now()).unwrap();
    let path = seed(base.path(), now);
    std::os::unix::fs::symlink(outside.path(), path.parent().unwrap().join("redirect")).unwrap();
    assert!(prepare(base.path(), policy(), now + 7200).is_err());
    assert_eq!(fs::read(&witness).unwrap(), b"outside");
    assert!(path.exists());
}

/// Fills the default quota with closed copies: the next admission evicts the oldest
/// one past its launch grace instead of failing, and protected copies fail cleanly.
#[test]
fn the_default_quota_saturates_at_its_entry_count() {
    let base = tempfile::tempdir().unwrap();
    let now = seconds(SystemTime::now()).unwrap();
    let policy = Policy { max_entries: 8, ..Policy::DEFAULT };
    let mut paths = Vec::new();
    for index in 0..7 {
        let entry = prepare(base.path(), policy, now + index).unwrap();
        let path = entry.payload().join(format!("{index}.txt"));
        fs::write(&path, b"copy").unwrap();
        paths.push(entry.finish(path).unwrap());
    }
    // Within the launch grace, nothing may be evicted: the eighth is the limit.
    let readers: Vec<_> = paths.iter().map(|path| File::open(path).unwrap()).collect();
    assert!(prepare(base.path(), policy, now + 30).is_ok());
    assert!(prepare(base.path(), policy, now + 120).is_ok(), "pending entries do not count past their drop");
    let full = prepare(base.path(), Policy { max_entries: 7, ..policy }, now + 120).err().unwrap();
    assert!(full.to_string().contains("protected"), "{full}");
    drop(readers);
    drop(prepare(base.path(), Policy { max_entries: 7, ..policy }, now + 120).unwrap());
    assert!(!paths[0].exists() && paths[1..].iter().all(|path| path.exists()));
}
