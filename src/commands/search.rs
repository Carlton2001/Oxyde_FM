use crate::models::FileEntry;

use crate::utils::archive::{is_archive, ArchiveFormat};
use crate::utils::hardware::get_physical_disk_id;
use dashmap::DashMap;
use glob::Pattern;
use iso9660_core::iso9660entry::{ISO9660Record, IsISO9660Record};
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
