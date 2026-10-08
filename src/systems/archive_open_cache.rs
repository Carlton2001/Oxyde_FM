//! Bounded, owned archive-open entries. Old unversioned cache data is untouched.
use crate::models::error::ArchiveCacheIssue;
use crate::platform::platform;
use crate::utils::archive_security::ExtractionRoot;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const OWNER: &str = ".owner.json";
const MAX_SCAN_NODES: usize = 65_536;
const MAX_SCAN_DEPTH: usize = 80;
const MAX_PENDING_ENTRIES: usize = 4;

#[derive(Clone, Copy)]
struct Policy {
    max_bytes: u64,
    max_entries: usize,
    max_entry_bytes: u64,
    retention: Duration,
    launch_grace: Duration,
}
impl Policy {
    const DEFAULT: Self = Self {
        max_bytes: 1024 * 1024 * 1024,
        max_entries: 512,
        // Bound the selected file plus provenance and its fingerprint record.
        max_entry_bytes: 256 * 1024 * 1024 + MAX_OWNER_BYTES + 64 * 1024,
        retention: Duration::from_secs(7 * 24 * 60 * 60),
        launch_grace: Duration::from_secs(60),
    };
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    version: u32,
    created: u64,
    #[serde(default)]
    original: Option<Original>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Original {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    hash: String,
}

struct Session {
    root: PathBuf,
    _guard: ExtractionRoot,
    _lock: Option<File>,
}

pub(crate) struct CacheEntry {
    // Drop the entry handles before TempDir cleanup, but keep the cache root
    // and inter-process lease until the temporary directory is gone.
    _guard: ExtractionRoot,
    marker: File,
    directory: Option<tempfile::TempDir>,
    _session: Session,
    payload: PathBuf,
    policy: Policy,
    created: u64,
}

impl CacheEntry {
    pub fn new() -> io::Result<Self> {
        let base = std::env::temp_dir();
        let started = Instant::now();
        loop {
            match Self::prepare(&base, Policy::DEFAULT, seconds(SystemTime::now())?) {
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock
                        && started.elapsed() < Duration::from_secs(15) =>
                {
                    // This runs on the existing background archive-open worker.
                    // Another Oxyde process may still be preparing its entry.
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(error) => {
                    log::warn!("Archive cache preparation failed: {error}");
                    let issue = if error.kind() == io::ErrorKind::WouldBlock {
                        ArchiveCacheIssue::Busy
                    } else {
                        error
                            .get_ref()
                            .and_then(|e| e.downcast_ref::<ArchiveCacheIssue>())
                            .copied()
                            .unwrap_or(ArchiveCacheIssue::Unavailable)
                    };
                    return Err(io::Error::other(issue));
                }
                result => return result,
            }
        }
    }

    fn prepare(base: &Path, policy: Policy, now: u64) -> io::Result<Self> {
        let base_guard = ExtractionRoot::new(base)?;
        let destination = base_guard.destination("oxyde-archive-open/v1", true)?;
        let guard = ExtractionRoot::new(&destination.path)?;
        let root = destination.path.clone();
        let lock = platform()
            .lock_cache_file(&root.join(".cache-lock"))
            .map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("Archive cache is busy or unavailable: {error}"),
                )
            })?;
        let session = Session {
            root,
            _guard: guard,
            _lock: Some(lock),
        };
        session.make_room(policy, now, 0, None)?;
        let directory = tempfile::Builder::new()
            .prefix("entry-")
            .tempdir_in(&session.root)?;
        let entry_guard = ExtractionRoot::new(directory.path())?;
        let payload = entry_guard.destination("data", true)?.path;
        let mut marker = platform().create_archive_file(&directory.path().join(OWNER))?;
        serde_json::to_writer(
            &mut marker,
            &Owner {
                version: 2,
                created: now,
                original: None,
            },
        )?;
        marker.flush()?;
        marker.sync_all()?;
        let mut entry = Self {
            _guard: entry_guard,
            marker,
            directory: Some(directory),
            _session: session,
            payload,
            policy,
            created: now,
        };
        // Hold the lease only for admission/publication, not decompression.
        entry._session._lock.take();
        Ok(entry)
    }

    pub fn payload(&self) -> &Path {
        &self.payload
    }

    pub fn finish(mut self, path: PathBuf) -> io::Result<PathBuf> {
        if !path.starts_with(&self.payload) || !path.is_file() {
            return Err(io::Error::other("Invalid cached archive file"));
        }
        // Bound the completed entry independently of decoder declarations.
        let mut scan = Scan::default();
        scan.directory(self.directory.as_ref().unwrap().path())?;
        if scan.bytes > self.policy.max_entry_bytes {
            return Err(io::Error::other(
                "Archive cache entry exceeded its file size limit",
            ));
        }
        self._session.acquire()?;
        let metadata = fs::metadata(&path)?;
        let original = Original {
            path: path
                .strip_prefix(self.directory.as_ref().unwrap().path())
                .map_err(io::Error::other)?
                .into(),
            size: metadata.len(),
            modified: metadata.modified().ok(),
            hash: hash_file(&mut File::open(&path)?, metadata.len())?,
        };
        self.marker.set_len(0)?;
        use std::io::{Seek, SeekFrom};
        self.marker.seek(SeekFrom::Start(0))?;
        serde_json::to_writer(
            &mut self.marker,
            &Owner {
                version: 2,
                created: self.created,
                original: Some(original),
            },
        )?;
        self.marker.sync_all()?;
        // Admission uses actual bytes, not a 256 MiB reservation for every small file.
        scan = Scan::default();
        let directory = self.directory.as_ref().unwrap().path();
        scan.directory(directory)?;
        self._session.make_room(
            self.policy,
            seconds(SystemTime::now())?,
            scan.bytes,
            Some(directory),
        )?;
        let _ = self.directory.take().unwrap().keep();
        Ok(path)
    }
}

fn seconds(time: SystemTime) -> io::Result<u64> {
    time.duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .map_err(io::Error::other)
}

impl Session {
    fn acquire(&mut self) -> io::Result<()> {
        let started = Instant::now();
        loop {
            match platform().lock_cache_file(&self.root.join(".cache-lock")) {
                Ok(file) => {
                    self._lock = Some(file);
                    return Ok(());
                }
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock
                        && started.elapsed() < Duration::from_secs(15) =>
                {
                    std::thread::sleep(Duration::from_millis(25))
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn make_room(
        &self,
        policy: Policy,
        now: u64,
        incoming: u64,
        excluded: Option<&Path>,
    ) -> io::Result<()> {
        struct Candidate {
            path: PathBuf,
            bytes: u64,
            created: u64,
        }
        let mut candidates = Vec::new();
        let mut visited = 0usize;
        let mut remaining = MAX_SCAN_NODES;
        let mut scan = Scan::default();
        let mut pending = 0usize;
        for item in fs::read_dir(&self.root)? {
            let item = item?;
            if item.file_name() == ".cache-lock" || excluded == Some(item.path().as_path()) {
                continue;
            }
            visited += 1;
            if visited > MAX_SCAN_NODES {
                return Err(io::Error::other("Cache root scan limit exceeded"));
            }
            let path = item.path();
            if !item.file_name().to_string_lossy().starts_with("entry-") {
                return Err(io::Error::other(
                    "Archive cache contains an unrecognized entry",
                ));
            }
            let metadata = match fs::symlink_metadata(&path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                result => result?,
            };
            if !metadata.is_dir() || platform().file_attributes(&metadata, "").2 {
                return Err(io::Error::other(
                    "Archive cache contains a non-ordinary directory",
                ));
            }
            if self
                .evict_expired(&path, policy, now, false, &mut remaining)
                .unwrap_or(false)
            {
                continue;
            }
            let owner = read_owner(&path).ok();
            if owner
                .as_ref()
                .is_some_and(|owner| owner.version == 2 && owner.original.is_none())
            {
                // A live preparation keeps its marker and directory open.
                // Traversing it would race decoder writes and TempDir cleanup.
                // These at most four entries have their own extraction limits;
                // the persistent byte quota is checked when they finish.
                pending += 1;
                continue;
            }
            let before = scan.bytes;
            if let Err(error) = scan.directory(&path) {
                if error.kind() == io::ErrorKind::NotFound {
                    scan.bytes = before;
                    continue;
                }
                return Err(error);
            }
            let created = owner.map(|owner| owner.created).unwrap_or(0);
            candidates.push(Candidate {
                path,
                bytes: scan.bytes - before,
                created,
            });
        }
        if excluded.is_none() && pending >= MAX_PENDING_ENTRIES {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                ArchiveCacheIssue::Busy,
            ));
        }
        let mut bytes = scan.bytes;
        let mut entries = candidates.len() + pending;
        let fits = |entries: usize, bytes: u64| {
            entries < policy.max_entries
                && bytes
                    .checked_add(incoming)
                    .is_some_and(|total| total <= policy.max_bytes)
        };
        if fits(entries, bytes) {
            return Ok(());
        }
        // Pressure overrides the seven-day expiry, oldest completed copy first.
        // A short grace leaves time for the shell to hand the file to its reader.
        candidates.sort_by_key(|candidate| candidate.created);
        for candidate in candidates {
            match self.evict_expired(&candidate.path, policy, now, true, &mut remaining) {
                Ok(true) => {
                    entries -= 1;
                    bytes = bytes.saturating_sub(candidate.bytes);
                }
                Ok(false) => {}
                Err(error) => log::debug!(
                    "Protected cache entry retained {}: {error}",
                    candidate.path.display()
                ),
            }
            if fits(entries, bytes) {
                return Ok(());
            }
        }
        Err(io::Error::other(ArchiveCacheIssue::Full))
    }

    fn evict_expired(
        &self,
        path: &Path,
        policy: Policy,
        now: u64,
        pressure: bool,
        remaining: &mut usize,
    ) -> io::Result<bool> {
        // The fixed root is already held. Never accept an outside cleanup path.
        if !path.is_absolute() || path.parent() != Some(self.root.as_path()) {
            return Err(io::Error::other("Cache cleanup path escaped its root"));
        }
        let root_handle = platform().open_cache_cleanup(path, true)?;
        let marker_path = path.join(OWNER);
        let mut marker = platform().open_cache_cleanup(&marker_path, false)?;
        let mut bytes = Vec::new();
        (&mut marker)
            .take(MAX_OWNER_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_OWNER_BYTES {
            return Err(io::Error::other("Cache owner record is too large"));
        }
        let owner: Owner = serde_json::from_slice(&bytes)?;
        let abandoned = owner.version == 2 && owner.original.is_none();
        let required_age = if pressure || abandoned {
            policy.launch_grace
        } else {
            policy.retention
        };
        if !matches!(owner.version, 1 | 2)
            || now.saturating_sub(owner.created) < required_age.as_secs()
            || (pressure && owner.version == 1 && owner.original.is_none())
        {
            return Ok(false);
        }
        let mut nodes = Vec::new();
        collect_cleanup(path, &marker_path, &mut nodes, remaining, 0)?;
        if let Some(original) = owner.original {
            let expected = crate::utils::archive_security::relative_archive_path(
                &original.path.to_string_lossy(),
            )?;
            if expected != original.path || !expected.starts_with("data") {
                return Ok(false);
            }
            let mut files = nodes.iter_mut().filter(|node| !node.directory);
            let Some(node) = files.next() else {
                return Ok(false);
            };
            let metadata = node.file.metadata()?;
            if node.path != path.join(expected)
                || files.next().is_some()
                || metadata.len() != original.size
                || metadata.modified().ok() != original.modified
                || hash_file(&mut node.file, original.size)? != original.hash
            {
                return Ok(false);
            }
        } else if !abandoned {
            // Version-one entries have no content baseline: only expire after
            // the old retention window, with recent modifications protected.
            for node in &nodes {
                if !node.directory
                    && now.saturating_sub(seconds(node.file.metadata()?.modified()?)?)
                        < policy.retention.as_secs()
                {
                    return Ok(false);
                }
            }
        }
        // All data handles were obtained exclusively before any deletion.
        // Existing readers prevent the whole eviction, including readers that
        // allow FILE_SHARE_DELETE. Directories are deleted by held identity.
        for node in nodes {
            platform().remove_open_file(&node.file, &node.path)?;
            drop(node.file);
        }
        platform().remove_open_file(&marker, &marker_path)?;
        drop(marker);
        platform().remove_open_file(&root_handle, path)?;
        drop(root_handle);
        Ok(true)
    }
}

const MAX_OWNER_BYTES: u64 = 512 * 1024;
fn read_owner(path: &Path) -> io::Result<Owner> {
    let mut file = platform().read_cache_file(&path.join(OWNER))?;
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_OWNER_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_OWNER_BYTES {
        return Err(io::Error::other("Cache owner record is too large"));
    }
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}
fn hash_file(file: &mut File, size: u64) -> io::Result<String> {
    if size > Policy::DEFAULT.max_entry_bytes {
        return Err(io::Error::other("Cache fingerprint size limit exceeded"));
    }
    let mut hash = blake3::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| io::Error::other("Cache fingerprint size overflow"))?;
        if total > size {
            return Err(io::Error::other("Cache file changed during fingerprinting"));
        }
        hash.update(&buffer[..count]);
    }
    if total != size {
        return Err(io::Error::other("Cache file changed during fingerprinting"));
    }
    Ok(hash.finalize().to_hex().to_string())
}

struct Node {
    path: PathBuf,
    file: File,
    directory: bool,
}
fn collect_cleanup(
    path: &Path,
    marker: &Path,
    nodes: &mut Vec<Node>,
    remaining: &mut usize,
    depth: usize,
) -> io::Result<()> {
    if depth > MAX_SCAN_DEPTH {
        return Err(io::Error::other("Cache cleanup depth limit exceeded"));
    }
    for item in fs::read_dir(path)? {
        *remaining = remaining
            .checked_sub(1)
            .ok_or_else(|| io::Error::other("Cache cleanup scan limit exceeded"))?;
        let path = item?.path();
        if path == marker {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)?;
        let directory = metadata.is_dir();
        let file = platform().open_cache_cleanup(&path, directory)?;
        if directory {
            collect_cleanup(&path, marker, nodes, remaining, depth + 1)?;
        }
        nodes.push(Node {
            path,
            file,
            directory,
        });
    }
    Ok(())
}

#[derive(Default)]
struct Scan {
    bytes: u64,
    nodes: usize,
}
impl Scan {
    fn directory(&mut self, path: &Path) -> io::Result<()> {
        self.directory_at(path, 0)
    }
    fn directory_at(&mut self, path: &Path, depth: usize) -> io::Result<()> {
        if depth > MAX_SCAN_DEPTH {
            return Err(io::Error::other("Cache scan depth limit exceeded"));
        }
        let _guard = platform().hold_archive_directory(path)?;
        for item in fs::read_dir(path)? {
            self.nodes = self
                .nodes
                .checked_add(1)
                .ok_or_else(|| io::Error::other("Cache scan count overflow"))?;
            if self.nodes > MAX_SCAN_NODES {
                return Err(io::Error::other("Cache scan limit exceeded"));
            }
            let path = item?.path();
            let metadata = fs::symlink_metadata(&path)?;
            if platform().file_attributes(&metadata, "").2 || metadata.file_type().is_symlink() {
                return Err(io::Error::other(
                    "Cache scan refuses links and reparse points",
                ));
            }
            if metadata.is_dir() {
                self.directory_at(&path, depth + 1)?;
            } else if metadata.is_file() {
                self.bytes = self
                    .bytes
                    .checked_add(
                        metadata
                            .len()
                            .checked_add(16 * 1024)
                            .ok_or_else(|| io::Error::other("Cache stream size overflow"))?,
                    )
                    .ok_or_else(|| io::Error::other("Cache size overflow"))?;
            } else {
                return Err(io::Error::other("Cache contains a special file"));
            }
        }
        Ok(())
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    fn policy() -> Policy {
        Policy {
            max_bytes: 1024 * 1024,
            max_entries: 1,
            max_entry_bytes: 64 * 1024,
            retention: Duration::from_secs(3600),
            launch_grace: Duration::from_secs(60),
        }
    }
    fn seed(base: &Path, now: u64) -> PathBuf {
        let entry = CacheEntry::prepare(base, policy(), now).unwrap();
        let path = entry.payload().join("document.txt");
        fs::write(&path, b"safe cached document").unwrap();
        entry.finish(path).unwrap()
    }
    #[test]
    fn excessively_deep_cache_is_refused_without_deleting_data() {
        let base = tempfile::tempdir().unwrap();
        let now = seconds(SystemTime::now()).unwrap();
        let cached = seed(base.path(), now);
        let mut path = fs::canonicalize(cached.parent().unwrap()).unwrap();
        for _ in 0..=MAX_SCAN_DEPTH {
            path = path.join("d");
            fs::create_dir(&path).unwrap();
        }
        fs::write(path.join("witness"), b"preserved").unwrap();
        assert!(CacheEntry::prepare(base.path(), policy(), now + 7200).is_err());
        assert_eq!(fs::read(path.join("witness")).unwrap(), b"preserved");
        assert!(cached.exists());
    }

    #[test]
    fn small_files_use_actual_bytes_instead_of_maximum_file_reservations() {
        let base = tempfile::tempdir().unwrap();
        let now = seconds(SystemTime::now()).unwrap();
        let first = seed(base.path(), now);
        let constrained = Policy {
            max_bytes: 70 * 1024,
            max_entries: 10,
            ..policy()
        };
        let entry = CacheEntry::prepare(base.path(), constrained, now + 10).unwrap();
        let second = entry.payload().join("small.txt");
        fs::write(&second, b"tiny").unwrap();
        entry.finish(second.clone()).unwrap();
        assert!(first.exists());
        assert!(second.exists());
    }

    #[test]
    fn pressure_evicts_unmodified_closed_entries_before_expiry() {
        let base = tempfile::tempdir().unwrap();
        let now = seconds(SystemTime::now()).unwrap();
        let old = seed(base.path(), now);
        let entry = CacheEntry::prepare(base.path(), policy(), now + 61).unwrap();
        assert!(!old.exists());
        drop(entry);
    }

    #[test]
    fn pressure_preserves_modified_content_even_with_original_size_and_timestamp() {
        let base = tempfile::tempdir().unwrap();
        let now = seconds(SystemTime::now()).unwrap();
        let path = seed(base.path(), now);
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        fs::write(&path, b"user edited document").unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        assert!(CacheEntry::prepare(base.path(), policy(), now + 61).is_err());
        assert_eq!(fs::read(path).unwrap(), b"user edited document");
    }

    #[test]
    fn pressure_skips_open_oldest_entry_and_evicts_the_next_closed_one() {
        use std::os::windows::fs::OpenOptionsExt;
        let base = tempfile::tempdir().unwrap();
        let now = seconds(SystemTime::now()).unwrap();
        let oldest = seed(base.path(), now);
        let entry = CacheEntry::prepare(
            base.path(),
            Policy {
                max_entries: 2,
                ..policy()
            },
            now + 10,
        )
        .unwrap();
        let second = entry.payload().join("second.txt");
        fs::write(&second, b"closed").unwrap();
        entry.finish(second.clone()).unwrap();
        let reader = File::options()
            .read(true)
            .share_mode(7)
            .open(&oldest)
            .unwrap();
        let entry = CacheEntry::prepare(
            base.path(),
            Policy {
                max_entries: 2,
                ..policy()
            },
            now + 80,
        )
        .unwrap();
        assert!(oldest.exists());
        assert!(!second.exists());
        drop(reader);
        drop(entry);
    }

    #[test]
    fn cleanup_and_quota_scan_never_follow_a_cache_junction() {
        use std::os::windows::process::CommandExt;
        let base = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let witness = outside.path().join("witness");
        fs::write(&witness, b"outside").unwrap();
        let now = seconds(SystemTime::now()).unwrap();
        let path = seed(base.path(), now);
        let link = path.parent().unwrap().join("redirect");
        let output = std::process::Command::new(platform().system_executable(crate::platform::SystemExecutable::PowerShell).unwrap())
            .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:OXYDE_TEST_LINK -Value $env:OXYDE_TEST_TARGET -ErrorAction Stop | Out-Null"])
            .env("OXYDE_TEST_LINK", &link).env("OXYDE_TEST_TARGET", outside.path())
            .creation_flags(0x08000000).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(CacheEntry::prepare(base.path(), policy(), now + 7200).is_err());
        assert_eq!(fs::read(&witness).unwrap(), b"outside");
        assert!(path.exists());
        fs::remove_dir(link).unwrap();
    }

    #[test]
    fn recent_entries_are_preserved_and_capacity_is_enforced() {
        let base = tempfile::tempdir().unwrap();
        let now = seconds(SystemTime::now()).unwrap();
        let path = seed(base.path(), now);
        assert!(CacheEntry::prepare(base.path(), policy(), now + 10).is_err());
        assert_eq!(fs::read(path).unwrap(), b"safe cached document");
    }
    #[test]
    fn expired_open_file_is_preserved_even_when_its_reader_allows_deletion() {
        use std::os::windows::fs::OpenOptionsExt;
        let base = tempfile::tempdir().unwrap();
        let now = seconds(SystemTime::now()).unwrap();
        let path = seed(base.path(), now);
        let reader = File::options()
            .read(true)
            .share_mode(7)
            .open(&path)
            .unwrap();
        assert!(CacheEntry::prepare(base.path(), policy(), now + 7200).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"safe cached document");
        drop(reader);
        let entry = CacheEntry::prepare(base.path(), policy(), now + 7200).unwrap();
        assert!(!path.exists());
        drop(entry);
    }
    #[test]
    fn expired_entry_with_recent_edits_is_preserved() {
        let base = tempfile::tempdir().unwrap();
        let now = seconds(SystemTime::now()).unwrap();
        let path = seed(base.path(), now);
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(now + 7190))
            .unwrap();
        assert!(CacheEntry::prepare(base.path(), policy(), now + 7200).is_err());
        assert!(path.exists());
    }
    #[test]
    fn preparations_can_overlap_but_pending_entries_are_bounded_and_cleaned() {
        let base = tempfile::tempdir().unwrap();
        let now = seconds(SystemTime::now()).unwrap();
        let policy = Policy {
            max_entries: 10,
            ..policy()
        };
        let first = CacheEntry::prepare(base.path(), policy, now).unwrap();
        let payload = first.payload().to_path_buf();
        let mut others = Vec::new();
        for _ in 1..MAX_PENDING_ENTRIES {
            others.push(CacheEntry::prepare(base.path(), policy, now).unwrap());
        }
        let error = CacheEntry::prepare(base.path(), policy, now).err().unwrap();
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        drop(first);
        assert!(!payload.exists());
        assert!(CacheEntry::prepare(base.path(), policy, now).is_ok());
        drop(others);
    }

    #[test]
    fn active_workspaces_are_not_scanned_and_closed_abandoned_entries_are_reclaimed() {
        let base = tempfile::tempdir().unwrap();
        let now = seconds(SystemTime::now()).unwrap();
        let policy = Policy {
            max_entries: 10,
            ..policy()
        };
        let first = CacheEntry::prepare(base.path(), policy, now).unwrap();
        let mut deep = fs::canonicalize(first.payload()).unwrap();
        for _ in 0..=MAX_SCAN_DEPTH {
            deep = deep.join("d");
            fs::create_dir(&deep).unwrap();
        }
        let second = CacheEntry::prepare(base.path(), policy, now + 61).unwrap();
        assert!(deep.exists());
        drop(first);
        let abandoned = second.directory.as_ref().unwrap().path().to_path_buf();
        let directory = second.directory.unwrap().keep();
        drop(second.marker);
        drop(second._guard);
        drop(second._session);
        assert_eq!(directory, abandoned);
        let third = CacheEntry::prepare(base.path(), policy, now + 122).unwrap();
        assert!(!abandoned.exists());
        drop(third);
    }
}
