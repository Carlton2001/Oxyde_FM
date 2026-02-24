use crate::models::{CommandError, FileEntry};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;
use tauri::{State, AppHandle, Emitter};
use dashmap::DashMap;

use crate::utils::hardware::get_physical_disk_id;

#[derive(Clone, Serialize)]
pub struct DuplicatesProgress {
    pub stage: String,
    pub current: usize,
    pub total: usize,
    pub message: String,
}

pub struct DuplicateSearchState(pub Arc<AtomicBool>);

impl DuplicateSearchState {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
}

#[derive(Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub size: u64,
    pub files: Vec<FileEntry>,
}

#[derive(Deserialize)]
pub struct DuplicateSearchOptions {
    pub by_name: bool,
    pub by_size: bool,
    pub by_content: bool,
}

const PARTIAL_HASH_SIZE: usize = 4096;

// get_physical_disk_id is used from crate::utils::hardware

fn calculate_hash<P: AsRef<Path>>(path: P, limit: Option<usize>, from_end: bool) -> Result<blake3::Hash, std::io::Error> {
    let mut file = File::open(path.as_ref())?;
    let mut hasher = blake3::Hasher::new();
    
    if let Some(limit) = limit {
        if from_end {
            let metadata = file.metadata()?;
            let size = metadata.len();
            if size > limit as u64 {
                use std::io::Seek;
                file.seek(std::io::SeekFrom::End(-(limit as i64)))?;
            }
        }
        
        let mut reader = BufReader::new(file);
        let mut buffer = vec![0; limit];
        let bytes_read = reader.read(&mut buffer)?;
        hasher.update(&buffer[..bytes_read]);
    } else {
        let mut reader = BufReader::new(file);
        std::io::copy(&mut reader, &mut hasher)?;
    }
    
    Ok(hasher.finalize())
}

#[tauri::command]
pub async fn find_duplicates(
    app: AppHandle,
    state: State<'_, DuplicateSearchState>,
    paths: Vec<String>,
    options: DuplicateSearchOptions,
) -> Result<Vec<DuplicateGroup>, CommandError> {
    state.0.store(false, Ordering::Relaxed);
    let cancel_flag = state.0.clone();
    
    tokio::task::spawn_blocking(move || {
        let emit_progress = |stage: &str, current: usize, total: usize, message: &str| {
            let _ = app.emit("duplicates_progress", DuplicatesProgress {
                stage: stage.to_string(),
                current,
                total,
                message: message.to_string(),
            });
        };

        emit_progress("Scanning", 0, 0, "Initializing...");
        
        // 1. Collect all files and group by selected criteria (Parallel Scan)
        #[derive(PartialEq, Eq, Hash, Debug)]
        struct GroupKey {
            name: Option<String>,
            size: Option<u64>,
        }

        let initial_groups: Arc<Mutex<HashMap<GroupKey, Vec<PathBuf>>>> = Arc::new(Mutex::new(HashMap::new()));
        let file_count = Arc::new(AtomicUsize::new(0));

        paths.par_iter().for_each(|path_str| {
            if cancel_flag.load(Ordering::Relaxed) { return; }
            
            let mut root_path = PathBuf::from(path_str);
            if cfg!(windows) && root_path.to_string_lossy().len() == 2 && root_path.to_string_lossy().ends_with(':') {
                root_path = PathBuf::from(format!("{}\\", root_path.to_string_lossy()));
            }

            if !root_path.exists() { return; }
            
            for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
                if cancel_flag.load(Ordering::Relaxed) { break; }
                if entry.file_type().is_file() {
                    if let Ok(metadata) = entry.metadata() {
                        let size = metadata.len();
                        // If no criteria selected, skip (shouldn't happen with UI)
                        if !options.by_name && !options.by_size && !options.by_content { continue; }

                        let mut key = GroupKey { name: None, size: None };
                        if options.by_name {
                            key.name = Some(entry.file_name().to_string_lossy().to_string().to_lowercase());
                        }
                        if options.by_size || options.by_content {
                            key.size = Some(size);
                        }

                        let mut groups = initial_groups.lock().unwrap();
                        groups.entry(key).or_default().push(entry.path().to_path_buf());
                        let count = file_count.fetch_add(1, Ordering::Relaxed) + 1;
                        if count % 5000 == 0 {
                            drop(groups); // Don't hold lock during emit
                            emit_progress("Scanning", count, 0, &format!("Found {} files...", count));
                        }
                    }
                }
            }
        });

        let initial_groups = Arc::try_unwrap(initial_groups).unwrap().into_inner().unwrap();
        let total_scanned = file_count.load(Ordering::Relaxed);
        emit_progress("Scanning", total_scanned, 0, "Scan complete.");
        
        if cancel_flag.load(Ordering::Relaxed) { return Ok(vec![]); }

        // Filter groups with at least 2 files
        let groups_to_process: Vec<(u64, Vec<PathBuf>)> = initial_groups
            .into_iter()
            .filter(|(_, files)| files.len() > 1)
            .map(|(key, files)| (key.size.unwrap_or(0), files))
            .collect();

        // If not checking content, we are done
        if !options.by_content {
            let mut result_groups = Vec::new();
            for (size, paths) in groups_to_process {
                let mut files = Vec::new();
                for p in paths {
                    if let Ok(entry) = crate::models::file_entry::get_file_entry_from_path(&p) {
                        files.push(entry);
                    }
                }
                if files.len() > 1 {
                    result_groups.push(DuplicateGroup { size, files });
                }
            }
            result_groups.sort_by(|a, b| b.size.cmp(&a.size));
            return Ok(result_groups);
        }

        // --- HASHING LOGIC (only if by_content is true) ---
            
        let total_to_hash = groups_to_process.iter().map(|(_, files)| files.len()).sum::<usize>();
        if total_to_hash == 0 {
            return Ok(vec![]);
        }

        emit_progress("Partial Hashing", 0, total_to_hash, "Preparing...");

        let mut flat_potential = Vec::with_capacity(total_to_hash);
        for (size, files) in groups_to_process {
            for path in files {
                flat_potential.push((size, path));
            }
        }

        let vol_semaphores: Arc<DashMap<u64, Arc<Mutex<()>>>> = Arc::new(DashMap::new());
        let processed_count = Arc::new(AtomicUsize::new(0));
        let partial_results: Vec<Option<(u64, PathBuf, blake3::Hash)>> = flat_potential
            .into_par_iter()
            .map(|(size, path)| {
                if cancel_flag.load(Ordering::Relaxed) { return None; }
                let vol_id = get_physical_disk_id(&path);
                let lock = vol_semaphores.entry(vol_id).or_insert_with(|| Arc::new(Mutex::new(()))).clone();
                let _guard = lock.lock().unwrap();
                let limit = if size > PARTIAL_HASH_SIZE as u64 { Some(PARTIAL_HASH_SIZE) } else { None };
                let hash = calculate_hash(&path, limit, false).ok()?;
                let p = processed_count.fetch_add(1, Ordering::Relaxed) + 1;
                if p % 500 == 0 || p == total_to_hash {
                    emit_progress("Partial Hashing (Start)", p, total_to_hash, &path.file_name().unwrap_or_default().to_string_lossy());
                }
                Some((size, path, hash))
            })
            .collect();

        if cancel_flag.load(Ordering::Relaxed) { return Ok(vec![]); }

        let mut partial_start_groups: HashMap<(u64, blake3::Hash), Vec<PathBuf>> = HashMap::new();
        for res in partial_results.into_iter().flatten() {
            partial_start_groups.entry((res.0, res.2)).or_default().push(res.1);
        }

        let mut end_check_list = Vec::new();
        for ((size, _), files) in partial_start_groups {
            if files.len() > 1 {
                for path in files {
                    end_check_list.push((size, path));
                }
            }
        }

        let total_end = end_check_list.len();
        if total_end == 0 { return Ok(vec![]); }

        emit_progress("Partial Hashing (End)", 0, total_end, "Verifying file footers...");

        let processed_end = Arc::new(AtomicUsize::new(0));
        let partial_end_results: Vec<Option<(u64, PathBuf, blake3::Hash)>> = end_check_list
            .into_par_iter()
            .map(|(size, path)| {
                if cancel_flag.load(Ordering::Relaxed) { return None; }
                let vol_id = get_physical_disk_id(&path);
                let lock = vol_semaphores.entry(vol_id).or_insert_with(|| Arc::new(Mutex::new(()))).clone();
                let _guard = lock.lock().unwrap();
                let limit = if size > PARTIAL_HASH_SIZE as u64 { Some(PARTIAL_HASH_SIZE) } else { None };
                let hash = calculate_hash(&path, limit, true).ok()?;
                let p = processed_end.fetch_add(1, Ordering::Relaxed) + 1;
                if p % 500 == 0 || p == total_end {
                    emit_progress("Partial Hashing (End)", p, total_end, &path.file_name().unwrap_or_default().to_string_lossy());
                }
                Some((size, path, hash))
            })
            .collect();

        if cancel_flag.load(Ordering::Relaxed) { return Ok(vec![]); }

        let mut partial_end_groups: HashMap<(u64, blake3::Hash), Vec<PathBuf>> = HashMap::new();
        for res in partial_end_results.into_iter().flatten() {
            partial_end_groups.entry((res.0, res.2)).or_default().push(res.1);
        }

        let mut final_check_list = Vec::new();
        for ((size, _), files) in partial_end_groups {
            if files.len() > 1 {
                for path in files {
                    final_check_list.push((size, path));
                }
            }
        }

        let total_final = final_check_list.len();
        if total_final == 0 { return Ok(vec![]); }

        emit_progress("Full Hashing", 0, total_final, "Comparing large files...");

        let processed_final = Arc::new(AtomicUsize::new(0));
        let full_results: Vec<Option<(u64, PathBuf, blake3::Hash)>> = final_check_list
            .into_par_iter()
            .map(|(size, path)| {
                if cancel_flag.load(Ordering::Relaxed) { return None; }
                let vol_id = get_physical_disk_id(&path);
                let lock = vol_semaphores.entry(vol_id).or_insert_with(|| Arc::new(Mutex::new(()))).clone();
                let _guard = lock.lock().unwrap();
                let hash = calculate_hash(&path, None, false).ok()?;
                let p = processed_final.fetch_add(1, Ordering::Relaxed) + 1;
                if p % 100 == 0 || p == total_final {
                    emit_progress("Full Hashing", p, total_final, &path.file_name().unwrap_or_default().to_string_lossy());
                }
                Some((size, path, hash))
            })
            .collect();

        if cancel_flag.load(Ordering::Relaxed) { return Ok(vec![]); }

        let mut final_groups: HashMap<(u64, blake3::Hash), Vec<PathBuf>> = HashMap::new();
        for res in full_results.into_iter().flatten() {
            final_groups.entry((res.0, res.2)).or_default().push(res.1);
        }

        let mut result_groups = Vec::new();
        for ((size, _), paths) in final_groups {
            if paths.len() > 1 {
                let mut files = Vec::new();
                for p in paths {
                    if let Ok(entry) = crate::models::file_entry::get_file_entry_from_path(&p) {
                        files.push(entry);
                    }
                }
                if files.len() > 1 {
                    result_groups.push(DuplicateGroup { size, files });
                }
            }
        }

        result_groups.sort_by(|a, b| b.size.cmp(&a.size));
        Ok(result_groups)
    }).await.map_err(|e| CommandError::IoError(format!("Task execution failed: {}", e)))?
}

#[tauri::command]
pub fn cancel_find_duplicates(state: State<'_, DuplicateSearchState>) {
    state.0.store(true, Ordering::Relaxed);
}
