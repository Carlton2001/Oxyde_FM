use super::*;

/// Waits out the cache lock a concurrent test's child may share, as `prepare` does.
fn prepare(base: &Path, policy: Policy, now: u64) -> io::Result<CacheEntry> {
    let started = Instant::now();
    loop {
        match CacheEntry::prepare(base, policy, now) {
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock
                    && error.get_ref().and_then(|inner| inner.downcast_ref::<ArchiveCacheIssue>()).is_none()
                    && started.elapsed() < Duration::from_secs(5) =>
            {
                std::thread::sleep(Duration::from_millis(5))
            }
            result => return result,
        }
    }
}

#[test]
fn preparations_cut_by_a_crash_stop_holding_pending_slots() {
    let base = tempfile::tempdir().unwrap();
    let now = seconds(SystemTime::now()).unwrap();
    let policy = Policy { max_entries: 10, ..Policy::DEFAULT };
    // Unfinished entries from crashes hours ago, which recovery could not prove.
    let crashed = now - 2 * PREPARATION_WINDOW.as_secs();
    let abandoned: Vec<_> = (0..MAX_PENDING_ENTRIES)
        .map(|_| prepare(base.path(), policy, crashed).unwrap())
        .collect();
    assert!(prepare(base.path(), policy, now).is_ok());
    drop(abandoned);
}

#[test]
fn a_foreign_file_in_the_cache_root_is_left_alone() {
    let base = tempfile::tempdir().unwrap();
    let now = seconds(SystemTime::now()).unwrap();
    let policy = Policy { max_entries: 10, ..Policy::DEFAULT };
    let first = prepare(base.path(), policy, now).unwrap();
    let root = first.payload().ancestors().find(|path| path.ends_with("v1") || path.ends_with("v2")).unwrap().to_path_buf();
    drop(first);
    fs::write(root.join("Thumbs.db"), b"x").unwrap();
    assert!(prepare(base.path(), policy, now).is_ok());
    assert_eq!(fs::read(root.join("Thumbs.db")).unwrap(), b"x");
}
