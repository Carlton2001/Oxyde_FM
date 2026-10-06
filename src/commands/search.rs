use crate::models::session::SearchContext;
use crate::models::{FileEntry, SessionManager};

use crate::utils::archive::{is_archive, ArchiveFormat};
use crate::utils::hardware::{get_physical_disk_id, is_network_path, is_ssd};
use dashmap::DashMap;
use glob::Pattern;
use iso9660_core::iso9660entry::{ISO9660Record, IsISO9660Record};
use log::info;
use once_cell::sync::Lazy;
use regex::{Regex, RegexBuilder};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::SystemTime;

use crate::platform::{platform, ThreadPriority};

/*
#[derive(Clone, serde::Serialize)]
struct SearchEvent {
    pub panel_id: String,
    pub tab_id: String,
    pub results: Vec<FileEntry>,
    pub completed: bool,
    pub request_id: Option<u64>,
}
*/

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
    Finished { limited: bool },
    Failed(String),
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
                            file_contains_content(path, r, options.ignore_accents, false)
                        });
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
                            is_readonly: m.permissions().readonly(),
                            ..Default::default()
                        });
                    }
                    if options.archives && !m.is_dir() && is_archive(path) {
                        // As in Tauri, archive entries are matched by name and metadata.
                        batch.extend(search_in_archive(path, &params, &cancellation));
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
                        is_readonly: metadata.permissions().readonly(),
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

fn search_in_archive(
    archive_path: &std::path::Path,
    params: &SearchParams,
    cancellation: &Arc<AtomicBool>,
) -> Vec<FileEntry> {
    let mut results = Vec::new();
    let format = match ArchiveFormat::from_path(archive_path) {
        Some(f) => f,
        None => return results,
    };

    match format {
        ArchiveFormat::Zip => {
            if let Ok(file) = File::open(archive_path) {
                if let Ok(mut archive) = zip::ZipArchive::new(file) {
                    for i in 0..archive.len() {
                        if cancellation.load(Ordering::Relaxed) {
                            break;
                        }
                        if let Ok(file) = archive.by_index(i) {
                            let name_with_path = file.name().replace('\\', "/");
                            let last_part = name_with_path.split('/').next_back().unwrap_or("");
                            if last_part.is_empty() {
                                continue;
                            }

                            if params.pattern.matches(last_part) {
                                let is_dir = file.is_dir();
                                let size = if is_dir { 0 } else { file.size() };

                                // Basic size filter
                                if !is_dir {
                                    if let Some(min) = params.min_size {
                                        if size < min {
                                            continue;
                                        }
                                    }
                                    if let Some(max) = params.max_size {
                                        if size > max {
                                            continue;
                                        }
                                    }
                                }

                                let modified = file
                                    .last_modified()
                                    .and_then(|dt| {
                                        let t: Result<chrono::NaiveDateTime, _> = dt.try_into();
                                        t.ok()
                                    })
                                    .map(|ts| ts.and_utc().timestamp_millis() as u64)
                                    .unwrap_or(0);

                                if let Some(min) = params.min_date {
                                    if modified < min {
                                        continue;
                                    }
                                }
                                if let Some(max) = params.max_date {
                                    if modified > max {
                                        continue;
                                    }
                                }

                                results.push(FileEntry {
                                    name: last_part.to_string(),
                                    path: format!(
                                        "{}\\{}",
                                        archive_path.to_string_lossy(),
                                        name_with_path.replace('/', "\\")
                                    ),
                                    is_dir,
                                    is_hidden: false,
                                    size,
                                    modified,
                                    ..FileEntry::default()
                                });
                            }
                        }
                    }
                }
            }
        }
        ArchiveFormat::SevenZip => {
            if let Ok(file) = File::open(archive_path) {
                if let Ok(mut reader) =
                    sevenz_rust2::ArchiveReader::new(file, sevenz_rust2::Password::empty())
                {
                    let _ = reader.for_each_entries(|entry, _| {
                        if cancellation.load(Ordering::Relaxed) {
                            return Ok(false);
                        }
                        let name_with_path = entry.name().replace('\\', "/");
                        let last_part = name_with_path.split('/').next_back().unwrap_or("");
                        if last_part.is_empty() {
                            return Ok(true);
                        }

                        if params.pattern.matches(last_part) {
                            let is_dir = entry.is_directory();
                            let size = entry.size();

                            if !is_dir {
                                if let Some(min) = params.min_size {
                                    if size < min {
                                        return Ok(true);
                                    }
                                }
                                if let Some(max) = params.max_size {
                                    if size > max {
                                        return Ok(true);
                                    }
                                }
                            }

                            results.push(FileEntry {
                                name: last_part.to_string(),
                                path: format!(
                                    "{}\\{}",
                                    archive_path.to_string_lossy(),
                                    name_with_path.replace('/', "\\")
                                ),
                                is_dir,
                                is_hidden: false,
                                size,
                                modified: 0,
                                folders_count: None,
                                files_count: None,
                                ..FileEntry::default()
                            });
                        }
                        Ok(true)
                    });
                }
            }
        }
        ArchiveFormat::Tar
        | ArchiveFormat::TarGz
        | ArchiveFormat::TarXz
        | ArchiveFormat::TarZst
        | ArchiveFormat::TarBz2 => {
            if let Ok(file) = File::open(archive_path) {
                let reader: Box<dyn Read> = match format {
                    ArchiveFormat::TarGz => Box::new(flate2::read::GzDecoder::new(file)),
                    ArchiveFormat::TarXz => Box::new(xz2::read::XzDecoder::new(file)),
                    ArchiveFormat::TarBz2 => Box::new(bzip2::read::BzDecoder::new(file)),
                    ArchiveFormat::TarZst => match zstd::stream::read::Decoder::new(file) {
                        Ok(d) => Box::new(d),
                        Err(_) => return results,
                    },
                    _ => Box::new(file),
                };

                let mut archive = tar::Archive::new(reader);
                if let Ok(entries) = archive.entries() {
                    for entry in entries.flatten() {
                        if cancellation.load(Ordering::Relaxed) {
                            break;
                        }
                        if let Ok(path) = entry.path() {
                            let name_with_path = path.to_string_lossy().replace('\\', "/");
                            let last_part = name_with_path.split('/').next_back().unwrap_or("");
                            if last_part.is_empty() {
                                continue;
                            }

                            if params.pattern.matches(last_part) {
                                let is_dir = entry.header().entry_type().is_dir();
                                let size = entry.header().size().unwrap_or(0);

                                if !is_dir {
                                    if let Some(min) = params.min_size {
                                        if size < min {
                                            continue;
                                        }
                                    }
                                    if let Some(max) = params.max_size {
                                        if size > max {
                                            continue;
                                        }
                                    }
                                }

                                let modified = entry.header().mtime().unwrap_or(0) * 1000;
                                if let Some(min) = params.min_date {
                                    if modified < min {
                                        continue;
                                    }
                                }
                                if let Some(max) = params.max_date {
                                    if modified > max {
                                        continue;
                                    }
                                }

                                results.push(FileEntry {
                                    name: last_part.to_string(),
                                    path: format!(
                                        "{}\\{}",
                                        archive_path.to_string_lossy(),
                                        name_with_path.replace('/', "\\")
                                    ),
                                    is_dir,
                                    is_hidden: false,
                                    size,
                                    modified,
                                    ..FileEntry::default()
                                });
                            }
                        }
                    }
                }
            }
        }
        ArchiveFormat::Iso => {
            if let Ok(file) = File::open(archive_path) {
                if let Ok(mut iso) = iso9660_core::ISO9660::load(file) {
                    search_in_iso(
                        &mut iso,
                        "/",
                        params,
                        cancellation,
                        archive_path,
                        &mut results,
                    );
                }
            }
        }
        _ => {}
    }

    results
}

fn search_in_iso(
    iso: &mut iso9660_core::ISO9660<File>,
    internal_path: &str,
    params: &SearchParams,
    cancellation: &Arc<AtomicBool>,
    archive_path: &std::path::Path,
    results: &mut Vec<FileEntry>,
) {
    if cancellation.load(Ordering::Relaxed) {
        return;
    }

    let mut iter = match iso.listdir(internal_path) {
        Ok(it) => it,
        Err(_) => return,
    };

    let mut records = Vec::new();
    while let Some(rec) = iter.next(iso) {
        records.push(rec);
    }

    for rec in records {
        if cancellation.load(Ordering::Relaxed) {
            break;
        }

        let name = match &rec {
            ISO9660Record::Directory(d) => d.identifier(),
            ISO9660Record::File(f) => f.identifier(),
        };
        if name == "." || name == ".." {
            continue;
        }

        let display_name = name.split(';').next().unwrap_or(name);
        let new_internal = if internal_path == "/" {
            format!("/{}", display_name)
        } else {
            format!("{}/{}", internal_path.trim_end_matches('/'), display_name)
        };

        if params.pattern.matches(display_name) {
            let is_dir = matches!(rec, ISO9660Record::Directory(_));
            let size = match &rec {
                ISO9660Record::File(f) => f.data_length() as u64,
                _ => 0,
            };

            // Filters
            if !is_dir {
                if let Some(min) = params.min_size {
                    if size < min {
                        continue;
                    }
                }
                if let Some(max) = params.max_size {
                    if size > max {
                        continue;
                    }
                }
            }

            // Note: date filtering is skipped for ISO as we don't parse it yet from this crate

            results.push(FileEntry {
                name: display_name.to_string(),
                path: format!(
                    "{}\\{}",
                    archive_path.to_string_lossy(),
                    new_internal.trim_start_matches('/').replace('/', "\\")
                ),
                is_dir,
                size,
                folders_count: None,
                files_count: None,
                ..FileEntry::default()
            });
        }

        if matches!(&rec, ISO9660Record::Directory(_)) {
            search_in_iso(
                iso,
                &new_internal,
                params,
                cancellation,
                archive_path,
                results,
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

fn office_file_contains_content(
    path: &std::path::Path,
    pattern: &Regex,
    ignore_accents: bool,
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
        if let Ok(mut content_file) = archive.by_name(target) {
            let mut content = String::new();
            if content_file.read_to_string(&mut content).is_ok() {
                let mut in_tag = false;
                let mut tag_buffer = String::new();
                let mut stripped = String::with_capacity(content.len() / 2);

                for c in content.chars() {
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

                let check_text = if ignore_accents {
                    crate::utils::remove_accents(&stripped)
                } else {
                    stripped
                };
                for line in check_text.lines() {
                    if pattern.is_match(line) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

fn file_contains_content(
    path: &std::path::Path,
    pattern: &Regex,
    ignore_accents: bool,
    ssd_hint: bool,
) -> bool {
    if is_binary_file(path) {
        return false;
    }

    let is_office = is_office_doc(path);

    // Hardware-aware throttling
    if !ssd_hint {
        let vol_id = get_physical_disk_id(path);
        let lock = DISK_IO_LOCKS
            .entry(vol_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();
        let _guard = lock.lock().unwrap();
        if is_office {
            office_file_contains_content(path, pattern, ignore_accents)
        } else {
            read_file_and_check(path, pattern, ignore_accents)
        }
    } else if is_office {
        office_file_contains_content(path, pattern, ignore_accents)
    } else {
        read_file_and_check(path, pattern, ignore_accents)
    }
}

fn read_file_and_check(path: &std::path::Path, pattern: &Regex, ignore_accents: bool) -> bool {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let reader = BufReader::new(file);
    for line in reader.lines().map_while(Result::ok) {
        if ignore_accents {
            if pattern.is_match(&crate::utils::remove_accents(&line)) {
                return true;
            }
        } else if pattern.is_match(&line) {
            return true;
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub async fn start_search(
    session_manager: crate::models::SessionManager,
    config_manager: crate::models::ConfigManager,
    panel_id: String,
    tab_id: String,
    query: String,
    search_root: Option<String>,
    regex: Option<bool>,
    case_sensitive: Option<bool>,
    recursive: Option<bool>,
    min_size: Option<u64>,
    max_size: Option<u64>,
    min_date: Option<u64>,
    max_date: Option<u64>,
    content_query: Option<String>,
    content_regex: Option<bool>,
    ignore_accents: Option<bool>,
    search_in_archives: Option<bool>,
    _request_id: Option<u64>,
) -> Result<(), String> {
    let cancellation = Arc::new(AtomicBool::new(false));
    let cancel_thread = cancellation.clone();

    // 1. Setup Session Context
    let root_path = {
        let mut session = session_manager.0.lock().map_err(|e| e.to_string())?;
        let panel = if panel_id == "left" {
            &mut session.left_panel
        } else {
            &mut session.right_panel
        };

        let mut path_to_search = if let Some(root) = search_root {
            std::path::PathBuf::from(root)
        } else {
            panel
                .tabs
                .iter()
                .find(|t| t.id == panel.active_tab_id)
                .map(|t| t.path.clone())
                .unwrap_or_else(|| std::path::PathBuf::from("C:\\"))
        };

        if path_to_search.is_absolute()
            && path_to_search.to_string_lossy().len() == 2
            && path_to_search.to_string_lossy().ends_with(':')
        {
            path_to_search =
                std::path::PathBuf::from(format!("{}\\", path_to_search.to_string_lossy()));
        }

        panel.search_context = Some(SearchContext {
            tab_id: tab_id.clone(),
            query: query.clone(),
            results: Vec::new(),
            is_searching: true,
            cancellation_token: Some(cancellation),
        });

        path_to_search
    };

    info!(
        "Starting advanced search in {:?} for '{}'",
        root_path, query
    );

    if !root_path.exists() {
        return Err(format!("Search root path does not exist: {:?}", root_path));
    }

    // 2. Prep Patterns and Filters
    let is_regex = regex.unwrap_or(false);
    let is_case_sensitive = case_sensitive.unwrap_or(false);
    let should_ignore_accents = ignore_accents.unwrap_or(false);

    let search_pattern = if is_regex {
        let pattern_str = if should_ignore_accents {
            crate::utils::remove_accents(&query)
        } else {
            query.clone()
        };
        let r = RegexBuilder::new(&pattern_str)
            .case_insensitive(!is_case_sensitive)
            .build()
            .map_err(|e| format!("Invalid regex: {}", e))?;
        SearchPattern::Regex(r, should_ignore_accents)
    } else if query.contains('*') || query.contains('?') {
        let pattern_str = if should_ignore_accents {
            crate::utils::remove_accents(&query).to_lowercase()
        } else {
            query.to_lowercase()
        };
        let p = Pattern::new(&pattern_str).map_err(|e| e.to_string())?;
        SearchPattern::Glob(p, should_ignore_accents, false)
    } else {
        SearchPattern::Literal(query.clone(), is_case_sensitive, should_ignore_accents)
    };

    let content_regex_pattern = if let Some(cq) = content_query {
        let is_content_regex = content_regex.unwrap_or(false);
        let pattern = if is_content_regex {
            cq
        } else {
            regex::escape(&cq)
        };
        Some(
            RegexBuilder::new(&pattern)
                .case_insensitive(!is_case_sensitive)
                .build()
                .map_err(|e| format!("Invalid content pattern: {}", e))?,
        )
    } else {
        None
    };

    // 3. Prep Parameters
    let (search_limit, is_turbo, show_hidden, show_system) = {
        let config = config_manager.0.lock().unwrap();
        (
            config.search_limit as usize,
            config.default_turbo_mode,
            config.show_hidden,
            config.show_system,
        )
    };
    let is_recursive = recursive.unwrap_or(true);
    let should_search_archives = search_in_archives.unwrap_or(false);

    let search_params = Arc::new(SearchParams {
        pattern: search_pattern,
        min_size,
        max_size,
        min_date,
        max_date,
    });

    let root_path_for_hardware = root_path.clone();
    let panel_id_clone = panel_id.clone();
    thread::spawn(move || {
        let _config = config_manager; // Move it in
        let is_target_ssd = is_ssd(&root_path_for_hardware);
        let is_network = is_network_path(&root_path_for_hardware);

        if !is_turbo && !is_network {
            platform().set_thread_priority(ThreadPriority::Background);
        }

        let max_depth = if !is_recursive { 1 } else { usize::MAX };
        let num_threads = if is_target_ssd { 16 } else { 2 };

        let filtered_walker = jwalk::WalkDir::new(&root_path)
            .skip_hidden(!show_hidden)
            .max_depth(max_depth)
            .parallelism(jwalk::Parallelism::RayonNewPool(num_threads))
            .process_read_dir(move |_depth, _path, _state, children| {
                if !show_system {
                    children.retain(|res| {
                        if let Ok(entry) = res {
                            if entry.file_type().is_dir() {
                                let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                                if name == "windows"
                                    || name == "programdata"
                                    || name == "$recycle.bin"
                                    || name == "system volume information"
                                    || name == "winsxs"
                                {
                                    return false;
                                }
                            }
                        }
                        true
                    });
                }
            })
            .into_iter();

        let mut total_results = Vec::new();
        let mut files_processed: u64 = 0;

        for entry in filtered_walker.filter_map(|e| e.ok()) {
            if cancel_thread.load(Ordering::Relaxed) {
                break;
            }
            if entry.depth == 0 {
                continue;
            }

            files_processed += 1;
            if !is_turbo && !is_network && files_processed.is_multiple_of(100) {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }

            let path = entry.path();
            let name = match path.file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => {
                    if path == root_path {
                        String::new()
                    } else {
                        path.to_string_lossy().to_string()
                    }
                }
            };

            if name.is_empty() {
                continue;
            }

            let matches_name = search_params.pattern.matches(&name);

            if matches_name {
                if let Ok(metadata) = entry.metadata() {
                    let is_dir = metadata.is_dir();
                    let (is_hidden_attr, is_system_attr, _) =
                        crate::utils::get_file_attributes(&metadata, &name);

                    if !show_hidden && is_hidden_attr {
                        continue;
                    }
                    if !show_system && is_system_attr {
                        continue;
                    }

                    if is_dir
                        && (search_params.min_size.is_some()
                            || search_params.max_size.is_some()
                            || content_regex_pattern.is_some())
                    {
                        continue;
                    }

                    if !is_dir {
                        let size = metadata.len();
                        if let Some(min) = search_params.min_size {
                            if size < min {
                                continue;
                            }
                        }
                        if let Some(max) = search_params.max_size {
                            if size > max {
                                continue;
                            }
                        }
                    }

                    let modified = metadata
                        .modified()
                        .unwrap_or(SystemTime::UNIX_EPOCH)
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    if let Some(min) = search_params.min_date {
                        if modified < min {
                            continue;
                        }
                    }
                    if let Some(max) = search_params.max_date {
                        if modified > max {
                            continue;
                        }
                    }

                    if let Some(ref c_reg) = content_regex_pattern {
                        if is_dir
                            || !file_contains_content(
                                &path,
                                c_reg,
                                should_ignore_accents,
                                is_target_ssd,
                            )
                        {
                            continue;
                        }
                    }

                    total_results.push(FileEntry {
                        name,
                        path: path.to_string_lossy().to_string(),
                        is_dir,
                        is_hidden: is_hidden_attr,
                        is_system: is_system_attr,
                        is_symlink: metadata.file_type().is_symlink(),
                        is_junction: false,
                        size: if is_dir { 0 } else { metadata.len() },
                        is_calculated: false,
                        modified,
                        folders_count: None,
                        files_count: None,
                        is_readonly: metadata.permissions().readonly(),
                        is_protected: false,
                        original_path: None,
                        network_details: None,
                        deleted_time: None,
                    });

                    if total_results.len() >= search_limit {
                        break;
                    }
                }
            }

            if should_search_archives && is_archive(&path) {
                if let Ok(metadata) = entry.metadata() {
                    if !metadata.is_dir() {
                        let internal_results =
                            search_in_archive(&path, &search_params, &cancel_thread);
                        for res in internal_results {
                            total_results.push(res);
                            if total_results.len() >= search_limit {
                                break;
                            }
                        }
                    }
                }
            }

            if total_results.len() >= search_limit {
                break;
            }
        }

        // Save results to session
        if let Ok(mut session) = session_manager.0.lock() {
            let panel = if panel_id_clone == "left" {
                &mut session.left_panel
            } else {
                &mut session.right_panel
            };

            let config = panel.sort_config.clone();
            crate::commands::io::sort_file_entries(&mut total_results, &config);

            if let Some(ctx) = &mut panel.search_context {
                ctx.results = total_results;
                ctx.is_searching = false;
            }
        }

        if !is_turbo {
            platform().set_thread_priority(ThreadPriority::Normal);
        }
    });

    Ok(())
}

pub async fn cancel_search(
    session_manager: &SessionManager,
    panel_id: String,
    tab_id: Option<String>,
) -> Result<(), String> {
    let mut session = session_manager.0.lock().map_err(|e| e.to_string())?;
    let panel = if panel_id == "left" {
        &mut session.left_panel
    } else {
        &mut session.right_panel
    };

    if let Some(ctx) = &mut panel.search_context {
        if let Some(ref tid) = tab_id {
            if ctx.tab_id != *tid {
                return Ok(());
            }
        }

        if let Some(token) = &ctx.cancellation_token {
            token.store(true, Ordering::Relaxed);
        }
        ctx.is_searching = false;
        ctx.results.clear();
    }
    Ok(())
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
