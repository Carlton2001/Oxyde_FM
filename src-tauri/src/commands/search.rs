use crate::models::{FileEntry, SessionManager, ConfigManager};
use crate::models::session::SearchContext;

use log::info;
use glob::Pattern;
use regex::{Regex, RegexBuilder};
use std::time::SystemTime;
use walkdir::{DirEntry, WalkDir};
use tauri::{AppHandle, State, Emitter, Manager};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use crate::utils::archive::{ArchiveFormat, is_archive};
use crate::utils::hardware::{get_physical_disk_id, is_ssd};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use iso9660_core::iso9660entry::{IsISO9660Record, ISO9660Record};

#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{
    GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN, THREAD_MODE_BACKGROUND_END,
};

#[derive(Clone, serde::Serialize)]
struct SearchEvent {
    panel_id: String,
    results: Vec<FileEntry>,
    completed: bool,
}

#[derive(Clone)]
enum SearchPattern {
    Glob(Pattern, bool),    // (pattern, ignore_accents)
    Regex(Regex, bool),   // (regex, ignore_accents)
    Literal(String, bool, bool), // (query, case_sensitive, ignore_accents)
}

static DISK_IO_LOCKS: Lazy<DashMap<u64, Arc<Mutex<()>>>> = Lazy::new(|| DashMap::new());
impl SearchPattern {
    fn matches(&self, text: &str) -> bool {
        match self {
            SearchPattern::Glob(p, ia) => {
                let target = if *ia { crate::utils::remove_accents(text).to_lowercase() } else { text.to_lowercase() };
                p.matches(&target)
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
                    (crate::utils::remove_accents(text), crate::utils::remove_accents(q))
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
                        if cancellation.load(Ordering::Relaxed) { break; }
                        if let Ok(file) = archive.by_index(i) {
                            let name_with_path = file.name().replace('\\', "/");
                            let last_part = name_with_path.split('/').next_back().unwrap_or("");
                            if last_part.is_empty() { continue; }

                            if params.pattern.matches(last_part) {
                                let is_dir = file.is_dir();
                                let size = if is_dir { 0 } else { file.size() };
                                
                                // Basic size filter
                                if !is_dir {
                                    if let Some(min) = params.min_size { if size < min { continue; } }
                                    if let Some(max) = params.max_size { if size > max { continue; } }
                                }

                                let modified = file.last_modified()
                                    .and_then(|dt| {
                                        let t: Result<time::OffsetDateTime, _> = dt.try_into();
                                        t.ok()
                                    })
                                    .map(|ts| ts.unix_timestamp() as u64 * 1000)
                                    .unwrap_or(0);
                                
                                if let Some(min) = params.min_date { if modified < min { continue; } }
                                if let Some(max) = params.max_date { if modified > max { continue; } }

                                results.push(FileEntry {
                                    name: last_part.to_string(),
                                    path: format!("{}\\{}", archive_path.to_string_lossy(), name_with_path.replace('/', "\\")),
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
                if let Ok(len) = file.metadata().map(|m| m.len()) {
                    if let Ok(mut reader) = sevenz_rust::SevenZReader::new(file, len, "".into()) {
                        let _ = reader.for_each_entries(|entry, _| {
                            if cancellation.load(Ordering::Relaxed) { return Ok(false); }
                            let name_with_path = entry.name().replace('\\', "/");
                            let last_part = name_with_path.split('/').next_back().unwrap_or("");
                            if last_part.is_empty() { return Ok(true); }

                            if params.pattern.matches(last_part) {
                                let is_dir = entry.is_directory();
                                let size = entry.size();

                                if !is_dir {
                                    if let Some(min) = params.min_size { if size < min { return Ok(true); } }
                                    if let Some(max) = params.max_size { if size > max { return Ok(true); } }
                                }

                                results.push(FileEntry {
                                    name: last_part.to_string(),
                                    path: format!("{}\\{}", archive_path.to_string_lossy(), name_with_path.replace('/', "\\")),
                                    is_dir,
                                    is_hidden: false,
                                    size,
                                    modified: 0,
                                    ..FileEntry::default()
                                });
                            }
                            Ok(true)
                        });
                    }
                }
            }
        }
        ArchiveFormat::Tar | ArchiveFormat::TarGz | ArchiveFormat::TarXz | ArchiveFormat::TarZst | ArchiveFormat::TarBz2 => {
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
                        if cancellation.load(Ordering::Relaxed) { break; }
                        if let Ok(path) = entry.path() {
                            let name_with_path = path.to_string_lossy().replace('\\', "/");
                            let last_part = name_with_path.split('/').next_back().unwrap_or("");
                            if last_part.is_empty() { continue; }

                            if params.pattern.matches(last_part) {
                                let is_dir = entry.header().entry_type().is_dir();
                                let size = entry.header().size().unwrap_or(0);

                                if !is_dir {
                                    if let Some(min) = params.min_size { if size < min { continue; } }
                                    if let Some(max) = params.max_size { if size > max { continue; } }
                                }

                                let modified = entry.header().mtime().unwrap_or(0) * 1000;
                                if let Some(min) = params.min_date { if modified < min { continue; } }
                                if let Some(max) = params.max_date { if modified > max { continue; } }

                                results.push(FileEntry {
                                    name: last_part.to_string(),
                                    path: format!("{}\\{}", archive_path.to_string_lossy(), name_with_path.replace('/', "\\")),
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
                    search_in_iso(&mut iso, "/", params, cancellation, archive_path, &mut results);
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
    if cancellation.load(Ordering::Relaxed) { return; }
    
    let mut iter = match iso.listdir(internal_path) {
        Ok(it) => it,
        Err(_) => return,
    };
    
    let mut records = Vec::new();
    while let Some(rec) = iter.next(iso) {
        records.push(rec);
    }
    
    for rec in records {
        if cancellation.load(Ordering::Relaxed) { break; }
        
        let name = match &rec {
            ISO9660Record::Directory(d) => d.identifier(),
            ISO9660Record::File(f) => f.identifier(),
        };
        if name == "." || name == ".." { continue; }
        
        let display_name = name.split(';').next().unwrap_or(&name);
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
                if let Some(min) = params.min_size { if size < min { continue; } }
                if let Some(max) = params.max_size { if size > max { continue; } }
            }
            
            // Note: date filtering is skipped for ISO as we don't parse it yet from this crate

            results.push(FileEntry {
                name: display_name.to_string(),
                path: format!("{}\\{}", archive_path.to_string_lossy(), new_internal.trim_start_matches('/').replace('/', "\\")),
                is_dir,
                size,
                ..FileEntry::default()
            });
        }
        
        if matches!(&rec, ISO9660Record::Directory(_)) {
            search_in_iso(iso, &new_internal, params, cancellation, archive_path, results);
        }
    }
}


fn is_binary_file(path: &std::path::Path) -> bool {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return true,
    };

    let mut buffer = [0u8; 1024];
    match file.read(&mut buffer) {
        Ok(n) => {
            // A file containing a NULL byte is almost certainly binary
            buffer[..n].iter().any(|&b| b == 0)
        }
        Err(_) => true,
    }
}

fn file_contains_content(path: &std::path::Path, pattern: &Regex, ignore_accents: bool, ssd_hint: bool) -> bool {
    if is_binary_file(path) {
        return false;
    }

    // Hardware-aware throttling
    if !ssd_hint {
        let vol_id = get_physical_disk_id(path);
        let lock = DISK_IO_LOCKS.entry(vol_id).or_insert_with(|| Arc::new(Mutex::new(()))).clone();
        let _guard = lock.lock().unwrap();
        read_file_and_check(path, pattern, ignore_accents)
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
        } else {
            if pattern.is_match(&line) {
                return true;
            }
        }
    }
    false
}


#[tauri::command]
pub async fn start_search(
    app: AppHandle,
    state: State<'_, SessionManager>,
    config_state: State<'_, ConfigManager>,
    panel_id: String,
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
    search_in_archives: Option<bool>
) -> Result<(), String> {
    let cancellation = Arc::new(AtomicBool::new(false));
    let cancel_thread = cancellation.clone();

    // 1. Setup Session Context
    let root_path = {
        let mut session = state.0.lock().map_err(|e| e.to_string())?;
        let panel = if panel_id == "left" { &mut session.left_panel } else { &mut session.right_panel };
        
        let mut path_to_search = if let Some(root) = search_root {
            std::path::PathBuf::from(root)
        } else {
            panel.tabs.iter()
                .find(|t| t.id == panel.active_tab_id)
                .map(|t| t.path.clone())
                .unwrap_or_else(|| std::path::PathBuf::from("C:\\"))
        };

        if path_to_search.is_absolute() && path_to_search.to_string_lossy().len() == 2 && path_to_search.to_string_lossy().ends_with(':') {
            path_to_search = std::path::PathBuf::from(format!("{}\\", path_to_search.to_string_lossy()));
        }

        panel.search_context = Some(SearchContext {
            query: query.clone(),
            results: Vec::new(),
            is_searching: true,
            cancellation_token: Some(cancellation),
        });
        
        path_to_search
    };

    info!("Starting advanced search in {:?} for '{}'", root_path, query);
    
    if !root_path.exists() {
        return Err(format!("Search root path does not exist: {:?}", root_path));
    }

    // 2. Prep Patterns and Filters
    let is_regex = regex.unwrap_or(false);
    let is_case_sensitive = case_sensitive.unwrap_or(false);
    
    let should_ignore_accents = ignore_accents.unwrap_or(false);
    
    let search_pattern = if is_regex {
        let pattern_str = if should_ignore_accents { crate::utils::remove_accents(&query) } else { query.clone() };
        let r = RegexBuilder::new(&pattern_str)
            .case_insensitive(!is_case_sensitive)
            .build()
            .map_err(|e| format!("Invalid regex: {}", e))?;
        SearchPattern::Regex(r, should_ignore_accents)
    } else if query.contains('*') || query.contains('?') {
        let pattern_str = if should_ignore_accents { crate::utils::remove_accents(&query).to_lowercase() } else { query.to_lowercase() };
        let p = Pattern::new(&pattern_str).map_err(|e| e.to_string())?;
        SearchPattern::Glob(p, should_ignore_accents)
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
        Some(RegexBuilder::new(&pattern)
            .case_insensitive(!is_case_sensitive)
            .build()
            .map_err(|e| format!("Invalid content pattern: {}", e))?)
    } else {
        None
    };

    // 3. Spawn Thread
    let panel_id_clone = panel_id.clone();
    let app_handle = app.clone();
    let (search_limit, is_turbo) = {
        let config = config_state.0.lock().unwrap();
        (config.search_limit as usize, config.default_turbo_mode)
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
    thread::spawn(move || {
        let is_target_ssd = is_ssd(&root_path_for_hardware);
        
        #[cfg(target_os = "windows")]
        if !is_turbo {
            unsafe {
                let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
            }
        }

        let mut walker = WalkDir::new(&root_path);
        if !is_recursive {
            walker = walker.max_depth(1);
        }
        
        let mut total_results = Vec::new();
        let mut batch_start_idx: usize = 0;
        let mut last_emit = std::time::Instant::now();

        let is_hidden_fn = |entry: &DirEntry| -> bool {
            if let Ok(metadata) = entry.metadata() {
                let name = entry.file_name().to_str().unwrap_or("");
                let (hidden, _, _) = crate::utils::get_file_attributes(&metadata, name);
                return hidden;
            }
            false
        };

        let filtered_walker = walker.into_iter().filter_entry(move |e| {
            if e.depth() == 0 { return true; }
            if is_hidden_fn(e) { return false; }
            true
        });

        for entry in filtered_walker.filter_map(|e| e.ok()) {
            if cancel_thread.load(Ordering::Relaxed) { break; }
            
            if !is_turbo {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }

            let path = entry.path();
            let name = match path.file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => if path == root_path { String::new() } else { path.to_string_lossy().to_string() }
            };

            if name.is_empty() { continue; }

            // 1. Name Match
            if search_params.pattern.matches(&name) {
                if let Ok(metadata) = entry.metadata() {
                    let is_dir = metadata.is_dir();
                    
                    if is_dir && (search_params.min_size.is_some() || search_params.max_size.is_some() || content_regex_pattern.is_some()) {
                        continue;
                    }

                    // 2. Size Filter
                    if !is_dir {
                        let size = metadata.len();
                        if let Some(min) = search_params.min_size { if size < min { continue; } }
                        if let Some(max) = search_params.max_size { if size > max { continue; } }
                    }

                    // 3. Date Filter
                    let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH)
                        .duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
                    if let Some(min) = search_params.min_date { if modified < min { continue; } }
                    if let Some(max) = search_params.max_date { if modified > max { continue; } }

                    // 4. Content Filter
                    if let Some(ref c_reg) = content_regex_pattern {
                        if is_dir || !file_contains_content(path, c_reg, should_ignore_accents, is_target_ssd) {
                            continue;
                        }
                    }

                    let (is_hidden_attr, is_system_attr, _) = crate::utils::get_file_attributes(&metadata, &name);
                    
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
                        is_readonly: metadata.permissions().readonly(),
                        original_path: None,
                        deleted_time: None,
                    });

                    if total_results.len() >= search_limit { break; }
                }
            }
            // 5. Archive Search
            if should_search_archives && is_archive(path) {
                if let Ok(metadata) = entry.metadata() {
                    if !metadata.is_dir() {
                        let internal_results = search_in_archive(path, &search_params, &cancel_thread);
                        for res in internal_results {
                            total_results.push(res);
                            if total_results.len() >= search_limit { break; }
                        }
                    }
                }
            }

            if total_results.len() >= search_limit { break; }

            // Emit batch using index slice (no per-item clone)
            let batch_len = total_results.len() - batch_start_idx;
            if (batch_len >= 1000 || last_emit.elapsed().as_millis() > 750) && batch_len > 0 {
                let _ = app_handle.emit("search_event", SearchEvent {
                    panel_id: panel_id_clone.clone(),
                    results: total_results[batch_start_idx..].to_vec(),
                    completed: false
                });
                batch_start_idx = total_results.len();
                last_emit = std::time::Instant::now();
            }
        }

        // Emit remaining unsent results before moving into session
        if batch_start_idx < total_results.len() {
            let _ = app_handle.emit("search_event", SearchEvent {
                panel_id: panel_id_clone.clone(),
                results: total_results[batch_start_idx..].to_vec(),
                completed: false
            });
        }

        // Save results to session
        if let Some(state_manager) = app_handle.try_state::<SessionManager>() {
            if let Ok(mut session) = state_manager.0.lock() {
                let panel = if panel_id_clone == "left" { &mut session.left_panel } else { &mut session.right_panel };
                 
                // Sort using shared function (no duplication)
                let config = panel.sort_config.clone();
                crate::commands::io::sort_file_entries(&mut total_results, &config);

                if let Some(ctx) = &mut panel.search_context {
                    ctx.results = total_results;
                    ctx.is_searching = false;
                }
            }
        }

        // Signal search completion
        #[cfg(target_os = "windows")]
        if !is_turbo {
            unsafe {
                let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END);
            }
        }

        let _ = app_handle.emit("search_event", SearchEvent {
            panel_id: panel_id_clone,
            results: Vec::new(),
            completed: true
        });
    });

    Ok(())
}

#[tauri::command]
pub async fn cancel_search(
    state: State<'_, SessionManager>,
    panel_id: String
) -> Result<(), String> {
    let mut session = state.0.lock().map_err(|e| e.to_string())?;
    let panel = if panel_id == "left" { &mut session.left_panel } else { &mut session.right_panel };

    if let Some(ctx) = &mut panel.search_context {
        if let Some(token) = &ctx.cancellation_token {
            token.store(true, Ordering::Relaxed);
        }
        ctx.is_searching = false;
        ctx.results.clear();
    }
    Ok(())
}
