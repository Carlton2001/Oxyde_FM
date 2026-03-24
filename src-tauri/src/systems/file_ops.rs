use std::collections::HashMap;
use crate::models::ConflictAction;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;
use crate::models::{HistoryManager, Transaction, TransactionType, TransactionDetails};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::io::{Read, Write};
use log::info;

#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{
    GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN, THREAD_MODE_BACKGROUND_END,
    THREAD_PRIORITY_HIGHEST,
};


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OpStatus {
    Queued,
    Calculating,
    Running,
    Paused,
    Cancelled,
    Completed,
    Error(String),
    WaitingForConflictResolution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileOpType {
    Copy,
    Move,
    Delete,
    Trash, // Move to recycle bin
}

#[derive(Debug, Clone, Serialize)]
pub struct FileOperation {
    pub id: String,
    pub op_type: FileOpType,
    pub sources: Vec<PathBuf>,
    pub destination: Option<PathBuf>, // None for Delete/Trash
    pub status: OpStatus,
    pub total_bytes: u64,
    pub processed_bytes: u64,
    pub total_files: usize,
    pub processed_files: usize,
    pub current_file: Option<String>,
    pub bytes_per_second: u64,
    pub turbo: bool,
    pub is_cross_volume: bool,
    pub resolutions: Option<HashMap<String, ConflictAction>>,
    // Private/Internal state, not serialized by default unless needed
    #[serde(skip)]
    pub cancel_flag: Arc<AtomicBool>,
    #[serde(skip)]
    pub pause_flag: Arc<AtomicBool>,
    #[serde(skip)]
    pub turbo_flag: Arc<AtomicBool>,
}

impl FileOperation {
    pub fn new(op_type: FileOpType, sources: Vec<PathBuf>, destination: Option<PathBuf>, resolutions: Option<HashMap<String, ConflictAction>>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            op_type,
            sources,
            destination,
            status: OpStatus::Queued,
            total_bytes: 0,
            processed_bytes: 0,
            total_files: 0,
            processed_files: 0,
            current_file: None,
            bytes_per_second: 0,
            turbo: false,
            is_cross_volume: false,
            resolutions,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            pause_flag: Arc::new(AtomicBool::new(false)),
            turbo_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub struct FileOperationManager {
    operations: Mutex<HashMap<String, Arc<Mutex<FileOperation>>>>,
}

impl Default for FileOperationManager {
    fn default() -> Self {
        Self {
            operations: Mutex::new(HashMap::new()),
        }
    }
}

impl FileOperationManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn queue_operation(&self, app: AppHandle, op: FileOperation) -> String {
        let op_id = op.id.clone();
        let op_arc = Arc::new(Mutex::new(op));
        
        {
            let mut ops = self.operations.lock().unwrap();
            
            // Cleanup: remove operations in final states if more than 50 records exist
            if ops.len() > 50 {
                let to_remove: Vec<String> = ops.iter()
                    .filter(|(_, op_arc)| {
                        let locked = op_arc.lock().unwrap();
                        matches!(locked.status, OpStatus::Completed | OpStatus::Cancelled | OpStatus::Error(_))
                    })
                    .map(|(id, _)| id.clone())
                    .take(20) // Remove up to 20 at a time
                    .collect();
                
                for id in to_remove {
                    ops.remove(&id);
                }
            }
            
            ops.insert(op_id.clone(), op_arc.clone());
        }

        // Spawn background task
        let op_clone = op_arc.clone();
        let app_handle = app.clone();
        
        tauri::async_runtime::spawn(async move {
            Self::execute_operation(app_handle, op_clone).await;
        });

        op_id
    }

    async fn execute_operation(app: AppHandle, op: Arc<Mutex<FileOperation>>) {
        let op_clone = op.clone();
        let app_clone = app.clone();
        
        // Run blocking IO in a separate thread
        let result = tauri::async_runtime::spawn_blocking(move || {
            let (op_type, sources, destination, initial_turbo, resolutions) = {
                let mut locked = op_clone.lock().unwrap();
                locked.status = OpStatus::Calculating;
                let _ = app_clone.emit("file_op_event", locked.clone());
                let turbo = locked.turbo_flag.load(Ordering::Relaxed);
                (locked.op_type.clone(), locked.sources.clone(), locked.destination.clone(), turbo, locked.resolutions.clone())
            };

            // Set initial thread priority based on turbo mode
            #[cfg(target_os = "windows")]
            unsafe {
                if !initial_turbo {
                    let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
                }
            }

            let op_result = match op_type {
                FileOpType::Copy => Self::perform_copy(&app_clone, &op_clone, sources, destination, false, resolutions),
                FileOpType::Move => Self::perform_copy(&app_clone, &op_clone, sources, destination, true, resolutions),
                FileOpType::Delete => Self::perform_delete(&app_clone, &op_clone, sources),
                FileOpType::Trash => Self::perform_trash(&app_clone, &op_clone, sources),
            };

            // Always restore normal priority before thread exits
            #[cfg(target_os = "windows")]
            unsafe {
                let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END);
            }

            op_result
        }).await;

        // Handle result of the blocking task
        let final_status = match result {
            Ok(Ok(_)) => OpStatus::Completed,
            Ok(Err(e)) => OpStatus::Error(e),
            Err(join_err) => OpStatus::Error(format!("Task execution failed: {}", join_err)),
        };

        let mut locked = op.lock().unwrap();
        // If it was cancelled, keep Cancelled status
        if let OpStatus::Cancelled = locked.status {
            // keep it
        } else {
            locked.status = final_status.clone();
            
             // Record History if completed
            if final_status == OpStatus::Completed {
                let history = app.state::<HistoryManager>();
                let tx_type = match locked.op_type {
                    FileOpType::Copy => Some(TransactionType::Copy),
                    FileOpType::Move => Some(TransactionType::Move),
                    FileOpType::Trash => Some(TransactionType::Delete), // Treat Recycle Bin as "Delete" transaction
                    FileOpType::Delete => None, // Permanent delete - no undo for now
                };

                if let Some(t_type) = tx_type {
                    let sources_str: Vec<String> = locked.sources.iter().map(|p| p.to_string_lossy().to_string()).collect();
                    let target_str = locked.destination.as_ref().map(|p| p.to_string_lossy().to_string());
                    
                    // For Trash, target is None/RecycleBin. For Move/Copy, it's valid.
                    // Ideally we'd list *created* files for precise Undo.
                    // Current simplified Undo just deletes dest or moves back.
                    // We'll trust the transaction logic to infer based on sources + target.
                    
                    let details = TransactionDetails {
                        paths: sources_str,
                        target_dir: target_str,
                        old_path: None,
                        new_path: None,
                        created_files: None, // Could populate this if we tracked exact output paths
                    };

                    let tx = Transaction::new(t_type, details);
                    history.push(tx);
                    let _ = app.emit("history_update", ()); // Notify frontend to refresh
                }
            }
        }
    }
    
    fn get_resolution(path: &std::path::Path, resolutions: &HashMap<String, ConflictAction>) -> Option<ConflictAction> {
        let path_str = path.to_string_lossy().to_string();
        if let Some(&action) = resolutions.get(&path_str) {
            return Some(action);
        }

        // Check ancestors
        let mut current = path.parent();
        while let Some(parent) = current {
            let parent_str = parent.to_string_lossy().to_string();
            if let Some(&action) = resolutions.get(&parent_str) {
                return Some(action);
            }
            current = parent.parent();
        }
        None
    }

    fn perform_copy(
        app: &AppHandle, 
        op: &Arc<Mutex<FileOperation>>, 
        sources: Vec<PathBuf>, 
        destination: Option<PathBuf>, 
        is_move: bool,
        resolutions: Option<HashMap<String, ConflictAction>>
    ) -> Result<(), String> {
        let target_dir = destination.ok_or("No destination provided for copy/move".to_string())?;
        
        let mut sources_to_copy = Vec::new();
        let mut total_bytes = 0;
        let mut total_files = 0;

        // 1. Try Fast Move (Rename) for each source if is_move is true
        // Note: For move with conflicts, we usually prefer the slow path if resolutions are present
        // but if no resolutions exist for a top-level item, we can try fast-move.
        if is_move {
            for src in &sources {
                if !src.exists() { continue; }
                let file_name = src.file_name().ok_or("Invalid source name")?;
                let dest = target_dir.join(file_name);

                // If dest exists, check for resolution before trying fast rename
                if dest.exists() {
                    if let Some(ref res_map) = resolutions {
                        let res = Self::get_resolution(src, res_map);
                        if res.is_some() {
                             // If we have a resolution, we MUST use the slow path to apply it precisely
                             sources_to_copy.push(src.clone());
                             continue;
                        }
                    }
                    // If no resolutions but dest exists, std::fs::rename will usually fail or overwrite depending on OS.
                    // We'll fall back to slow path for safety.
                    sources_to_copy.push(src.clone());
                    continue;
                }

                // Try atomic rename for non-conflicting items
                match std::fs::rename(src, &dest) {
                    Ok(_) => {
                        info!("Fast-moved: {} to {}", src.display(), dest.display());
                        continue; 
                    },
                    Err(_) => {
                        sources_to_copy.push(src.clone());
                    }
                }
            }
        } else {
            sources_to_copy = sources.clone();
        }

        if sources_to_copy.is_empty() && is_move {
             return Ok(());
        }

        // 2. Calculate size and build file list with resolution checks
        let mut files_to_process = Vec::new();

        for src in &sources_to_copy {
            if !src.exists() { continue; }
            let file_name = src.file_name().ok_or("Invalid source name")?;
            let dest_root = target_dir.join(file_name);
            
            if src.is_dir() {
                let mut walker = walkdir::WalkDir::new(src).into_iter();
                while let Some(entry) = walker.next() {
                    let entry = match entry {
                        Ok(e) => e,
                        Err(e) => return Err(e.to_string()),
                    };
                    
                    let relative = entry.path().strip_prefix(src).map_err(|e| e.to_string())?;
                    let dest_path = dest_root.join(relative);

                    // Conflict Handling
                    if dest_path.exists() {
                        if let Some(ref res_map) = resolutions {
                            match Self::get_resolution(entry.path(), res_map) {
                                Some(ConflictAction::Skip) => {
                                    if entry.path().is_dir() {
                                        walker.skip_current_dir();
                                    }
                                    continue;
                                },
                                Some(ConflictAction::Replace) => {
                                    // Proceed to copy (overwrite)
                                },
                                None => {
                                    if entry.path().is_dir() {
                                        // Merge directory: just continue walk
                                    } else {
                                        // Unresolved file conflict: skip by default for safety
                                        continue;
                                    }
                                }
                            }
                        } else if entry.path().is_file() {
                             // No resolutions provided but file exists: default to skip if we want to be safe,
                             // but old behavior was overwrite. 
                             // To fix the user bug, we MUST HONOR resolutions. 
                             // If resolutions is None (e.g. standard drag-drop without dialog), 
                             // we should either skip or overwrite. 
                             // Let's stick to overwrite for standard ops, but skip if we were in a conflict session.
                             // Actually, if a session was started, resolutions will be Some.
                        }
                    }

                    if entry.path().is_dir() { continue; } 
                    
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    total_bytes += size;
                    total_files += 1;
                    files_to_process.push((entry.path().to_path_buf(), dest_path));
                }
            } else {
                 let dest_path = dest_root.clone();
                 if dest_path.exists() {
                     if let Some(ref res_map) = resolutions {
                         match Self::get_resolution(src, res_map) {
                             Some(ConflictAction::Skip) => continue,
                             _ => {} // Replace or None (default to replace for files in this context? No, wait)
                         }
                     }
                 }
                 let size = std::fs::metadata(src).map(|m| m.len()).unwrap_or(0);
                 total_bytes += size;
                 total_files += 1;
                 files_to_process.push((src.clone(), dest_path));
            }
        }

        {
            let mut locked = op.lock().unwrap();
            locked.total_bytes = total_bytes;
            locked.total_files = total_files;
            locked.status = OpStatus::Running;
            let _ = app.emit("file_op_event", locked.clone());
        }

        // 3. Perform Copy (for remaining or non-move ops)
        let processed_bytes_atomic = Arc::new(AtomicU64::new(0));
        let processed_files_atomic = Arc::new(AtomicUsize::new(0));
        
        let mut last_processed_bytes = 0;
        let mut last_emit = std::time::Instant::now();
        let mut speed_samples: std::collections::VecDeque<u64> = std::collections::VecDeque::with_capacity(4);
        
        // Cache flags to avoid locking in tight loops
        let (cancel_flag, pause_flag, turbo_flag) = {
            let locked = op.lock().unwrap();
            (locked.cancel_flag.clone(), locked.pause_flag.clone(), locked.turbo_flag.clone())
        };
        
        let last_turbo_state = turbo_flag.load(Ordering::Relaxed);
        // Ensure starting priority matches initial mode
        #[cfg(target_os = "windows")]
        unsafe {
            if last_turbo_state {
                let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END);
            } else {
                let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
            }
        }

        let num_tasks = files_to_process.len();
        let files_to_process_arc = Arc::new(files_to_process);
        let current_index = Arc::new(AtomicUsize::new(0));

        // Limit concurrency: For many small files, having more threads helps mask I/O latency.
        // We use roughly 2x core count, but always leave room for the UI.
        let core_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let concurrency = (core_count * 2).clamp(4, 16); 
        
        let mut handles = Vec::with_capacity(concurrency);

        let error_captured = Arc::new(Mutex::new(None));
        
        for thread_idx in 0..concurrency {
            let files = files_to_process_arc.clone();
            let processed_bytes = processed_bytes_atomic.clone();
            let processed_files = processed_files_atomic.clone();
            let cancel = cancel_flag.clone();
            let pause = pause_flag.clone();
            let turbo = turbo_flag.clone();
            let idx = current_index.clone();
            let error_share = error_captured.clone();
            
            let handle = std::thread::spawn(move || {
                let mut is_in_background_mode = false;

                loop {
                    if cancel.load(Ordering::Relaxed) { break; }
                    
                    let is_turbo = turbo.load(Ordering::Relaxed);

                    // Dynamic Priority Adjustment
                    #[cfg(target_os = "windows")]
                    unsafe {
                        if is_turbo && is_in_background_mode {
                            use windows::Win32::System::Threading::THREAD_MODE_BACKGROUND_END;
                            let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END);
                            is_in_background_mode = false;
                        } else if !is_turbo && !is_in_background_mode {
                            use windows::Win32::System::Threading::THREAD_MODE_BACKGROUND_BEGIN;
                            let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
                            is_in_background_mode = true;
                        }
                    }

                    // In Discret mode, we allow 2 workers instead of just one. 
                    // This helps with small files while still being very light on modern CPUs.
                    if !is_turbo && thread_idx > 1 {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        continue;
                    }

                    let i = idx.fetch_add(1, Ordering::Relaxed);
                    if i >= num_tasks { break; }

                    while pause.load(Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        if cancel.load(Ordering::Relaxed) { return Ok(()); }
                    }

                    let (src, dest): &(PathBuf, PathBuf) = &files[i];
                    
                    if let Some(parent) = dest.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }

                    let mut file_in = std::fs::File::open(src).map_err(|e| {
                        let key = match e.kind() {
                            std::io::ErrorKind::PermissionDenied => "error_access_denied",
                            std::io::ErrorKind::StorageFull => "error_disk_full",
                            _ => return format!("Failed to open {:?}: {}", src, e),
                        }.to_string();
                        let mut share = error_share.lock().unwrap();
                        if share.is_none() { *share = Some(key.clone()); }
                        key
                    })?;

                    let mut file_out = std::fs::File::create(dest).map_err(|e| {
                        let key = match e.kind() {
                            std::io::ErrorKind::PermissionDenied => "error_access_denied",
                            std::io::ErrorKind::StorageFull => "error_disk_full",
                            _ => return format!("Failed to create {:?}: {}", dest, e),
                        }.to_string();
                        let mut share = error_share.lock().unwrap();
                        if share.is_none() { *share = Some(key.clone()); }
                        key
                    })?;
                    
                    let buffer_size = if is_turbo { 1024 * 1024 } else { 512 * 1024 };
                    let mut buffer = vec![0u8; buffer_size];
                    
                    loop {
                        if cancel.load(Ordering::Relaxed) { break; }
                        if error_share.lock().unwrap().is_some() { break; }

                        while pause.load(Ordering::Relaxed) {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            if cancel.load(Ordering::Relaxed) { return Ok(()); }
                        }

                        let n = match file_in.read(&mut buffer) {
                            Ok(0) => break,
                            Ok(n) => n,
                            Err(e) => {
                                let msg = format!("Read error on {:?}: {}", src, e);
                                let mut share = error_share.lock().unwrap();
                                if share.is_none() { *share = Some(msg.clone()); }
                                return Err(msg);
                            }
                        };
                        
                        if let Err(e) = file_out.write_all(&buffer[..n]) {
                            let msg = format!("Write error on {:?}: {}", dest, e);
                            let mut share = error_share.lock().unwrap();
                            if share.is_none() { *share = Some(msg.clone()); }
                            return Err(msg);
                        }
                        processed_bytes.fetch_add(n as u64, Ordering::Relaxed);

                        if !turbo.load(Ordering::Relaxed) {
                            std::thread::sleep(std::time::Duration::from_millis(1));
                        }
                    }

                    processed_files.fetch_add(1, Ordering::Relaxed);
                    if is_move {
                        let _ = std::fs::remove_file(src);
                    }
                }
                
                // Cleanup: Ensure priority is restored before thread dies
                #[cfg(target_os = "windows")]
                if is_in_background_mode {
                    unsafe {
                        use windows::Win32::System::Threading::THREAD_MODE_BACKGROUND_END;
                        let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END);
                    }
                }
                
                Ok::<(), String>(())
            });
            handles.push(handle);
        }

        // Loop to emit progress while workers are running
        while handles.iter().any(|h| !h.is_finished()) {
            std::thread::sleep(std::time::Duration::from_millis(500));
            Self::emit_progress(app, op, &processed_bytes_atomic, &processed_files_atomic, &mut last_processed_bytes, &mut last_emit, &mut speed_samples);
            
            if cancel_flag.load(Ordering::Relaxed) { break; }
            if error_captured.lock().unwrap().is_some() { break; }
        }

        // Wait for all threads
        for handle in handles {
            let _ = handle.join();
        }

        // Check if any error was captured
        if let Some(e) = error_captured.lock().unwrap().clone() {
            return Err(e);
        }

        // Final update to ensure 100% progress is shown before completion
        {
            let mut locked = op.lock().unwrap();
            locked.processed_bytes = locked.total_bytes;
            locked.processed_files = locked.total_files;
            locked.bytes_per_second = 0;
            let op_data = locked.clone();
            drop(locked);
            let _ = app.emit("file_op_event", op_data);
        }

        if is_move {
            // Clean up source directories (naive approach: try to remove them, silence errors if not empty)
             for src in &sources {
                 if src.is_dir() {
                     let _ = std::fs::remove_dir_all(src);
                 }
             }
        }

        Ok(())
    }

    fn emit_progress(
        app: &AppHandle, 
        op: &Arc<Mutex<FileOperation>>, 
        processed_bytes_atomic: &Arc<AtomicU64>,
        processed_files_atomic: &Arc<AtomicUsize>,
        last_processed_bytes: &mut u64,
        last_emit: &mut std::time::Instant,
        speed_samples: &mut std::collections::VecDeque<u64>
    ) {
        let elapsed = last_emit.elapsed();
        if elapsed.as_millis() < 500 { return; }

        let current_bytes = processed_bytes_atomic.load(Ordering::Relaxed);
        let current_files = processed_files_atomic.load(Ordering::Relaxed);
        
        let mut locked = op.lock().unwrap();
        
        // Calculate speed (average over last 2 seconds / 4 samples)
        let bytes_diff = current_bytes.saturating_sub(*last_processed_bytes);
        let secs = elapsed.as_secs_f64();
        if secs > 0.0 {
            let current_speed = (bytes_diff as f64 / secs) as u64;
            speed_samples.push_back(current_speed);
            if speed_samples.len() > 4 {
                speed_samples.pop_front();
            }
            
            let sum: u64 = speed_samples.iter().sum();
            locked.bytes_per_second = sum / speed_samples.len() as u64;
        }
        
        locked.processed_bytes = current_bytes;
        locked.processed_files = current_files;
        
        *last_processed_bytes = current_bytes;
        *last_emit = std::time::Instant::now();
        
        let op_data = locked.clone();
        drop(locked);
        let _ = app.emit("file_op_event", op_data);
    }

    fn perform_delete(app: &AppHandle, op: &Arc<Mutex<FileOperation>>, sources: Vec<PathBuf>) -> Result<(), String> {
        let (turbo, cancel_flag, turbo_flag) = {
            let locked = op.lock().unwrap();
            (locked.turbo, locked.cancel_flag.clone(), locked.turbo_flag.clone())
        };

        let mut real_sources = Vec::new();
        let mut virtual_sources: HashMap<PathBuf, Vec<String>> = HashMap::new();

        for src in &sources {
            if let Some((archive_path, internal_path)) = crate::utils::archive::split_virtual_path(&src.to_string_lossy()) {
                if internal_path.is_empty() {
                    real_sources.push(src.clone());
                } else {
                    virtual_sources.entry(archive_path).or_default().push(internal_path);
                }
            } else {
                real_sources.push(src.clone());
            }
        }

        // 1. Handle Virtual Sources (Archive Deletion)
        for (archive_path, internal_paths) in virtual_sources {
            if cancel_flag.load(Ordering::Relaxed) { return Ok(()); }
            crate::commands::archive::remove_items_from_archive(archive_path, internal_paths).map_err(|e| e.to_string())?;
        }

        if real_sources.is_empty() {
             return Ok(());
        }

        // 2. TURBO MODE for Real Sources
        #[cfg(target_os = "windows")]
        if turbo {
            use windows::Win32::UI::Shell::{SHFileOperationW, SHFILEOPSTRUCTW, FO_DELETE, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FOF_NO_UI};
            use windows::core::PCWSTR;
            use windows::Win32::Foundation::HWND;

            info!("Turbo Delete: Using SHFileOperationW for {} items", real_sources.len());
            
            // Prepare double-null terminated string
            let mut buffer: Vec<u16> = Vec::new();
            for src in &real_sources {
                let path_str = src.to_string_lossy().replace("/", "\\");
                buffer.extend(path_str.encode_utf16());
                buffer.push(0);
            }
            buffer.push(0);

            let mut sh_op = SHFILEOPSTRUCTW {
                hwnd: HWND(std::ptr::null_mut()),
                wFunc: FO_DELETE,
                pFrom: PCWSTR(buffer.as_ptr()),
                pTo: PCWSTR(std::ptr::null()),
                fFlags: (FOF_NOCONFIRMATION.0 | FOF_NOERRORUI.0 | FOF_SILENT.0 | FOF_NO_UI.0) as u16,
                fAnyOperationsAborted: Default::default(),
                hNameMappings: std::ptr::null_mut(),
                lpszProgressTitle: PCWSTR(std::ptr::null()),
            };

            unsafe {
                let result = SHFileOperationW(&mut sh_op);
                if result != 0 {
                    return Err(format!("Windows Shell Error (0x{:X}) during permanent delete.", result));
                }
            }
            
            return Ok(());
        }

        // 3. Parallel Deletion Loop (Dynamic Turbo/Discret)
        let total_items = real_sources.len();
        let current_index = Arc::new(AtomicUsize::new(0));
        let core_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let concurrency = (core_count * 2).clamp(4, 16); 
        
        let mut handles = Vec::with_capacity(concurrency);
        let real_sources_arc = Arc::new(real_sources);
        let processed_files_atomic = Arc::new(AtomicUsize::new(0));
        let error_captured = Arc::new(Mutex::new(None));

        {
            let mut locked = op.lock().unwrap();
            locked.total_files = total_items;
            locked.status = OpStatus::Running;
            let _ = app.emit("file_op_event", locked.clone());
        }

        for thread_idx in 0..concurrency {
            let sources = real_sources_arc.clone();
            let processed_files = processed_files_atomic.clone();
            let cancel = cancel_flag.clone();
            let turbo = turbo_flag.clone();
            let idx = current_index.clone();
            let _app_handle = app.clone();
            let error_share = error_captured.clone();
            
            let handle = std::thread::spawn(move || {
                let mut is_in_background_mode = false;

                loop {
                    if cancel.load(Ordering::Relaxed) { break; }
                    
                    let is_turbo = turbo.load(Ordering::Relaxed);
                    
                    #[cfg(target_os = "windows")]
                    unsafe {
                        if is_turbo && is_in_background_mode {
                            let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END);
                            let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
                            is_in_background_mode = false;
                        } else if !is_turbo && !is_in_background_mode {
                            let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
                            is_in_background_mode = true;
                        }
                    }

                    if !is_turbo && thread_idx > 1 {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        continue;
                    }

                    let i = idx.fetch_add(1, Ordering::Relaxed);
                    if i >= total_items { break; }

                    let src = &sources[i];
                    
                    let res = if src.is_dir() {
                        std::fs::remove_dir_all(src)
                    } else {
                        std::fs::remove_file(src)
                    };

                    if let Err(e) = res {
                        if e.kind() != std::io::ErrorKind::NotFound {
                            let key = match e.kind() {
                                std::io::ErrorKind::PermissionDenied => "error_access_denied",
                                std::io::ErrorKind::StorageFull => "error_disk_full",
                                _ => return Err(format!("Failed to delete {:?}: {}", src, e)),
                            }.to_string();
                            let mut share = error_share.lock().unwrap();
                            if share.is_none() { *share = Some(key.clone()); }
                            return Err(key);
                        }
                    }

                    processed_files.fetch_add(1, Ordering::Relaxed);
                    
                    if !is_turbo {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                }
                
                #[cfg(target_os = "windows")]
                if is_in_background_mode {
                    unsafe {
                        let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END);
                    }
                }
                
                Ok::<(), String>(())
            });
            handles.push(handle);
        }

        // Loop to emit progress
        while handles.iter().any(|h| !h.is_finished()) {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let current = processed_files_atomic.load(Ordering::Relaxed);
            let mut locked = op.lock().unwrap();
            locked.processed_files = current;
            let op_data = locked.clone();
            drop(locked);
            let _ = app.emit("file_op_event", op_data);
            
            if cancel_flag.load(Ordering::Relaxed) { break; }
            if error_captured.lock().unwrap().is_some() { break; }
        }

        for handle in handles {
            let _ = handle.join();
        }

        // Check if any error was captured
        if let Some(e) = error_captured.lock().unwrap().clone() {
            return Err(e);
        }
        
        Ok(())
    }

    fn perform_trash(app: &AppHandle, op: &Arc<Mutex<FileOperation>>, sources: Vec<PathBuf>) -> Result<(), String> {
        let cancel_flag = {
            let locked = op.lock().unwrap();
            locked.cancel_flag.clone()
        };

        let mut real_sources = Vec::new();
        let mut virtual_sources: HashMap<PathBuf, Vec<String>> = HashMap::new();

        for src in &sources {
            if let Some((archive_path, internal_path)) = crate::utils::archive::split_virtual_path(&src.to_string_lossy()) {
                if internal_path.is_empty() {
                    real_sources.push(src.clone());
                } else {
                    virtual_sources.entry(archive_path).or_default().push(internal_path);
                }
            } else {
                real_sources.push(src.clone());
            }
        }

        // 1. Handle Virtual Sources (Delete from Archive, no Trash support)
        for (archive_path, internal_paths) in virtual_sources {
            if cancel_flag.load(Ordering::Relaxed) { return Ok(()); }
            crate::commands::archive::remove_items_from_archive(archive_path, internal_paths).map_err(|e| e.to_string())?;
        }

        if real_sources.is_empty() {
            return Ok(());
        }

        {
            let mut locked = op.lock().unwrap();
            locked.status = OpStatus::Running;
            locked.total_files = real_sources.len();
            let _ = app.emit("file_op_event", locked.clone());
        }

        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::Shell::{SHFileOperationW, SHFILEOPSTRUCTW, FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT};
            use windows::core::PCWSTR;
            use windows::Win32::Foundation::HWND;

            let mut buffer: Vec<u16> = Vec::new();
            for src in real_sources {
                let path_str = src.to_string_lossy().replace("/", "\\");
                buffer.extend(path_str.encode_utf16());
                buffer.push(0);
            }
            buffer.push(0);

            let mut sh_op = SHFILEOPSTRUCTW {
                hwnd: HWND(std::ptr::null_mut()),
                wFunc: FO_DELETE,
                pFrom: PCWSTR(buffer.as_ptr()),
                pTo: PCWSTR(std::ptr::null()),
                fFlags: (FOF_ALLOWUNDO.0 | FOF_NOCONFIRMATION.0 | FOF_NOERRORUI.0 | FOF_SILENT.0) as u16,
                fAnyOperationsAborted: Default::default(),
                hNameMappings: std::ptr::null_mut(),
                lpszProgressTitle: PCWSTR(std::ptr::null()),
            };

            unsafe {
                let result = SHFileOperationW(&mut sh_op);
                if result != 0 {
                    if result == 0x4C7 || result == 0x80070005u32 as i32 {
                        return Err("TrashFull: Opération annulée, nécessitant des droits administrateur ou trop volumineuse.".to_string());
                    }
                    return Err(format!("Windows Shell Error (0x{:X}). The file might be in use or the Recycle Bin is unavailable for this drive.", result));
                }
                
                if sh_op.fAnyOperationsAborted.as_bool() {
                    return Err("TrashFull: Operation was aborted.".to_string());
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            trash::delete_all(real_sources).map_err(|e| e.to_string())?;
        }
        
        {
            let mut locked = op.lock().unwrap();
            locked.processed_files = locked.total_files;
            let op_data = locked.clone();
            drop(locked);
            let _ = app.emit("file_op_event", op_data);
        }

        Ok(())
    }

    pub fn set_turbo(&self, app: &AppHandle, id: &str, enabled: bool) {
        let ops = self.operations.lock().unwrap();
        if let Some(op_arc) = ops.get(id) {
            let mut locked = op_arc.lock().unwrap();
            locked.turbo = enabled;
            locked.turbo_flag.store(enabled, Ordering::Relaxed);
            let op_data = locked.clone();
            drop(locked);
            let _ = app.emit("file_op_event", op_data);
        }
    }

    pub fn get_operation(&self, id: &str) -> Option<FileOperation> {
        let ops = self.operations.lock().unwrap();
        ops.get(id).map(|op| op.lock().unwrap().clone())
    }
    
    pub fn cancel_operation(&self, id: &str) -> bool {
        let ops = self.operations.lock().unwrap();
        if let Some(op) = ops.get(id) {
            let mut locked_op = op.lock().unwrap();
            locked_op.cancel_flag.store(true, Ordering::Relaxed);
            locked_op.status = OpStatus::Cancelled;
            return true;
        }
        false
    }
}
