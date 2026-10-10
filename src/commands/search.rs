use crate::models::FileEntry;

use crate::utils::archive::{is_archive, ArchiveFormat};
use crate::utils::hardware::get_physical_disk_id;
use dashmap::DashMap;
use glob::Pattern;
use once_cell::sync::Lazy;
use regex::{Regex, RegexBuilder};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::SystemTime;

#[derive(Clone)]
enum SearchPattern {
    Glob(Pattern, bool, bool),   // (pattern, ignore_accents, case_sensitive)
    Regex(Regex, bool),          // (regex, ignore_accents)
    Literal(String, bool, bool), // (query, case_sensitive, ignore_accents)
}

static DISK_IO_LOCKS: Lazy<DashMap<u64, Arc<Mutex<()>>>> = Lazy::new(DashMap::new);

/// Native quick search, independent of the legacy two-panel session.
pub enum QuickSearchEvent {
    Batch(Vec<FileEntry>),
    ArchiveSkipped { path: String, detail: String },
    Finished { limited: bool },
    Failed(String),
}

impl QuickSearchEvent {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Finished { .. } | Self::Failed(_))
    }
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AdvancedSearchOptions {
    pub query: String,
    pub regex: bool,
    pub content: String,
    pub content_regex: bool,
    pub recursive: bool,
    pub archives: bool,
    pub case_sensitive: bool,
    pub ignore_accents: bool,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub min_date: Option<u64>,
    pub max_date: Option<u64>,
}

impl AdvancedSearchOptions {
    pub fn validate(&self) -> Result<(), String> {
        self.pattern(&self.query, self.regex)?;
        if !self.regex && self.query.contains(['*', '?']) {
            Pattern::new(&self.query).map_err(|e| e.to_string())?;
        }
        self.pattern(&self.content, self.content_regex)?;
        if self.min_size.zip(self.max_size).is_some_and(|(a, b)| a > b)
            || self.min_date.zip(self.max_date).is_some_and(|(a, b)| a > b)
        {
            return Err("Minimum > maximum".into());
        }
        Ok(())
    }

    fn pattern(&self, query: &str, is_regex: bool) -> Result<SearchPattern, String> {
        let query = if self.ignore_accents {
            crate::utils::remove_accents(query)
        } else {
            query.into()
        };
        if is_regex {
            RegexBuilder::new(&query)
                .case_insensitive(!self.case_sensitive)
                .build()
                .map(|r| SearchPattern::Regex(r, self.ignore_accents))
                .map_err(|e| e.to_string())
        } else {
            Ok(SearchPattern::Literal(
                query,
                self.case_sensitive,
                self.ignore_accents,
            ))
        }
    }
}

/// Uses the native bounded result stream and the retained archive readers.
pub fn advanced_search(
    root: String,
    options: AdvancedSearchOptions,
    limit: usize,
    show_hidden: bool,
    show_system: bool,
    cancellation: Arc<AtomicBool>,
) -> std::sync::mpsc::Receiver<QuickSearchEvent> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(8);
    thread::spawn(move || {
        let run = || -> Result<(), String> {
            options.validate()?;
            let pattern = if !options.regex && options.query.contains(['*', '?']) {
                let mut query = options.query.clone();
                if options.ignore_accents {
                    query = crate::utils::remove_accents(&query);
                }
                if !options.case_sensitive {
                    query = query.to_lowercase();
                }
                SearchPattern::Glob(
                    Pattern::new(&query).map_err(|e| e.to_string())?,
                    options.ignore_accents,
                    options.case_sensitive,
                )
            } else {
                options.pattern(&options.query, options.regex)?
            };
            let content = if options.content.trim().is_empty() {
                None
            } else {
                let query = if options.ignore_accents {
                    crate::utils::remove_accents(&options.content)
                } else {
                    options.content.clone()
                };
                Some(
                    RegexBuilder::new(&if options.content_regex {
                        query
                    } else {
                        regex::escape(&query)
                    })
                    .case_insensitive(!options.case_sensitive)
                    .build()
                    .map_err(|e| e.to_string())?,
                )
            };
            let params = SearchParams {
                pattern,
                min_size: options.min_size,
                max_size: options.max_size,
                min_date: options.min_date,
                max_date: options.max_date,
            };
            let mut batch = Vec::new();
            let mut count = 0;
            let mut flush = std::time::Instant::now();
            let walker = walkdir::WalkDir::new(&root)
                .follow_links(false)
                .max_depth(if options.recursive { usize::MAX } else { 1 })
                .into_iter()
                .filter_entry(|e| {
                    if e.depth() == 0 {
                        return true;
                    }
                    let name = e.file_name().to_string_lossy();
                    e.metadata().is_ok_and(|m| {
                        let (hidden, system, _) = crate::utils::get_file_attributes(&m, &name);
                        (show_hidden || !hidden) && (show_system || !system)
                    })
                });
            for entry in walker {
                if cancellation.load(Ordering::Relaxed) {
                    return Ok(());
                }
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) if e.depth() == 0 => return Err(e.to_string()),
                    Err(_) => continue,
                };
                if entry.depth() == 0 {
                    continue;
                }
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().into_owned();
                if let Ok(m) = entry.metadata() {
                    let modified = m
                        .modified()
                        .unwrap_or(SystemTime::UNIX_EPOCH)
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    let matches = params.pattern.matches(&name)
                        && (!m.is_dir()
                            || (options.min_size.is_none()
                                && options.max_size.is_none()
                                && content.is_none()))
                        && options.min_size.is_none_or(|v| m.len() >= v)
                        && options.max_size.is_none_or(|v| m.len() <= v)
                        && options.min_date.is_none_or(|v| modified >= v)
                        && options.max_date.is_none_or(|v| modified <= v)
                        && content.as_ref().is_none_or(|r| {
                            file_contains_content(path, r, options.ignore_accents, &cancellation)
                        });
                    if cancellation.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    if matches {
                        let (hidden, system, _) = crate::utils::get_file_attributes(&m, &name);
                        batch.push(FileEntry {
                            name,
                            path: path.to_string_lossy().into_owned(),
                            is_dir: m.is_dir(),
                            size: if m.is_dir() { 0 } else { m.len() },
                            modified,
                            is_hidden: hidden,
                            is_system: system,
                            is_symlink: entry.file_type().is_symlink(),
                            ..Default::default()
                        });
                    }
                    if options.archives && !m.is_dir() && is_archive(path) {
                        // As in Tauri, archive entries are matched by name and metadata.
                        match search_archive(path, &params, &cancellation) {
                            Ok(entries) => batch.extend(entries),
                            Err(_) if cancellation.load(Ordering::Relaxed) => return Ok(()),
                            Err(error) => {
                                log::warn!("Archive search skipped {}: {error}", path.display());
                                if sender
                                    .send(QuickSearchEvent::ArchiveSkipped {
                                        path: path.to_string_lossy().into_owned(),
                                        detail: error.to_string(),
                                    })
                                    .is_err()
                                {
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
                let remaining = limit.max(1).saturating_sub(count);
                batch.truncate(remaining);
                if !batch.is_empty()
                    && (batch.len() >= 100
                        || flush.elapsed().as_millis() >= 100
                        || batch.len() >= remaining)
                {
                    count += batch.len();
                    if sender
                        .send(QuickSearchEvent::Batch(std::mem::take(&mut batch)))
                        .is_err()
                    {
                        return Ok(());
                    }
                    flush = std::time::Instant::now();
                }
                if count >= limit.max(1) {
                    let _ = sender.send(QuickSearchEvent::Finished { limited: true });
                    return Ok(());
                }
            }
            if !batch.is_empty() && sender.send(QuickSearchEvent::Batch(batch)).is_err() {
                return Ok(());
            }
            let _ = sender.send(QuickSearchEvent::Finished { limited: false });
            Ok(())
        };
        if let Err(error) = run() {
            let _ = sender.send(QuickSearchEvent::Failed(error));
        }
    });
    receiver
}

pub fn quick_search(
    root: String,
    query: String,
    limit: usize,
    show_hidden: bool,
    show_system: bool,
    cancellation: Arc<AtomicBool>,
) -> std::sync::mpsc::Receiver<QuickSearchEvent> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(8);
    thread::spawn(move || {
        let pattern = if query.contains('*') || query.contains('?') {
            match Pattern::new(&query.to_lowercase()) {
                Ok(pattern) => SearchPattern::Glob(pattern, false, false),
                Err(error) => {
                    let _ = sender.send(QuickSearchEvent::Failed(error.to_string()));
                    return;
                }
            }
        } else {
            SearchPattern::Literal(query, false, false)
        };
        let walker = walkdir::WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| {
                if cancellation.load(Ordering::Relaxed) {
                    return false;
                }
                if entry.depth() == 0 {
                    return true;
                }
                let name = entry.file_name().to_string_lossy();
                if !show_system
                    && entry.file_type().is_dir()
                    && matches!(
                        name.to_ascii_lowercase().as_str(),
                        "windows"
                            | "programdata"
                            | "$recycle.bin"
                            | "system volume information"
                            | "winsxs"
                    )
                {
                    return false;
                }
                let Ok(metadata) = entry.metadata() else {
                    return false;
                };
                let (hidden, system, _) = crate::utils::get_file_attributes(&metadata, &name);
                (show_hidden || !hidden) && (show_system || !system)
            });
        let mut batch = Vec::new();
        let mut count = 0;
        let mut last_flush = std::time::Instant::now();
        for entry in walker {
            if cancellation.load(Ordering::Relaxed) {
                return;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) if error.depth() == 0 => {
                    let _ = sender.send(QuickSearchEvent::Failed(error.to_string()));
                    return;
                }
                Err(_) => continue,
            };
            if entry.depth() == 0 {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if pattern.matches(&name) {
                if let Ok(metadata) = entry.metadata() {
                    let (hidden, system, _) = crate::utils::get_file_attributes(&metadata, &name);
                    batch.push(FileEntry {
                        name,
                        path: entry.path().to_string_lossy().into_owned(),
                        is_dir: metadata.is_dir(),
                        is_hidden: hidden,
                        is_system: system,
                        is_symlink: entry.file_type().is_symlink(),
                        size: if metadata.is_dir() { 0 } else { metadata.len() },
                        modified: metadata
                            .modified()
                            .unwrap_or(SystemTime::UNIX_EPOCH)
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                        ..FileEntry::default()
                    });
                    count += 1;
                }
            }
            if !batch.is_empty()
                && (batch.len() >= 100
                    || last_flush.elapsed().as_millis() >= 100
                    || count >= limit.max(1))
            {
                if sender
                    .send(QuickSearchEvent::Batch(std::mem::take(&mut batch)))
                    .is_err()
                {
                    return;
                }
                last_flush = std::time::Instant::now();
            }
            if count >= limit.max(1) {
                let _ = sender.send(QuickSearchEvent::Finished { limited: true });
                return;
            }
        }
        if !batch.is_empty() && sender.send(QuickSearchEvent::Batch(batch)).is_err() {
            return;
        }
        let _ = sender.send(QuickSearchEvent::Finished { limited: false });
    });
    receiver
}

impl SearchPattern {
    fn matches(&self, text: &str) -> bool {
        match self {
            SearchPattern::Glob(p, ia, case_sensitive) => {
                let target = if *ia {
                    crate::utils::remove_accents(text)
                } else {
                    text.to_string()
                };
                let target = if *case_sensitive {
                    target
                } else {
                    target.to_lowercase()
                };
                p.matches_with(
                    &target,
                    glob::MatchOptions {
                        case_sensitive: *case_sensitive,
                        require_literal_separator: false,
                        require_literal_leading_dot: false,
                    },
                )
            }
            SearchPattern::Regex(r, ia) => {
                if *ia {
                    r.is_match(&crate::utils::remove_accents(text))
                } else {
                    r.is_match(text)
                }
            }
            SearchPattern::Literal(q, cs, ia) => {
                let (target, query) = if *ia {
                    (
                        crate::utils::remove_accents(text),
                        crate::utils::remove_accents(q),
                    )
                } else {
                    (text.to_string(), q.clone())
                };

                if *cs {
                    target.contains(&query)
                } else {
                    target.to_lowercase().contains(&query.to_lowercase())
                }
            }
        }
    }
}

struct SearchParams {
    pattern: SearchPattern,
    min_size: Option<u64>,
    max_size: Option<u64>,
    min_date: Option<u64>,
    max_date: Option<u64>,
}

fn search_archive(
    archive_path: &std::path::Path,
    params: &SearchParams,
    cancellation: &Arc<AtomicBool>,
) -> Result<Vec<FileEntry>, crate::models::CommandError> {
    let mut results = Vec::new();
    let format = match ArchiveFormat::from_path(archive_path) {
        Some(f) => f,
        None => return Ok(results),
    };

    let checkpoint = || {
        if cancellation.load(Ordering::Relaxed) {
            Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "cancelled",
            ))
        } else {
            Ok(())
        }
    };
    let index = super::archive::index_archive_contents(archive_path, &checkpoint);
    for entry in index? {
        if checkpoint().is_err() {
            break;
        }
        if !params.pattern.matches(&entry.name) {
            continue;
        }
        if !entry.is_dir
            && (params.min_size.is_some_and(|min| entry.size < min)
                || params.max_size.is_some_and(|max| entry.size > max))
        {
            continue;
        }
        // ISO and 7z dates remain unavailable, as in the previous readers.
        if !matches!(format, ArchiveFormat::Iso | ArchiveFormat::SevenZip)
            && (params.min_date.is_some_and(|min| entry.modified < min)
                || params.max_date.is_some_and(|max| entry.modified > max))
        {
            continue;
        }
        results.push(entry);
    }

    Ok(results)
}

#[cfg(test)]
fn search_in_archive(
    path: &std::path::Path,
    params: &SearchParams,
    cancellation: &Arc<AtomicBool>,
) -> Vec<FileEntry> {
    search_archive(path, params, cancellation).unwrap_or_default()
}

#[cfg(test)]
mod archive_search_tests {
    use super::*;

    #[test]
    fn archive_search_warns_and_keeps_results_after_multiple_skipped_archives() {
        let temp = tempfile::tempdir().unwrap();
        let extensions = [
            "zip", "tar", "tar.gz", "tar.xz", "tar.zst", "tar.bz2", "iso", "7z", "rar",
        ];
        // More warnings than the channel capacity exercise backpressure too.
        for extension in extensions {
            std::fs::write(temp.path().join(format!("broken.{extension}")), b"bad").unwrap();
        }
        std::fs::write(temp.path().join("match.txt"), b"valid result").unwrap();
        let receiver = advanced_search(
            temp.path().to_string_lossy().into_owned(),
            AdvancedSearchOptions {
                query: "match".into(),
                archives: true,
                ..Default::default()
            },
            100,
            true,
            true,
            Arc::new(AtomicBool::new(false)),
        );
        let mut skipped = std::collections::HashSet::new();
        let mut results = Vec::new();
        loop {
            let event = receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            let terminal = event.is_terminal();
            match event {
                QuickSearchEvent::ArchiveSkipped { path, detail } => {
                    assert!(!terminal);
                    assert!(std::path::Path::new(&path).starts_with(temp.path()));
                    assert!(!detail.is_empty());
                    skipped.insert(path);
                }
                QuickSearchEvent::Batch(batch) => {
                    assert!(!terminal);
                    results.extend(batch);
                }
                QuickSearchEvent::Finished { limited } => {
                    assert!(terminal);
                    assert!(!limited);
                    break;
                }
                QuickSearchEvent::Failed(error) => panic!("{error}"),
            }
        }
        assert_eq!(skipped.len(), extensions.len());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "match.txt");
    }

    #[test]
    fn archive_search_warning_preserves_the_limit_kind_and_threshold() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("deep.zip");
        let name = format!("{}match.txt", "n/".repeat(64));
        metadata_fixture(&path, &[(&name, tar::EntryType::Regular)], 123);
        let receiver = advanced_search(
            temp.path().to_string_lossy().into_owned(),
            AdvancedSearchOptions {
                query: "match".into(),
                archives: true,
                ..Default::default()
            },
            100,
            true,
            true,
            Arc::new(AtomicBool::new(false)),
        );
        let mut warning = None;
        loop {
            match receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
            {
                QuickSearchEvent::ArchiveSkipped {
                    path: skipped,
                    detail,
                } => {
                    assert_eq!(std::path::Path::new(&skipped), path);
                    warning = crate::models::error::ArchiveLimitFailure::from_message(&detail);
                }
                QuickSearchEvent::Finished { limited } => {
                    assert!(!limited);
                    break;
                }
                QuickSearchEvent::Batch(batch) => assert!(batch.is_empty()),
                QuickSearchEvent::Failed(error) => panic!("{error}"),
            }
        }
        assert_eq!(
            warning,
            Some(crate::models::error::ArchiveLimitFailure {
                kind: crate::models::error::ArchiveLimitKind::Depth,
                limit: 64,
            })
        );
    }

    #[test]
    fn archive_search_cancellation_is_not_reported_as_a_skipped_archive() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("broken.zip"), b"bad").unwrap();
        let receiver = advanced_search(
            temp.path().to_string_lossy().into_owned(),
            AdvancedSearchOptions {
                archives: true,
                ..Default::default()
            },
            100,
            true,
            true,
            Arc::new(AtomicBool::new(true)),
        );
        assert!(matches!(
            receiver.recv_timeout(std::time::Duration::from_secs(5)),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
        ));
    }

    fn metadata_fixture(path: &std::path::Path, names: &[(&str, tar::EntryType)], mtime: u64) {
        use std::io::Write;
        if path.extension().is_some_and(|ext| ext == "zip") {
            let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
            for (name, kind) in names {
                if kind.is_symlink() {
                    writer
                        .add_symlink(*name, "safe.txt", zip::write::SimpleFileOptions::default())
                        .unwrap();
                } else {
                    writer
                        .start_file(
                            *name,
                            zip::write::SimpleFileOptions::default()
                                .last_modified_time(
                                    zip::DateTime::from_date_and_time(2009, 2, 13, 23, 31, 30)
                                        .unwrap(),
                                )
                                .compression_method(zip::CompressionMethod::Stored),
                        )
                        .unwrap();
                    writer.write_all(b"safe").unwrap();
                }
            }
            writer.finish().unwrap();
        } else {
            let mut builder = tar::Builder::new(Vec::new());
            for (name, kind) in names {
                let mut header = tar::Header::new_gnu();
                header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
                header.set_entry_type(*kind);
                header.set_mode(0o644);
                header.set_size(4);
                header.set_mtime(mtime);
                header.set_cksum();
                builder.append(&header, &b"safe"[..]).unwrap();
            }
            let bytes = builder.into_inner().unwrap();
            let format = ArchiveFormat::from_path(path).unwrap();
            let mut contents = Vec::new();
            match format {
                ArchiveFormat::Tar => contents = bytes,
                ArchiveFormat::TarZst => {
                    contents = zstd::stream::encode_all(&bytes[..], 1).unwrap()
                }
                format => {
                    let mut encoder: Box<dyn Read + '_> = match format {
                        ArchiveFormat::TarGz => Box::new(flate2::read::GzEncoder::new(
                            &bytes[..],
                            flate2::Compression::fast(),
                        )),
                        ArchiveFormat::TarXz => Box::new(xz2::read::XzEncoder::new(&bytes[..], 1)),
                        ArchiveFormat::TarBz2 => Box::new(bzip2::read::BzEncoder::new(
                            &bytes[..],
                            bzip2::Compression::fast(),
                        )),
                        _ => panic!("unsupported metadata fixture"),
                    };
                    encoder.read_to_end(&mut contents).unwrap();
                }
            }
            std::fs::write(path, contents).unwrap();
        }
    }

    #[test]
    fn zip_tar_metadata_search_rejects_unsafe_names_before_returning_results() {
        let temp = tempfile::tempdir().unwrap();
        let mut rejected = Vec::new();
        for extension in ["zip", "tar", "tar.gz", "tar.xz", "tar.zst", "tar.bz2"] {
            let path = temp.path().join(format!("unsafe.{extension}"));
            metadata_fixture(
                &path,
                &[
                    ("safe.txt", tar::EntryType::Regular),
                    ("../outside.txt", tar::EntryType::Regular),
                ],
                123,
            );
            let result = search_in_archive(&path, &params(), &Arc::new(AtomicBool::new(false)));
            rejected.push((extension, result.is_empty()));
        }
        assert!(
            rejected.iter().all(|(_, empty)| *empty),
            "unsafe index rejection: {rejected:?}"
        );
    }

    #[test]
    fn zip_tar_metadata_rejects_special_entries_in_search_and_navigation() {
        let temp = tempfile::tempdir().unwrap();
        let mut rejected = Vec::new();
        for extension in ["zip", "tar", "tar.gz", "tar.xz", "tar.zst", "tar.bz2"] {
            let path = temp.path().join(format!("link.{extension}"));
            metadata_fixture(
                &path,
                &[
                    ("safe.txt", tar::EntryType::Regular),
                    ("link.txt", tar::EntryType::Symlink),
                ],
                123,
            );
            let navigation = super::super::archive::list_archive_contents(
                path.to_string_lossy().into_owned(),
                String::new(),
            )
            .is_err();
            let search =
                search_in_archive(&path, &params(), &Arc::new(AtomicBool::new(false))).is_empty();
            rejected.push((extension, navigation, search));
        }
        assert!(
            rejected
                .iter()
                .all(|(_, navigation, search)| *navigation && *search),
            "special index rejection: {rejected:?}"
        );
    }

    #[test]
    fn zip_tar_metadata_rejects_overflowing_tar_dates_without_panicking() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("date.tar");
        metadata_fixture(&path, &[("safe.txt", tar::EntryType::Regular)], u64::MAX);
        let result = std::panic::catch_unwind(|| {
            search_in_archive(&path, &params(), &Arc::new(AtomicBool::new(false)))
        });
        let listing = std::panic::catch_unwind(|| {
            super::super::archive::list_archive_contents(
                path.to_string_lossy().into_owned(),
                String::new(),
            )
        });
        assert!(
            result.is_ok() && listing.is_ok(),
            "TAR timestamp panicked: search={}, navigation={}",
            result.is_err(),
            listing.is_err()
        );
        assert!(result.unwrap().is_empty());
        assert!(listing.unwrap().is_err());
    }

    #[test]
    fn zip_tar_metadata_search_preserves_name_size_date_filters_and_cancellation() {
        let temp = tempfile::tempdir().unwrap();
        for extension in ["zip", "tar", "tar.gz", "tar.xz", "tar.zst", "tar.bz2"] {
            let path = temp.path().join(format!("filters.{extension}"));
            metadata_fixture(
                &path,
                &[("folder/Été.txt", tar::EntryType::Regular)],
                1_234_567_890,
            );
            let cancellation = Arc::new(AtomicBool::new(false));
            let mut options = params();
            options.pattern = SearchPattern::Literal("ETE".into(), false, true);
            options.min_size = Some(4);
            options.max_size = Some(4);
            options.min_date = Some(1_234_567_890_000);
            options.max_date = Some(1_234_567_890_000);
            let result = search_in_archive(&path, &options, &cancellation);
            assert_eq!(result.len(), 1, "{extension}");
            assert_eq!(
                std::path::Path::new(&result[0].path),
                path.join("folder/Été.txt")
            );
            options.max_size = Some(3);
            assert!(search_in_archive(&path, &options, &cancellation).is_empty());
            options.max_size = Some(4);
            options.max_date = Some(1_234_567_889_999);
            assert!(search_in_archive(&path, &options, &cancellation).is_empty());
            options.max_date = None;
            cancellation.store(true, Ordering::Relaxed);
            assert!(search_in_archive(&path, &options, &cancellation).is_empty());
        }
    }

    #[test]
    fn zip_tar_metadata_zip_names_do_not_require_a_supported_content_codec() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("codec.zip");
        metadata_fixture(&path, &[("safe.txt", tar::EntryType::Regular)], 123);
        let mut bytes = std::fs::read(&path).unwrap();
        let local = bytes.windows(4).position(|b| b == b"PK\x03\x04").unwrap();
        let central = bytes.windows(4).position(|b| b == b"PK\x01\x02").unwrap();
        bytes[local + 8..local + 10].copy_from_slice(&0x7fu16.to_le_bytes());
        bytes[central + 10..central + 12].copy_from_slice(&0x7fu16.to_le_bytes());
        std::fs::write(&path, bytes).unwrap();
        assert!(zip::ZipArchive::new(File::open(&path).unwrap())
            .unwrap()
            .by_index(0)
            .is_err());
        let results = search_in_archive(&path, &params(), &Arc::new(AtomicBool::new(false)));
        assert_eq!(
            results.len(),
            1,
            "name search constructed a ZIP content decoder"
        );
        assert_eq!(
            super::super::archive::list_archive_contents(
                path.to_string_lossy().into_owned(),
                String::new()
            )
            .unwrap()
            .len(),
            1
        );
    }

    fn params() -> SearchParams {
        SearchParams {
            pattern: SearchPattern::Literal(String::new(), false, false),
            min_size: None,
            max_size: None,
            min_date: None,
            max_date: None,
        }
    }

    #[test]
    fn seven_search_lists_header_names_without_constructing_a_content_decoder() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("unsupported-codec.7z");
        super::super::archive_seven_open::block_crc_fixture_with_method(&archive, false, 0x7f);
        let mut reader = sevenz_rust2::ArchiveReader::new(
            File::open(&archive).unwrap(),
            sevenz_rust2::Password::empty(),
        )
        .unwrap();
        assert_eq!(reader.archive().files.len(), 2);
        assert!(reader.for_each_entries(|_, _| Ok(true)).is_err());
        let results = search_in_archive(&archive, &params(), &Arc::new(AtomicBool::new(false)));
        assert_eq!(results.len(), 2, "name search depends on a content decoder");
        assert_eq!(results[0].name, "selected.txt");
        assert_eq!(results[1].name, "tail.txt");
    }

    #[test]
    fn seven_search_preserves_size_filters_and_cancellation() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("filters.7z");
        super::super::archive_seven_open::block_crc_fixture_with_method(&archive, false, 0);
        let cancellation = Arc::new(AtomicBool::new(false));
        let mut params = params();
        params.pattern = SearchPattern::Literal("SELECTED".into(), false, false);
        params.min_size = Some(4);
        params.max_size = Some(4);
        let results = search_in_archive(&archive, &params, &cancellation);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].size, 4);
        assert_eq!(
            std::path::Path::new(&results[0].path),
            archive.join("selected.txt")
        );
        params.min_size = Some(5);
        params.max_size = None;
        assert!(search_in_archive(&archive, &params, &cancellation).is_empty());
        params.min_size = None;
        cancellation.store(true, Ordering::Relaxed);
        assert!(search_in_archive(&archive, &params, &cancellation).is_empty());
    }

    #[test]
    fn iso_search_keeps_nested_paths_size_filters_and_cancellation() {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("disc.img");
        crate::commands::archive::test_iso_image(&image);
        let cancellation = Arc::new(AtomicBool::new(false));
        let mut params = params();
        let results = search_in_archive(&image, &params, &cancellation);
        assert_eq!(results.len(), 3);
        assert!(results
            .iter()
            .any(|entry| entry.is_dir && entry.name == "DOCS"));
        params.min_size = Some(5);
        params.max_size = Some(11);
        params.pattern = SearchPattern::Literal("readme".into(), false, false);
        let results = search_in_archive(&image, &params, &cancellation);
        assert_eq!(results.len(), 1);
        assert_eq!(
            std::path::Path::new(&results[0].path),
            image.join("DOCS/README.TXT")
        );
        assert_eq!(results[0].size, 11);
        cancellation.store(true, Ordering::Relaxed);
        assert!(search_in_archive(&image, &params, &cancellation).is_empty());
    }

    #[test]
    fn iso_search_rejects_malformed_identifiers_incomplete_records_and_cycles() {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("invalid.iso");
        for variant in 0..3 {
            crate::commands::archive::test_iso_image(&image);
            let mut bytes = std::fs::read(&image).unwrap();
            let directory = 20 * 2048 + 2 * 34;
            let file = directory + bytes[directory] as usize;
            match variant {
                0 => bytes[file + 32] = 255,
                1 => bytes[file] = 20,
                2 => {
                    bytes[directory + 2..directory + 6].copy_from_slice(&20u32.to_le_bytes());
                    bytes[directory + 6..directory + 10].copy_from_slice(&20u32.to_be_bytes());
                }
                _ => unreachable!(),
            }
            std::fs::write(&image, bytes).unwrap();
            assert!(
                search_in_archive(&image, &params(), &Arc::new(AtomicBool::new(false))).is_empty()
            );
        }
    }
}

fn is_office_doc(path: &std::path::Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(
        ext.as_str(),
        "docx"
            | "xlsx"
            | "pptx"
            | "docm"
            | "xlsm"
            | "pptm"
            | "odt"
            | "ods"
            | "odp"
            | "ott"
            | "ots"
            | "otp"
    )
}

fn is_binary_file(path: &std::path::Path) -> bool {
    if is_office_doc(path) {
        return false;
    }

    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return true,
    };

    let mut buffer = [0u8; 1024];
    match file.read(&mut buffer) {
        Ok(n) => buffer[..n].contains(&0),
        Err(_) => true,
    }
}

/// Bound each blocking read and check cancellation before/after it. A regular
/// I/O error is used because read_to_string/read_line retry Interrupted errors.
struct ContentReader<'a, R> {
    inner: R,
    cancellation: &'a AtomicBool,
}

impl<R: Read> Read for ContentReader<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.cancellation.load(Ordering::Relaxed) {
            return Err(std::io::Error::other("Search cancelled"));
        }
        let length = buffer.len().min(64 * 1024);
        let read = self.inner.read(&mut buffer[..length])?;
        if self.cancellation.load(Ordering::Relaxed) {
            return Err(std::io::Error::other("Search cancelled"));
        }
        Ok(read)
    }
}

fn office_file_contains_content(
    path: &std::path::Path,
    pattern: &Regex,
    ignore_accents: bool,
    cancellation: &AtomicBool,
) -> bool {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => return false,
    };

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let targets = match ext.as_str() {
        "docx" | "docm" => vec!["word/document.xml"],
        "xlsx" | "xlsm" => vec!["xl/sharedStrings.xml"], // Shared strings is where 99% of text lives
        "pptx" | "pptm" => {
            // Powerpoint is tricky, slides are separate. We'll check the first 5 slides for performance
            vec![
                "ppt/slides/slide1.xml",
                "ppt/slides/slide2.xml",
                "ppt/slides/slide3.xml",
            ]
        }
        "odt" | "ods" | "odp" | "ott" | "ots" | "otp" => vec!["content.xml"],
        _ => vec!["content.xml", "word/document.xml"],
    };

    for target in targets {
        if cancellation.load(Ordering::Relaxed) {
            return false;
        }
        if let Ok(mut content_file) = archive.by_name(target) {
            if office_entry_contains_content(
                &mut content_file,
                pattern,
                ignore_accents,
                cancellation,
            ) {
                return true;
            }
        }
    }

    false
}

fn office_entry_contains_content(
    reader: impl Read,
    pattern: &Regex,
    ignore_accents: bool,
    cancellation: &AtomicBool,
) -> bool {
    let mut content = String::new();
    let mut reader = ContentReader {
        inner: reader,
        cancellation,
    };
    if reader.read_to_string(&mut content).is_err() {
        return false;
    }
    let mut in_tag = false;
    let mut tag_buffer = String::new();
    let mut stripped = String::with_capacity(content.len() / 2);

    for (index, c) in content.chars().enumerate() {
        if index % 8192 == 0 && cancellation.load(Ordering::Relaxed) {
            return false;
        }
        if c == '<' {
            in_tag = true;
            tag_buffer.clear();
        } else if c == '>' {
            in_tag = false;
            let tl = &tag_buffer;
            if tl.starts_with("w:p")
                || tl.starts_with("/w:p")
                || tl.starts_with("w:br")
                || tl.starts_with("text:p")
                || tl.starts_with("/text:p")
                || tl == "p"
                || tl == "/p"
            {
                stripped.push('\n');
            }
        } else if in_tag {
            if tag_buffer.len() < 10 {
                tag_buffer.push(c);
            }
        } else {
            stripped.push(c);
        }
    }

    for line in stripped.lines() {
        if cancellation.load(Ordering::Relaxed) {
            return false;
        }
        let matched = if ignore_accents {
            pattern.is_match(&crate::utils::remove_accents(line))
        } else {
            pattern.is_match(line)
        };
        if matched && !cancellation.load(Ordering::Relaxed) {
            return true;
        }
    }
    false
}

fn content_lock<'a>(
    lock: &'a Mutex<()>,
    cancellation: &AtomicBool,
) -> Option<std::sync::MutexGuard<'a, ()>> {
    loop {
        if cancellation.load(Ordering::Relaxed) {
            return None;
        }
        match lock.try_lock() {
            Ok(guard) => return Some(guard),
            Err(std::sync::TryLockError::Poisoned(error)) => return Some(error.into_inner()),
            Err(std::sync::TryLockError::WouldBlock) => {
                thread::sleep(std::time::Duration::from_millis(10))
            }
        }
    }
}

fn file_contains_content(
    path: &std::path::Path,
    pattern: &Regex,
    ignore_accents: bool,
    cancellation: &AtomicBool,
) -> bool {
    if cancellation.load(Ordering::Relaxed) || is_binary_file(path) {
        return false;
    }

    let is_office = is_office_doc(path);

    // Preserve the existing serialization per physical disk. The unused SSD
    // bypass is removed; waiting for another search remains cancellable.
    let vol_id = get_physical_disk_id(path);
    let lock = DISK_IO_LOCKS
        .entry(vol_id)
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone();
    let Some(_guard) = content_lock(&lock, cancellation) else {
        return false;
    };
    if is_office {
        office_file_contains_content(path, pattern, ignore_accents, cancellation)
    } else {
        read_file_and_check(path, pattern, ignore_accents, cancellation)
    }
}

fn read_file_and_check(
    path: &std::path::Path,
    pattern: &Regex,
    ignore_accents: bool,
    cancellation: &AtomicBool,
) -> bool {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    text_reader_contains_content(file, pattern, ignore_accents, cancellation)
}

fn text_reader_contains_content(
    reader: impl Read,
    pattern: &Regex,
    ignore_accents: bool,
    cancellation: &AtomicBool,
) -> bool {
    let reader = BufReader::new(ContentReader {
        inner: reader,
        cancellation,
    });
    for line in reader.lines().map_while(Result::ok) {
        if cancellation.load(Ordering::Relaxed) {
            return false;
        }
        let matched = if ignore_accents {
            pattern.is_match(&crate::utils::remove_accents(&line))
        } else {
            pattern.is_match(&line)
        };
        if matched && !cancellation.load(Ordering::Relaxed) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod content_tests {
    use super::*;
    use std::cell::Cell;
    use std::io::{Cursor, Write};

    struct CancelAfter<'a, R> {
        inner: R,
        bytes: &'a Cell<usize>,
        cancellation: &'a AtomicBool,
    }
    impl<R: Read> Read for CancelAfter<'_, R> {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let count = self.inner.read(buffer)?;
            self.bytes.set(self.bytes.get() + count);
            if self.bytes.get() >= 8192 {
                self.cancellation.store(true, Ordering::Relaxed);
            }
            Ok(count)
        }
    }

    #[test]
    fn text_cancellation_stops_inside_a_large_line_before_publishing_a_match() {
        let text = format!("needle{}", "x".repeat(2 * 1024 * 1024));
        let cancellation = AtomicBool::new(false);
        let bytes = Cell::new(0);
        assert!(!text_reader_contains_content(
            CancelAfter {
                inner: Cursor::new(text.as_bytes()),
                bytes: &bytes,
                cancellation: &cancellation
            },
            &Regex::new("needle").unwrap(),
            false,
            &cancellation,
        ));
        assert!(bytes.get() <= 64 * 1024);
        assert!(text_reader_contains_content(
            Cursor::new("Résumé\n"),
            &Regex::new("(?i)resume").unwrap(),
            true,
            &AtomicBool::new(false)
        ));
        assert!(!text_reader_contains_content(
            Cursor::new(b"invalid\xff\nneedle\n"),
            &Regex::new("needle").unwrap(),
            false,
            &AtomicBool::new(false)
        ));
    }

    #[test]
    fn office_cancellation_stops_during_decompression_and_keeps_normal_matching() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "word/document.xml",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
        let xml = format!("<w:p>Résumé{}<w:br/>end</w:p>", "x".repeat(2 * 1024 * 1024));
        writer.write_all(xml.as_bytes()).unwrap();
        let mut archive = zip::ZipArchive::new(writer.finish().unwrap()).unwrap();
        let cancellation = AtomicBool::new(false);
        let bytes = Cell::new(0);
        assert!(!office_entry_contains_content(
            CancelAfter {
                inner: archive.by_name("word/document.xml").unwrap(),
                bytes: &bytes,
                cancellation: &cancellation
            },
            &Regex::new("(?i)resume").unwrap(),
            true,
            &cancellation,
        ));
        assert!(bytes.get() <= 64 * 1024);
        cancellation.store(false, Ordering::Relaxed);
        assert!(office_entry_contains_content(
            archive.by_name("word/document.xml").unwrap(),
            &Regex::new("(?i)resume").unwrap(),
            true,
            &cancellation
        ));
        assert!(office_entry_contains_content(
            Cursor::new("<w:p>Résumé</w:p>"),
            &Regex::new("Résumé").unwrap(),
            false,
            &cancellation
        ));
    }

    #[test]
    fn waiting_for_disk_lock_can_be_cancelled_without_releasing_the_lock() {
        let lock = Arc::new(Mutex::new(()));
        let guard = lock.lock().unwrap();
        let cancellation = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let worker_lock = lock.clone();
        let worker_cancel = cancellation.clone();
        let worker = thread::spawn(move || {
            ready_tx.send(()).unwrap();
            done_tx
                .send(content_lock(&worker_lock, &worker_cancel).is_none())
                .unwrap();
        });
        ready_rx.recv().unwrap();
        thread::sleep(std::time::Duration::from_millis(20));
        cancellation.store(true, Ordering::Relaxed);
        assert!(done_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap());
        worker.join().unwrap();
        drop(guard);
    }
}

#[cfg(test)]
mod quick_search_tests {
    use super::*;

    fn advanced(root: &std::path::Path, options: AdvancedSearchOptions) -> Vec<FileEntry> {
        let receiver = advanced_search(
            root.to_string_lossy().into_owned(),
            options,
            100,
            true,
            true,
            Arc::new(AtomicBool::new(false)),
        );
        let mut results = Vec::new();
        loop {
            match receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
            {
                QuickSearchEvent::Batch(batch) => results.extend(batch),
                QuickSearchEvent::ArchiveSkipped { .. } => {},
                QuickSearchEvent::Finished { .. } => return results,
                QuickSearchEvent::Failed(error) => panic!("{error}"),
            }
        }
    }

    #[test]
    fn advanced_filters_content_accents_size_and_recursion() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("nested")).unwrap();
        std::fs::write(dir.path().join("Été.txt"), "Résumé").unwrap();
        std::fs::write(dir.path().join("nested/Été.txt"), "Résumé").unwrap();
        std::fs::write(dir.path().join("Other.txt"), "none").unwrap();
        let options = AdvancedSearchOptions {
            query: "ete.*".into(),
            content: "resume".into(),
            ignore_accents: true,
            min_size: Some(3),
            max_size: Some(30),
            ..Default::default()
        };
        assert_eq!(advanced(dir.path(), options.clone()).len(), 1);
        assert_eq!(
            advanced(
                dir.path(),
                AdvancedSearchOptions {
                    recursive: true,
                    ..options.clone()
                }
            )
            .len(),
            2
        );
        assert!(advanced(
            dir.path(),
            AdvancedSearchOptions {
                min_size: Some(20),
                ..options
            }
        )
        .is_empty());
    }

    #[test]
    fn advanced_regex_case_dates_and_invalid_bounds() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Photo1.txt"), "example").unwrap();
        std::fs::write(dir.path().join("photo2.txt"), "example").unwrap();
        let options = AdvancedSearchOptions {
            query: r"^Photo\d".into(),
            regex: true,
            case_sensitive: true,
            ..Default::default()
        };
        assert_eq!(advanced(dir.path(), options.clone()).len(), 1);
        assert!(advanced(
            dir.path(),
            AdvancedSearchOptions {
                max_date: Some(1),
                ..options
            }
        )
        .is_empty());
        assert!(AdvancedSearchOptions {
            query: "[".into(),
            regex: true,
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(AdvancedSearchOptions {
            min_size: Some(3),
            max_size: Some(1),
            ..Default::default()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn advanced_searches_zip_entry_names() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let file = File::create(dir.path().join("archive.zip")).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file(
                "folder/result.txt",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(b"test").unwrap();
        writer.finish().unwrap();
        let options = AdvancedSearchOptions {
            query: "result".into(),
            ..Default::default()
        };
        assert!(advanced(dir.path(), options.clone()).is_empty());
        let results = advanced(
            dir.path(),
            AdvancedSearchOptions {
                archives: true,
                ..options
            },
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "result.txt");
    }

    #[test]
    fn advanced_stream_respects_limit_and_content_regex() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..5 {
            std::fs::write(dir.path().join(format!("{index}.txt")), "photo42").unwrap();
        }
        let options = AdvancedSearchOptions {
            content: r"photo\d+".into(),
            content_regex: true,
            ..Default::default()
        };
        let receiver = advanced_search(
            dir.path().to_string_lossy().into_owned(),
            options,
            2,
            true,
            true,
            Arc::new(AtomicBool::new(false)),
        );
        let mut count = 0;
        loop {
            match receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
            {
                QuickSearchEvent::Batch(batch) => count += batch.len(),
                QuickSearchEvent::ArchiveSkipped { .. } => {},
                QuickSearchEvent::Finished { limited } => {
                    assert!(limited);
                    assert_eq!(count, 2);
                    break;
                }
                QuickSearchEvent::Failed(error) => panic!("{error}"),
            }
        }
    }

    fn collect(
        root: &std::path::Path,
        query: &str,
        limit: usize,
        cancelled: bool,
    ) -> (Vec<FileEntry>, bool) {
        let receiver = quick_search(
            root.to_string_lossy().into_owned(),
            query.to_owned(),
            limit,
            true,
            true,
            Arc::new(AtomicBool::new(cancelled)),
        );
        let mut files = Vec::new();
        while let Ok(event) = receiver.recv_timeout(std::time::Duration::from_secs(5)) {
            match event {
                QuickSearchEvent::Batch(batch) => files.extend(batch),
                QuickSearchEvent::ArchiveSkipped { .. } => {},
                QuickSearchEvent::Finished { limited } => return (files, limited),
                QuickSearchEvent::Failed(error) => panic!("{error}"),
            }
        }
        assert!(cancelled, "search did not finish");
        (files, false)
    }

    #[test]
    fn recursive_names_are_case_insensitive_and_keep_actual_paths() {
        let directory = tempfile::tempdir().unwrap();
        let nested = directory.path().join("UserFolder");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("en-US.json"), "abc").unwrap();
        std::fs::write(nested.join("other.txt"), "us in content only").unwrap();
        let (files, limited) = collect(directory.path(), "us", 100, false);
        assert!(!limited);
        assert_eq!(files.len(), 2);
        assert!(files
            .iter()
            .any(|file| file.is_dir && file.name == "UserFolder"));
        let file = files.iter().find(|file| file.name == "en-US.json").unwrap();
        assert_eq!(std::path::Path::new(&file.path), nested.join("en-US.json"));
        assert_eq!(file.size, 3);
    }

    #[test]
    fn wildcard_limit_and_cancellation() {
        let directory = tempfile::tempdir().unwrap();
        for name in ["a.json", "b.JSON", "c.txt"] {
            std::fs::write(directory.path().join(name), "").unwrap();
        }
        let (files, limited) = collect(directory.path(), "*.json", 1, false);
        assert!(limited);
        assert_eq!(files.len(), 1);
        assert!(files[0].name.to_lowercase().ends_with(".json"));
        assert!(collect(directory.path(), "*", 100, true).0.is_empty());
        assert!(collect(directory.path(), "absent", 100, false).0.is_empty());
    }

    #[test]
    fn invalid_root_reports_failure() {
        let directory = tempfile::tempdir().unwrap();
        let receiver = quick_search(
            directory
                .path()
                .join("absent")
                .to_string_lossy()
                .into_owned(),
            "us".into(),
            100,
            true,
            true,
            Arc::new(AtomicBool::new(false)),
        );
        assert!(matches!(
            receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap(),
            QuickSearchEvent::Failed(_)
        ));
    }
}
