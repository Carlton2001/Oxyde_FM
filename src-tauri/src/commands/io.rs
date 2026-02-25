use crate::models::{
    FileEntry, FileProperties, FileSummary, FolderSizeResult, CommandError, Transaction, TransactionType, TransactionDetails, HistoryManager
};
use tauri::Manager;
use crate::utils::path_security::validate_path;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use tauri::{AppHandle, Emitter};
use log::info;
use serde::Serialize;

#[derive(Serialize)]
pub struct DirResponse {
    pub entries: Vec<FileEntry>,
    pub summary: FileSummary,
    pub is_complete: bool,
}

#[derive(Serialize, Clone)]
pub struct DirBatchEvent {
    pub panel_id: String,
    pub path: String,
    pub entries: Vec<FileEntry>,
    pub is_complete: bool,
}

pub fn get_file_entry_from_metadata(metadata: &fs::Metadata, name: &str, path: &std::path::Path) -> FileEntry {
    let modified = metadata.modified()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let (is_hidden, is_system, is_reparse_point) = crate::utils::get_file_attributes(metadata, name);
    let is_readonly = metadata.permissions().readonly();
    let is_dir = metadata.is_dir(); 
    let is_symlink = metadata.file_type().is_symlink();
    let is_junction = is_reparse_point && is_dir && !is_symlink;
    let size = if is_dir { 0 } else { metadata.len() };

    FileEntry {
        name: name.to_string(),
        path: path.to_string_lossy().to_string(),
        is_dir,
        is_hidden,
        is_system,
        is_symlink,
        is_junction,
        size,
        modified,
        is_readonly,
        is_protected: false,
        is_calculated: false,
        original_path: None,
        deleted_time: None,
    }
}

#[tauri::command]
pub async fn list_dir(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::models::SessionManager>,
    panel_id: String,
    path: String,
    sort_config: Option<crate::models::session::SortConfig>,
    show_hidden: Option<bool>,
    show_system: Option<bool>,
    force_refresh: Option<bool>
) -> Result<DirResponse, CommandError> {
    let show_hidden = show_hidden.unwrap_or(false);
    let show_system = show_system.unwrap_or(false);
    let sort_config = sort_config.unwrap_or_default();

    // 1. Check Cache
    let cached_all_entries = {
        if force_refresh.unwrap_or(false) {
            None
        } else {
            let session = state.0.lock().unwrap();
            let panel = if panel_id == "right" { &session.right_panel } else { &session.left_panel };
            
            if let Some(cached) = &panel.cached_results {
                if cached.path.to_string_lossy() == path {
                    // 1. Perfect match (path + config + filters)
                    if cached.config == sort_config && cached.show_hidden == show_hidden && cached.show_system == show_system {
                        return Ok(DirResponse {
                            entries: cached.entries.clone(),
                            summary: cached.summary.clone(),
                            is_complete: true,
                        });
                    }
                    
                    // 2. Path match and filters match, but sort changed -> Re-sort cached entries
                    if cached.show_hidden == show_hidden && cached.show_system == show_system {
                        Some((cached.entries.clone(), cached.summary.clone()))
                    } else {
                        // Filters changed (hidden/system) -> Must re-read from disk to be accurate
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
    }
    };

    let (mut all_entries, _summary) = if let Some(cached) = cached_all_entries {
        cached
    } else {
        // 2. Cache miss -> Read directory
        if let Some((archive_path, internal_path)) = crate::commands::archive::split_virtual_path(&path) {
            let entries = crate::commands::archive::list_archive_contents(
                archive_path.to_string_lossy().to_string(),
                internal_path
            )?;
            let summary = calculate_summary(&entries, Some(path.clone()));
            return Ok(DirResponse { entries, summary, is_complete: true });
        }

        let dir_path = validate_path(&path)?;
        let read_dir = fs::read_dir(&dir_path)?;

        let mut entries = Vec::with_capacity(2048);
        for entry in read_dir.flatten() {
            if let Ok(metadata) = entry.metadata() {
                let name = entry.file_name().to_string_lossy().to_string();
                let path = entry.path();
                let mut file_entry = get_file_entry_from_metadata(&metadata, &name, &path);
                
                // If it's a directory, check if it's protected (Access Denied)
                if file_entry.is_dir {
                    // Try to peek into the directory. If it fails with permission error, it's protected.
                    // We don't use read_dir fully, just check if it's possible.
                    if let Err(e) = fs::read_dir(&path) {
                         let kind = e.kind();
                         if kind == std::io::ErrorKind::PermissionDenied || kind == std::io::ErrorKind::NotFound {
                             file_entry.is_protected = true;
                         }
                    }
                }
                
                entries.push(file_entry);
            }
        }
        (entries, calculate_summary(&[], None)) // temporary summary, will be replaced
    };

    // Filter in-place (no clone needed)
    all_entries.retain(|e| {
        if e.is_system { return show_system; }
        if e.is_hidden { return show_hidden; }
        true
    });

    let summary = calculate_summary(&all_entries, Some(path.clone()));
    sort_file_entries(&mut all_entries, &sort_config);

    // Update cache (one clone here is unavoidable: cache needs its own copy)
    {
        let mut session = state.0.lock().unwrap();
        let panel = if panel_id == "right" { &mut session.right_panel } else { &mut session.left_panel };
        
        // CRITICAL: Clear search context when entering a normal directory to free RAM
        if let Some(mut ctx) = panel.search_context.take() {
            ctx.results.clear();
            ctx.results.shrink_to_fit();
        }

        panel.cached_results = Some(crate::models::session::CachedResults {
            path: PathBuf::from(&path),
            entries: all_entries.clone(),
            summary: summary.clone(),
            config: sort_config,
            show_hidden,
            show_system,
        });
    }

    // Progressive loading
    let total_visible = all_entries.len();
    let initial_count = 800;

    if total_visible <= initial_count {
        Ok(DirResponse {
            entries: all_entries,
            summary,
            is_complete: true,
        })
    } else {
        // Split: keep initial, spawn remaining
        let remaining_entries = all_entries.split_off(initial_count);
        
        let app_stream = app.clone();
        let panel_id_stream = panel_id.clone();
        let path_stream = path.clone();
        
        tauri::async_runtime::spawn(async move {
            let batch_size = 2000;
            let total_remaining = remaining_entries.len();
            
            for (i, chunk) in remaining_entries.chunks(batch_size).enumerate() {
                if i == 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
                
                let is_last = (i * batch_size) + chunk.len() >= total_remaining;
                
                let _ = app_stream.emit("dir_batch", DirBatchEvent {
                    panel_id: panel_id_stream.clone(),
                    path: path_stream.clone(),
                    entries: chunk.to_vec(),
                    is_complete: is_last,
                });
            }
        });

        Ok(DirResponse {
            entries: all_entries, // now contains only the initial batch
            summary,
            is_complete: false,
        })
    }
}

pub fn sort_file_entries(entries: &mut [FileEntry], config: &crate::models::session::SortConfig) {
    use crate::models::session::SortField;
    use crate::models::session::SortDirection;

    entries.sort_unstable_by(|a, b| {
        // Folders-first by default.
        // Exception: when sorting by size, we mix them ONLY IF the folders involved have been calculated.
        if config.field == SortField::Size {
            let a_uncalc = a.is_dir && !a.is_calculated;
            let b_uncalc = b.is_dir && !b.is_calculated;
            if (a_uncalc || b_uncalc) && a.is_dir != b.is_dir {
                return b.is_dir.cmp(&a.is_dir);
            }
        } else if a.is_dir != b.is_dir {
            return b.is_dir.cmp(&a.is_dir);
        }

        let cmp = match config.field {
            SortField::Name => crate::utils::compare_natural(&a.name, &b.name),
            SortField::Size => a.size.cmp(&b.size).then_with(|| crate::utils::compare_natural(&a.name, &b.name)),
            SortField::Date => a.modified.cmp(&b.modified).then_with(|| crate::utils::compare_natural(&a.name, &b.name)),
            SortField::Type => {
                let ext_a = std::path::Path::new(&a.name).extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                let ext_b = std::path::Path::new(&b.name).extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                ext_a.cmp(&ext_b).then_with(|| crate::utils::compare_natural(&a.name, &b.name))
            },
            SortField::Location => a.path.to_lowercase().cmp(&b.path.to_lowercase()).then_with(|| crate::utils::compare_natural(&a.name, &b.name)),
            SortField::DeletedDate => a.deleted_time.cmp(&b.deleted_time).then_with(|| crate::utils::compare_natural(&a.name, &b.name)),
        };

        match config.direction {
            SortDirection::Asc => cmp,
            SortDirection::Desc => cmp.reverse(),
        }
    });
}

fn calculate_summary(entries: &[FileEntry], parent_path: Option<String>) -> FileSummary {
    let mut total_size = 0;
    let mut files_count = 0;
    let mut folders_count = 0;
    let mut all_readonly = true;
    let mut any_readonly = false;
    let mut all_hidden = true;
    let mut any_hidden = false;

    for e in entries.iter() {
        all_readonly &= e.is_readonly;
        all_hidden &= e.is_hidden;
        any_readonly |= e.is_readonly;
        any_hidden |= e.is_hidden;

        if e.is_dir {
            folders_count += 1;
        } else {
            files_count += 1;
            total_size += e.size;
        }
    }

    if entries.is_empty() {
        all_readonly = false;
        all_hidden = false;
    }

    FileSummary {
        count: entries.len(),
        total_size,
        files_count,
        folders_count,
        all_readonly,
        any_readonly,
        all_hidden,
        any_hidden,
        parent_path,
    }
}

#[tauri::command]
pub async fn create_dir(app: AppHandle, path: String) -> Result<(), CommandError> {
    let pb = validate_path(&path)?;
    let p_abs = pb.to_string_lossy().to_string();
    info!("Creating directory: {:?}", pb);
    fs::create_dir_all(&pb)?;
    
    let tx_details = TransactionDetails {
        paths: vec![p_abs],
        target_dir: None,
        old_path: None,
        new_path: None,
        created_files: None,
    };
    let history = app.state::<HistoryManager>();
    history.push(Transaction::new(TransactionType::NewFolder, tx_details));

    Ok(())
}

#[tauri::command]
pub async fn rename_item(app: AppHandle, old_path: String, new_path: String) -> Result<(), CommandError> {
    let old_pb = validate_path(&old_path)?;
    let new_pb = validate_path(&new_path)?;
    let old_abs = old_pb.to_string_lossy().to_string();
    let new_abs = new_pb.to_string_lossy().to_string();
    
    info!("Renaming {:?} to {:?}", old_pb, new_pb);
    fs::rename(old_pb, new_pb)?;
    
    let tx_details = TransactionDetails {
        paths: vec![],
        target_dir: None,
        old_path: Some(old_abs),
        new_path: Some(new_abs),
        created_files: None,
    };
    let history = app.state::<HistoryManager>();
    history.push(Transaction::new(TransactionType::Rename, tx_details));
    
    Ok(())
}



#[tauri::command]
pub fn get_file_properties(path: String) -> Result<FileProperties, CommandError> {
    let path_buf = validate_path(&path)?;
    let metadata = fs::metadata(&path_buf)?;

    let name = path_buf
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let parent = path_buf
        .parent()
        .map(|p: &std::path::Path| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let created = metadata
        .created()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let modified = metadata
        .modified()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let accessed = metadata
        .accessed()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let readonly = metadata.permissions().readonly();

    let (is_hidden, is_system, _) = crate::utils::get_file_attributes(&metadata, &name);

    let is_dir = metadata.is_dir();
    let size = if is_dir { 0 } else { metadata.len() };

    // Check if this is a trash item and populate trash metadata
    let (original_path, deleted_time) = if path.to_lowercase().contains("$recycle.bin") {
        // Try to get trash metadata
        if let Ok(trash_items) = trash::os_limited::list() {
            let normalize = |p: &std::path::Path| -> String {
                p.to_string_lossy()
                    .to_lowercase()
                    .replace("/", "\\")
                    .trim_start_matches("\\\\?\\")
                    .to_string()
            };

            let normalized_target = normalize(&path_buf);

            // Find matching trash item
            if let Some(item) = trash_items.iter().find(|item| {
                let item_id = normalize(&PathBuf::from(&item.id));
                item_id == normalized_target
            }) {
                let orig_path = item.original_path().to_string_lossy().to_string();
                let del_time = item.time_deleted;
                (Some(orig_path), Some(del_time))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    let shortcut = get_shortcut_info(&path_buf);

    Ok(FileProperties {
        name,
        path,
        parent,
        is_dir,
        size,
        is_calculated: false,
        created,
        modified,
        accessed,
        readonly,
        is_hidden,
        is_system,
        original_path,
        deleted_time,
        folders_count: None,
        files_count: None,
        shortcut,
    })
}

#[cfg(target_os = "windows")]
fn get_shortcut_info(path: &std::path::Path) -> Option<crate::models::ShortcutInfo> {
    use windows::core::{Interface, PCWSTR, HSTRING};
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER, CoInitializeEx, COINIT_APARTMENTTHREADED, CoUninitialize, IPersistFile, STGM_READ};
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;

    if !path.extension().map_or(false, |ext| ext.to_ascii_lowercase() == "lnk") {
        return None;
    }

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        
        let link: IShellLinkW = match CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) {
            Ok(l) => l,
            Err(_) => {
                let _ = CoUninitialize();
                return None;
            }
        };

        let persist: IPersistFile = match link.cast() {
            Ok(p) => p,
            Err(_) => {
                let _ = CoUninitialize();
                return None;
            }
        };

        let wide_path = HSTRING::from(path.to_string_lossy().as_ref());
        if persist.Load(PCWSTR(wide_path.as_ptr()), STGM_READ).is_err() {
            let _ = CoUninitialize();
            return None;
        }

        let mut target_buf = [0u16; 1024];
        let mut find_data = WIN32_FIND_DATAW::default();
        let _ = link.GetPath(&mut target_buf, &mut find_data, 0);
        let target = String::from_utf16_lossy(&target_buf).trim_matches('\0').to_string();

        let mut args_buf = [0u16; 1024];
        let _ = link.GetArguments(&mut args_buf);
        let arguments = String::from_utf16_lossy(&args_buf).trim_matches('\0').to_string();

        let mut dir_buf = [0u16; 1024];
        let _ = link.GetWorkingDirectory(&mut dir_buf);
        let working_dir = String::from_utf16_lossy(&dir_buf).trim_matches('\0').to_string();

        let mut desc_buf = [0u16; 1024];
        let _ = link.GetDescription(&mut desc_buf);
        let description = String::from_utf16_lossy(&desc_buf).trim_matches('\0').to_string();

        let mut icon_buf = [0u16; 260];
        let mut icon_index = 0i32;
        let _ = link.GetIconLocation(&mut icon_buf, &mut icon_index);
        let icon_location = String::from_utf16_lossy(&icon_buf).trim_matches('\0').to_string();

        let run_window = link.GetShowCmd().map(|cmd| cmd.0).unwrap_or(1);

        let _ = CoUninitialize();

        Some(crate::models::ShortcutInfo {
            target,
            arguments,
            working_dir,
            description,
            icon_location,
            icon_index,
            run_window,
        })
    }
}

#[cfg(not(target_os = "windows"))]
fn get_shortcut_info(_path: &std::path::Path) -> Option<crate::models::ShortcutInfo> {
    None
}

#[tauri::command]
pub async fn get_files_summary(paths: Vec<String>) -> Result<FileSummary, CommandError> {
    let mut total_size = 0;
    let mut files_count = 0;
    let mut folders_count = 0;
    let count = paths.len();

    let mut all_readonly = true;
    let mut any_readonly = false;
    let mut all_hidden = true;
    let mut any_hidden = false;
    let mut common_parent: Option<String> = None;
    let mut different_parents = false;

    for (i, p) in paths.iter().enumerate() {
        let pb = PathBuf::from(p);
        let metadata = fs::metadata(&pb)?;

        let readonly = metadata.permissions().readonly();
        let (is_hidden, _, _) = crate::utils::get_file_attributes(&metadata, "");
        let parent = pb
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        if i == 0 {
            all_readonly = readonly;
            all_hidden = is_hidden;
            common_parent = Some(parent);
        } else {
            all_readonly &= readonly;
            all_hidden &= is_hidden;
            if let Some(ref cp) = common_parent {
                if cp != &parent {
                    different_parents = true;
                }
            }
        }
        any_readonly |= readonly;
        any_hidden |= is_hidden;

        if pb.is_dir() {
            folders_count += 1;
            use walkdir::WalkDir;
            for entry in WalkDir::new(&pb).into_iter().skip(1).filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    files_count += 1;
                    total_size += entry.metadata().map(|m| m.len()).unwrap_or(0);
                } else if entry.file_type().is_dir() {
                    folders_count += 1;
                }
            }
        } else {
            files_count += 1;
            total_size += metadata.len();
        }
    }

    if count == 0 {
        all_readonly = false;
        all_hidden = false;
    }

    Ok(FileSummary {
        count,
        total_size,
        files_count,
        folders_count,
        all_readonly,
        any_readonly,
        all_hidden,
        any_hidden,
        parent_path: if different_parents {
            None
        } else {
            common_parent
        },
    })
}


#[tauri::command]
pub async fn show_system_properties(path: String) -> Result<(), CommandError> {
    let pb = validate_path(&path)?;
    let normalized_path = pb.to_string_lossy();
    #[cfg(target_os = "windows")]
    {
        use windows::core::{HSTRING, PCWSTR};
        use windows::Win32::UI::Shell::{SHObjectProperties, SHOP_FILEPATH};

        let wide_path = HSTRING::from(normalized_path.as_ref());

        unsafe {
            let success = SHObjectProperties(
                None,
                SHOP_FILEPATH,
                PCWSTR(wide_path.as_ptr()),
                PCWSTR::null(),
            );

            if !success.as_bool() {
                return Err(CommandError::SystemError("Failed to open system properties".to_string()));
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn calculate_folder_size(path: String) -> Result<FolderSizeResult, CommandError> {
    let pb = validate_path(&path)?;
    
    tauri::async_runtime::spawn_blocking(move || {
        if !pb.is_dir() {
            return Err(CommandError::PathError("Path is not a directory".to_string()));
        }

        let mut size = 0;
        let mut folders_count = 0;
        let mut files_count = 0;
        use walkdir::WalkDir;
        // skip(1) to avoid counting the root folder itself
        for entry in WalkDir::new(&pb).into_iter().skip(1).filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                files_count += 1;
                size += entry.metadata().map(|m| m.len()).unwrap_or(0);
            } else if entry.file_type().is_dir() {
                folders_count += 1;
            }
        }

        Ok(FolderSizeResult {
            size,
            folders_count,
            files_count,
        })
    }).await.map_err(|e| CommandError::SystemError(format!("Task join error: {}", e)))?
}

#[tauri::command]
pub async fn set_shortcut_info(path: String, info: crate::models::ShortcutInfo) -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::{Interface, PCWSTR, HSTRING};
        use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER, CoInitializeEx, COINIT_APARTMENTTHREADED, CoUninitialize, IPersistFile, STGM_READWRITE};
        use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
        use std::path::PathBuf;

        let path_buf = PathBuf::from(&path);
        
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| CommandError::SystemError(format!("CoCreateInstance failed: {}", e)))?;

            let persist: IPersistFile = link.cast()
                .map_err(|e| CommandError::SystemError(format!("Cast to IPersistFile failed: {}", e)))?;

            let wide_path = HSTRING::from(path_buf.to_string_lossy().as_ref());
            persist.Load(PCWSTR(wide_path.as_ptr()), STGM_READWRITE)
                .map_err(|e| CommandError::SystemError(format!("Load failed: {}", e)))?;

            let wide_target = HSTRING::from(info.target);
            link.SetPath(PCWSTR(wide_target.as_ptr()))
                .map_err(|e| CommandError::SystemError(format!("SetPath failed: {}", e)))?;

            let wide_args = HSTRING::from(info.arguments);
            link.SetArguments(PCWSTR(wide_args.as_ptr()))
                .map_err(|e| CommandError::SystemError(format!("SetArguments failed: {}", e)))?;

            let wide_dir = HSTRING::from(info.working_dir);
            link.SetWorkingDirectory(PCWSTR(wide_dir.as_ptr()))
                .map_err(|e| CommandError::SystemError(format!("SetWorkingDirectory failed: {}", e)))?;

            let wide_desc = HSTRING::from(info.description);
            link.SetDescription(PCWSTR(wide_desc.as_ptr()))
                .map_err(|e| CommandError::SystemError(format!("SetDescription failed: {}", e)))?;

            link.SetShowCmd(windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD(info.run_window))
                .map_err(|e| CommandError::SystemError(format!("SetShowCmd failed: {}", e)))?;

            persist.Save(PCWSTR(wide_path.as_ptr()), true)
                .map_err(|e| CommandError::SystemError(format!("Save failed: {}", e)))?;

            let _ = CoUninitialize();
        }
    }
    Ok(())
}
