use crate::models::{CommandError, FileEntry};
use dashmap::DashMap;
use jwalk::WalkDir;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::utils::hardware::{get_physical_disk_id, is_ssd};

#[derive(Clone, Serialize)]
pub struct DuplicatesProgress {
    pub stage: String,
    pub current: usize,
    pub total: usize,
    pub message: String,
}

pub struct DuplicateSearchState(pub Arc<AtomicBool>, pub Arc<Mutex<DuplicatesProgress>>);

impl DuplicateSearchState {
    pub fn new() -> Self {
        Self(
            Arc::new(AtomicBool::new(false)),
            Arc::new(Mutex::new(DuplicatesProgress {
                stage: "Scanning".into(),
                current: 0,
                total: 0,
                message: String::new(),
            })),
        )
    }
}

impl Default for DuplicateSearchState {
    fn default() -> Self {
        Self::new()
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

fn calculate_hash<P: AsRef<Path>>(
    path: P,
    limit: Option<usize>,
    from_end: bool,
    cancellation: &AtomicBool,
) -> Result<blake3::Hash, std::io::Error> {
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
        let mut buffer = [0u8; 64 * 1024];
        loop {
            if cancellation.load(Ordering::Relaxed) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "Search cancelled",
                ));
            }
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
    }

    Ok(hasher.finalize())
}

pub async fn find_duplicates(
    state: &DuplicateSearchState,
    paths: Vec<String>,
    options: DuplicateSearchOptions,
) -> Result<Vec<DuplicateGroup>, CommandError> {
    // Each run owns its token; never reset a token cancelled before the worker starts.
    let progress = state.1.clone();
    let cancel_flag = state.0.clone();

    tokio::task::spawn_blocking(move || {
        if cancel_flag.load(Ordering::Relaxed) {
            return Ok(vec![]);
        }
        if !options.by_name && !options.by_size && !options.by_content {
            return Err(CommandError::IoError(
                "Select at least one duplicate criterion".into(),
            ));
        }
        // Canonical roots eliminate aliases and overlapping selections before scanning.
        let mut roots = Vec::new();
        for path in paths {
            let path = if cfg!(windows) && path.len() == 2 && path.ends_with(':') {
                format!("{path}\\")
            } else {
                path
            };
            let root = std::fs::canonicalize(&path)
                .map_err(|e| CommandError::IoError(format!("{path}: {e}")))?;
            // Keep ordinary volume paths for hardware detection and navigation.
            let text = root.to_string_lossy();
            let root = if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
                PathBuf::from(format!(r"\\{unc}"))
            } else if let Some(local) = text.strip_prefix(r"\\?\") {
                PathBuf::from(local)
            } else {
                root
            };
            if !root.is_dir() {
                return Err(CommandError::IoError(format!("{path}: not a directory")));
            }
            if roots
                .iter()
                .any(|existing: &PathBuf| root.starts_with(existing))
            {
                continue;
            }
            roots.retain(|existing| !existing.starts_with(&root));
            roots.push(root);
        }
        let report = |stage: &str, current: usize, total: usize| {
            if let Ok(mut value) = progress.lock() {
                value.stage = stage.into();
                value.current = current;
                value.total = total;
            }
        };
        report("Scanning", 0, 0);
        // 1. Collect all files and group by selected criteria (Parallel Scan)
        #[derive(PartialEq, Eq, Hash, Debug)]
        struct GroupKey {
            name: Option<String>,
            size: Option<u64>,
        }

        let initial_groups: Arc<Mutex<HashMap<GroupKey, Vec<PathBuf>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let file_count = Arc::new(AtomicUsize::new(0));

        roots.par_iter().for_each(|root_path| {
            if cancel_flag.load(Ordering::Relaxed) {
                return;
            }

            let is_target_ssd = is_ssd(&root_path);
            let num_threads = if is_target_ssd { 16 } else { 2 };

            for entry in WalkDir::new(root_path)
                .parallelism(jwalk::Parallelism::RayonNewPool(num_threads))
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if cancel_flag.load(Ordering::Relaxed) {
                    break;
                }
                if entry.file_type().is_file() {
                    if let Ok(metadata) = entry.metadata() {
                        let size = metadata.len();
                        // If no criteria selected, skip (shouldn't happen with UI)
                        if !options.by_name && !options.by_size && !options.by_content {
                            continue;
                        }

                        let mut key = GroupKey {
                            name: None,
                            size: None,
                        };
                        if options.by_name {
                            key.name = Some(
                                entry
                                    .file_name()
                                    .to_string_lossy()
                                    .to_string()
                                    .to_lowercase(),
                            );
                        }
                        if options.by_size || options.by_content {
                            key.size = Some(size);
                        }

                        let mut groups = initial_groups.lock().unwrap();
                        groups
                            .entry(key)
                            .or_default()
                            .push(entry.path().to_path_buf());
                        let count = file_count.fetch_add(1, Ordering::Relaxed) + 1;
                        if count % 100 == 0 {
                            report("Scanning", count, 0);
                        }
                    }
                }
            }
        });

        report("Scanning", file_count.load(Ordering::Relaxed), 0);
        let initial_groups = Arc::try_unwrap(initial_groups)
            .unwrap()
            .into_inner()
            .unwrap();

        if cancel_flag.load(Ordering::Relaxed) {
            return Ok(vec![]);
        }

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
                    if cancel_flag.load(Ordering::Relaxed) {
                        return Ok(vec![]);
                    }
                    if let Ok(entry) = crate::models::file_entry::get_file_entry_from_path(&p) {
                        files.push(entry);
                    }
                }
                if files.len() > 1 {
                    result_groups.push(DuplicateGroup {
                        size: if options.by_size || options.by_content {
                            size
                        } else {
                            files[0].size
                        },
                        files,
                    });
                }
            }
            for group in &mut result_groups {
                group.files.sort_by(|a, b| a.path.cmp(&b.path));
            }
            result_groups.sort_by(|a, b| {
                b.size
                    .cmp(&a.size)
                    .then_with(|| a.files[0].path.cmp(&b.files[0].path))
            });
            return Ok(result_groups);
        }

        // --- HASHING LOGIC (only if by_content is true) ---

        let total_to_hash = groups_to_process
            .iter()
            .map(|(_, files)| files.len())
            .sum::<usize>();
        if total_to_hash == 0 {
            return Ok(vec![]);
        }

        let mut flat_potential = Vec::with_capacity(total_to_hash);
        for (size, files) in groups_to_process {
            for path in files {
                flat_potential.push((size, path));
            }
        }

        let vol_semaphores: Arc<DashMap<u64, Arc<Mutex<()>>>> = Arc::new(DashMap::new());
        let hashed = AtomicUsize::new(0);
        report("Hashing", 0, total_to_hash);
        let hash_total = total_to_hash;
        let partial_results: Vec<Option<(u64, PathBuf, blake3::Hash)>> = flat_potential
            .into_par_iter()
            .map(|(size, path)| {
                if cancel_flag.load(Ordering::Relaxed) {
                    return None;
                }
                let vol_id = get_physical_disk_id(&path);
                let lock = vol_semaphores
                    .entry(vol_id)
                    .or_insert_with(|| Arc::new(Mutex::new(())))
                    .clone();
                let _guard = lock.lock().unwrap();
                let limit = if size > PARTIAL_HASH_SIZE as u64 {
                    Some(PARTIAL_HASH_SIZE)
                } else {
                    None
                };
                let hash = calculate_hash(&path, limit, false, &cancel_flag).ok()?;
                let count = hashed.fetch_add(1, Ordering::Relaxed) + 1;
                if count % 20 == 0 {
                    report("Hashing", count, hash_total);
                }
                Some((size, path, hash))
            })
            .collect();

        if cancel_flag.load(Ordering::Relaxed) {
            return Ok(vec![]);
        }

        let mut partial_start_groups: HashMap<(u64, blake3::Hash), Vec<PathBuf>> = HashMap::new();
        for res in partial_results.into_iter().flatten() {
            partial_start_groups
                .entry((res.0, res.2))
                .or_default()
                .push(res.1);
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
        if total_end == 0 {
            return Ok(vec![]);
        }

        hashed.store(0, Ordering::Relaxed);
        let hash_total = total_end;
        report("Hashing", 0, hash_total);
        let partial_end_results: Vec<Option<(u64, PathBuf, blake3::Hash)>> = end_check_list
            .into_par_iter()
            .map(|(size, path)| {
                if cancel_flag.load(Ordering::Relaxed) {
                    return None;
                }
                let vol_id = get_physical_disk_id(&path);
                let lock = vol_semaphores
                    .entry(vol_id)
                    .or_insert_with(|| Arc::new(Mutex::new(())))
                    .clone();
                let _guard = lock.lock().unwrap();
                let limit = if size > PARTIAL_HASH_SIZE as u64 {
                    Some(PARTIAL_HASH_SIZE)
                } else {
                    None
                };
                let hash = calculate_hash(&path, limit, true, &cancel_flag).ok()?;
                let count = hashed.fetch_add(1, Ordering::Relaxed) + 1;
                if count % 20 == 0 {
                    report("Hashing", count, hash_total);
                }
                Some((size, path, hash))
            })
            .collect();

        if cancel_flag.load(Ordering::Relaxed) {
            return Ok(vec![]);
        }

        let mut partial_end_groups: HashMap<(u64, blake3::Hash), Vec<PathBuf>> = HashMap::new();
        for res in partial_end_results.into_iter().flatten() {
            partial_end_groups
                .entry((res.0, res.2))
                .or_default()
                .push(res.1);
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
        if total_final == 0 {
            return Ok(vec![]);
        }

        hashed.store(0, Ordering::Relaxed);
        let hash_total = total_final;
        report("Hashing", 0, hash_total);
        let full_results: Vec<Option<(u64, PathBuf, blake3::Hash)>> = final_check_list
            .into_par_iter()
            .map(|(size, path)| {
                if cancel_flag.load(Ordering::Relaxed) {
                    return None;
                }
                let vol_id = get_physical_disk_id(&path);
                let lock = vol_semaphores
                    .entry(vol_id)
                    .or_insert_with(|| Arc::new(Mutex::new(())))
                    .clone();
                let _guard = lock.lock().unwrap();
                let hash = calculate_hash(&path, None, false, &cancel_flag).ok()?;
                let count = hashed.fetch_add(1, Ordering::Relaxed) + 1;
                if count % 20 == 0 {
                    report("Hashing", count, hash_total);
                }
                Some((size, path, hash))
            })
            .collect();

        if cancel_flag.load(Ordering::Relaxed) {
            return Ok(vec![]);
        }

        let mut final_groups: HashMap<(u64, blake3::Hash, Option<String>), Vec<PathBuf>> =
            HashMap::new();
        for res in full_results.into_iter().flatten() {
            let name = options.by_name.then(|| {
                res.1
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase()
            });
            final_groups
                .entry((res.0, res.2, name))
                .or_default()
                .push(res.1);
        }

        let mut result_groups = Vec::new();
        for ((size, _, _), paths) in final_groups {
            if paths.len() > 1 {
                let mut files = Vec::new();
                for p in paths {
                    if let Ok(entry) = crate::models::file_entry::get_file_entry_from_path(&p) {
                        files.push(entry);
                    }
                }
                if files.len() > 1 {
                    result_groups.push(DuplicateGroup {
                        size: if options.by_size || options.by_content {
                            size
                        } else {
                            files[0].size
                        },
                        files,
                    });
                }
            }
        }

        for group in &mut result_groups {
            group.files.sort_by(|a, b| a.path.cmp(&b.path));
        }
        result_groups.sort_by(|a, b| {
            b.size
                .cmp(&a.size)
                .then_with(|| a.files[0].path.cmp(&b.files[0].path))
        });
        Ok(result_groups)
    })
    .await
    .map_err(|e| CommandError::IoError(format!("Task execution failed: {}", e)))?
}

pub fn cancel_find_duplicates(state: &DuplicateSearchState) {
    state.0.store(true, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search(
        paths: Vec<String>,
        name: bool,
        size: bool,
        content: bool,
        cancelled: bool,
    ) -> Vec<DuplicateGroup> {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let state = DuplicateSearchState::new();
        if cancelled {
            cancel_find_duplicates(&state);
        }
        runtime
            .block_on(find_duplicates(
                &state,
                paths,
                DuplicateSearchOptions {
                    by_name: name,
                    by_size: size,
                    by_content: content,
                },
            ))
            .unwrap()
    }

    #[test]
    fn overlapping_roots_do_not_duplicate_a_single_file() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("only.txt"), b"unique").unwrap();
        assert!(search(
            vec![
                nested.to_string_lossy().into_owned(),
                dir.path().to_string_lossy().into_owned()
            ],
            false,
            true,
            true,
            false
        )
        .is_empty());
    }

    #[test]
    fn content_and_name_criteria_are_combined() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("nested")).unwrap();
        std::fs::write(dir.path().join("same.txt"), b"identical").unwrap();
        std::fs::write(dir.path().join("nested/same.txt"), b"identical").unwrap();
        std::fs::write(dir.path().join("other.txt"), b"identical").unwrap();
        let paths = vec![dir.path().to_string_lossy().into_owned()];
        let groups = search(paths.clone(), true, true, true, false);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
        assert_eq!(search(paths, false, true, true, false)[0].files.len(), 3);
    }

    #[test]
    fn full_hash_rejects_matching_headers_and_footers() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = vec![0u8; 16384];
        std::fs::write(dir.path().join("a.bin"), &a).unwrap();
        a[8192] = 1;
        std::fs::write(dir.path().join("b.bin"), &a).unwrap();
        let paths = vec![dir.path().to_string_lossy().into_owned()];
        assert_eq!(search(paths.clone(), false, true, false, false).len(), 1);
        assert!(search(paths, false, true, true, false).is_empty());
    }

    #[test]
    fn cancellation_before_worker_start_is_preserved() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a"), b"same").unwrap();
        std::fs::write(dir.path().join("b"), b"same").unwrap();
        assert!(search(
            vec![dir.path().to_string_lossy().into_owned()],
            false,
            true,
            true,
            true
        )
        .is_empty());
    }

    #[test]
    fn content_search_includes_empty_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a"), b"").unwrap();
        std::fs::write(dir.path().join("b"), b"").unwrap();
        let groups = search(
            vec![dir.path().to_string_lossy().into_owned()],
            false,
            false,
            true,
            false,
        );
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
        assert_eq!(groups[0].size, 0);
    }
}
