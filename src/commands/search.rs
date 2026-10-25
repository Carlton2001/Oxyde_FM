use crate::models::FileEntry;

use crate::utils::archive::{is_archive, ArchiveFormat};
use crate::utils::hardware::get_physical_disk_id;
use dashmap::DashMap;
use glob::Pattern;
use once_cell::sync::Lazy;
use regex::RegexBuilder;
use std::fs::File;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::SystemTime;

mod content;
mod criteria;
use content::{text_reader_contains_content, ContentPattern};
use criteria::{SearchParams, SearchPattern};
pub use criteria::ArchiveSearchLimitation;

static DISK_IO_LOCKS: Lazy<DashMap<u64, Arc<Mutex<()>>>> = Lazy::new(DashMap::new);

/// Native quick search, independent of the legacy two-panel session.
pub enum QuickSearchEvent {
    Batch(Vec<FileEntry>),
    ArchiveSkipped { path: String, detail: String },
    ContentSkipped { path: String, detail: String },
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
                let query = if options.content_regex { query } else { regex::escape(&query) };
                Some(ContentPattern::new(&query, options.case_sensitive)?)
            };
            let params = SearchParams {
                pattern,
                min_size: options.min_size,
                max_size: options.max_size,
                min_date: options.min_date,
                max_date: options.max_date,
                content,
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
                        crate::utils::visibility::is_visible(hidden, system, show_hidden, show_system)
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
                    let modified = m.modified().ok()
                        .and_then(|date| date.duration_since(SystemTime::UNIX_EPOCH).ok())
                        .map(|date| date.as_millis() as u64);
                    let mut matches = params.matches_metadata(&name, m.is_dir(), m.len(), modified);
                    if let Some(pattern) = params.content.as_ref().filter(|_| matches) {
                        matches = match file_contains_content(path, pattern, options.ignore_accents, &cancellation) {
                            Ok(matched) => matched,
                            Err(_) if cancellation.load(Ordering::Relaxed) => return Ok(()),
                            Err(error) => {
                                log::warn!("Content search skipped {}: {error}", path.display());
                                if sender.send(QuickSearchEvent::ContentSkipped { path: path.to_string_lossy().into_owned(), detail: error.to_string() }).is_err() { return Ok(()); }
                                false
                            }
                        };
                    }
                    if cancellation.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    if matches {
                        let (hidden, system, _) = crate::utils::get_file_attributes(&m, &name);
                        batch.push(FileEntry {
                            content_stamp: crate::models::FileStamp::for_entry(path, &m),
                            created: m.created().ok().and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok()).and_then(|duration| duration.as_millis().try_into().ok()),
                            name,
                            path: path.to_string_lossy().into_owned(),
                            is_dir: m.is_dir(),
                            size: if m.is_dir() { 0 } else { m.len() },
                            modified: modified.unwrap_or_default(),
                            is_hidden: hidden,
                            is_system: system,
                            is_symlink: entry.file_type().is_symlink(),
                            ..Default::default()
                        });
                    }
                    if options.archives && !m.is_dir() && is_archive(path) {
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
                let Ok(metadata) = entry.metadata() else {
                    return false;
                };
                let (hidden, system, _) = crate::utils::get_file_attributes(&metadata, &name);
                crate::utils::visibility::is_visible(hidden, system, show_hidden, show_system)
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
                        content_stamp: crate::models::FileStamp::for_entry(entry.path(), &metadata),
                        created: metadata.created().ok().and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok()).and_then(|duration| duration.as_millis().try_into().ok()),
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

    // Metadata-only workers cannot verify content or supply dates for every format.
    // Report the limitation instead of treating an unchecked criterion as a match.
    let limitation = if params.content.is_some() {
        Some(ArchiveSearchLimitation::ContentNotSearched)
    } else if params.has_date_filter() && matches!(format, ArchiveFormat::Iso | ArchiveFormat::SevenZip) {
        Some(ArchiveSearchLimitation::DatesUnavailable)
    } else { None };
    if let Some(reason) = limitation {
        return Err(crate::models::CommandError::ArchiveError(reason.message().into()));
    }

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
        let modified = (!matches!(format, ArchiveFormat::Iso | ArchiveFormat::SevenZip))
            .then_some(entry.modified);
        if !params.matches_metadata(&entry.name, entry.is_dir, entry.size, modified) {
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
#[path = "search/tests/archive_search_tests.rs"]
mod archive_search_tests;

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
            | "dotx" | "dotm" | "xltx" | "xltm" | "potx" | "potm" | "ppsx" | "ppsm"
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
    pattern: &ContentPattern,
    ignore_accents: bool,
    cancellation: &AtomicBool,
) -> std::io::Result<bool> {
    if cancellation.load(Ordering::Relaxed) || is_binary_file(path) {
        return Ok(false);
    }
    let vol_id = get_physical_disk_id(path);
    let lock = DISK_IO_LOCKS.entry(vol_id).or_insert_with(|| Arc::new(Mutex::new(()))).clone();
    let Some(_guard) = content_lock(&lock, cancellation) else { return Ok(false); };
    if is_office_doc(path) {
        let text = crate::platform::platform().office_search_text(path, &|| {
            if cancellation.load(Ordering::Relaxed) { Err(std::io::ErrorKind::Interrupted.into()) } else { Ok(()) }
        })?;
        Ok(text_reader_contains_content(std::io::Cursor::new(text), pattern, ignore_accents, cancellation))
    } else {
        Ok(read_file_and_check(path, pattern, ignore_accents, cancellation))
    }
}

fn read_file_and_check(
    path: &std::path::Path,
    pattern: &ContentPattern,
    ignore_accents: bool,
    cancellation: &AtomicBool,
) -> bool {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    text_reader_contains_content(file, pattern, ignore_accents, cancellation)
}

#[cfg(test)]
#[path = "search/tests/content_tests.rs"]
mod content_tests;

#[cfg(test)]
#[path = "search/tests/quick_search_tests.rs"]
mod quick_search_tests;

#[cfg(test)]
#[path = "search/tests/criteria_tests.rs"]
mod criteria_tests;

#[cfg(test)]
#[path = "search/tests/visibility_tests.rs"]
mod visibility_tests;

#[cfg(test)]
#[path = "search/tests/content_stamps.rs"]
mod content_stamp_tests;
