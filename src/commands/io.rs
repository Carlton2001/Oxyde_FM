use crate::models::{
    CommandError, FileEntry, FileProperties, FileSummary, FolderSizeResult, TransactionDetails,
};
use crate::utils::path_security::validate_path;
use log::info;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

#[cfg(target_os = "windows")]
use windows::core::{GUID, PCWSTR};
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::IBindCtx;
#[cfg(target_os = "windows")]
use windows::Win32::UI::Shell::PropertiesSystem::{
    IPropertyStore, SHGetPropertyStoreFromParsingName, GPS_DEFAULT,
};

#[derive(Serialize)]
pub struct DirResponse {
    pub entries: Vec<FileEntry>,
    pub summary: FileSummary,
    pub is_complete: bool,
    pub is_protected: bool,
}

#[derive(Serialize, Clone)]
pub struct DirBatchEvent {
    pub panel_id: String,
    pub path: String,
    pub entries: Vec<FileEntry>,
    pub is_complete: bool,
    pub request_id: Option<u64>,
}

pub fn get_file_entry_from_metadata(
    metadata: &fs::Metadata,
    name: &str,
    path: &std::path::Path,
) -> FileEntry {
    let modified = metadata
        .modified()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let (is_hidden, is_system, is_reparse_point) =
        crate::utils::get_file_attributes(metadata, name);
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
        folders_count: None,
        files_count: None,
        original_path: None,
        network_details: None,
        deleted_time: None,
    }
}

pub async fn list_dir(
    path: String,
    sort_config: Option<crate::models::session::SortConfig>,
    show_hidden: Option<bool>,
    show_system: Option<bool>,
    _force_refresh: Option<bool>,
) -> Result<DirResponse, CommandError> {
    let show_hidden = show_hidden.unwrap_or(false);
    let show_system = show_system.unwrap_or(false);
    let sort_config = sort_config.unwrap_or_default();

    if path == "trash://" {
        let trash_items = tokio::task::spawn_blocking(crate::commands::ops::list_trash)
            .await
            .map_err(|error| CommandError::IoError(error.to_string()))??;
        let entries = trash_items
            .into_iter()
            .map(|t| FileEntry {
                name: t.name,
                path: t.path,
                is_dir: t.is_dir,
                is_hidden: false,
                is_system: false,
                is_symlink: false,
                is_junction: false,
                size: t.size,
                modified: t.modified,
                is_readonly: false,
                is_protected: false,
                is_calculated: true,
                folders_count: None,
                files_count: None,
                original_path: Some(t.original_path),
                network_details: None,
                deleted_time: Some(t.deleted_time as i64),
            })
            .collect();
        return Ok(DirResponse {
            entries,
            summary: FileSummary {
                count: 0,
                total_size: 0,
                files_count: 0,
                folders_count: 0,
                parent_path: Some(path),
            },
            is_complete: true,
            is_protected: false,
        });
    }

    #[cfg(target_os = "windows")]
    if path == "__network_vincinity__" || is_network_server(&path) {
        let network_path = path.clone();
        let entries =
            tokio::task::spawn_blocking(move || {
                if network_path == "__network_vincinity__" {
                    crate::platform::platform().list_network_vicinity()
                } else {
                    crate::platform::platform().list_network_shares(network_path.trim_end_matches('\\'))
                }
            })
                .await
                .map_err(|error| CommandError::IoError(error.to_string()))?
                .map_err(CommandError::SystemError)?;
        let summary = calculate_summary(&entries, Some(path));
        return Ok(DirResponse {
            entries,
            summary,
            is_complete: true,
            is_protected: false,
        });
    }

    tokio::task::spawn_blocking(move || {
        list_dir_blocking(path, sort_config, show_hidden, show_system)
    })
    .await
    .map_err(|e| CommandError::IoError(e.to_string()))?
}

/// A bare UNC server is enumerated through the network provider, not filesystem metadata.
pub fn is_network_server(path: &str) -> bool {
    path.strip_prefix("\\\\").is_some_and(|tail| {
        let server = tail.trim_end_matches('\\');
        !server.is_empty() && !server.contains(['\\', '/']) && server != "?" && server != "."
    })
}

fn list_dir_blocking(
    path: String,
    sort_config: crate::models::session::SortConfig,
    show_hidden: bool,
    show_system: bool,
) -> Result<DirResponse, CommandError> {
    let path_buf = PathBuf::from(&path);
    if let Some((archive, internal)) = crate::utils::archive::split_virtual_path(&path) {
        if archive.is_file() {
            let mut entries = crate::commands::archive::list_archive_contents(
                archive.to_string_lossy().into_owned(),
                internal,
            )?;
            sort_file_entries(&mut entries, &sort_config);
            let summary = calculate_summary(&entries, Some(path));
            return Ok(DirResponse {
                entries,
                summary,
                is_complete: true,
                is_protected: false,
            });
        }
    }
    let mut entries = Vec::new();

    {
        let dir = std::fs::read_dir(&path_buf).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => CommandError::PathNotFound(path.clone()),
            std::io::ErrorKind::PermissionDenied => CommandError::AccessDenied(path.clone()),
            _ => CommandError::IoError(error.to_string()),
        })?;
        for entry in dir {
            let entry = entry.map_err(|e| CommandError::IoError(e.to_string()))?;
            let metadata = entry
                .metadata()
                .map_err(|e| CommandError::IoError(e.to_string()))?;
            let name = entry.file_name().to_string_lossy().to_string();

            let (is_hidden, is_system, _) = crate::utils::get_file_attributes(&metadata, &name);

            // System flag takes precedence: once is_system is true, only show_system matters.
            // Hidden check is only applied to non-system files (mirrors React V1 logic).
            if is_system {
                if !show_system {
                    continue;
                }
            } else if is_hidden && !show_hidden {
                continue;
            }

            entries.push(crate::models::get_file_entry_from_path(&entry.path())?);
        }
    }

    sort_file_entries(&mut entries, &sort_config);
    let summary = calculate_summary(&entries, Some(path));

    Ok(DirResponse {
        entries,
        summary,
        is_complete: true,
        is_protected: false,
    })
}

pub fn sort_file_entries(entries: &mut [FileEntry], config: &crate::models::session::SortConfig) {
    use crate::models::session::SortDirection;
    use crate::models::session::SortField;

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
            SortField::Size => a
                .size
                .cmp(&b.size)
                .then_with(|| crate::utils::compare_natural(&a.name, &b.name)),
            SortField::Date => a
                .modified
                .cmp(&b.modified)
                .then_with(|| crate::utils::compare_natural(&a.name, &b.name)),
            SortField::Type => {
                let ext_a = std::path::Path::new(&a.name)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let ext_b = std::path::Path::new(&b.name)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                ext_a
                    .cmp(&ext_b)
                    .then_with(|| crate::utils::compare_natural(&a.name, &b.name))
            }
            SortField::Location => a
                .path
                .to_lowercase()
                .cmp(&b.path.to_lowercase())
                .then_with(|| crate::utils::compare_natural(&a.name, &b.name)),
            SortField::DeletedDate => a
                .deleted_time
                .cmp(&b.deleted_time)
                .then_with(|| crate::utils::compare_natural(&a.name, &b.name)),
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

    for e in entries.iter() {
        if e.is_dir {
            folders_count += 1;
        } else {
            files_count += 1;
            total_size += e.size;
        }
    }

    FileSummary {
        count: entries.len(),
        total_size,
        files_count,
        folders_count,
        parent_path,
    }
}

pub async fn create_dir(path: String) -> Result<(), CommandError> {
    let pb = validate_path(&path)?;
    let p_abs = pb.to_string_lossy().to_string();
    info!("Creating directory: {:?}", pb);
    fs::create_dir_all(&pb)?;

    let _tx_details = TransactionDetails {
        paths: vec![p_abs],
        target_dir: None,
        old_path: None,
        new_path: None,
        created_files: None,
        created_dirs: None,
        replaced_files: None,
        overwritten_files: None,
        moved_files: None,
        moved_dirs: None,
        copied_files: None,
        copied_file_hashes: None,
    };
    // History push logic removed for now

    Ok(())
}

pub async fn rename_item(old_path: String, new_path: String) -> Result<(), CommandError> {
    rename_item_with_quality(old_path, new_path, "normal".into()).await
}

pub async fn rename_item_with_quality(
    old_path: String,
    new_path: String,
    quality: String,
) -> Result<(), CommandError> {
    rename_item_controlled(old_path, new_path, quality, std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false))).await
}

pub async fn rename_item_controlled(
    old_path: String, new_path: String, quality: String,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), CommandError> {
    if cancel.load(std::sync::atomic::Ordering::Relaxed) { return Err(CommandError::IoError("Cancelled".into())); }
    let old_pb = validate_path(&old_path)?;
    let new_pb = validate_path(&new_path)?;
    if let Some((archive, internal)) = crate::systems::archive_workspace::virtual_source(&old_pb) {
        use crate::systems::archive_workspace::{
            mutation_operation, virtual_source, writable, ArchiveWorkspace, mutation_locks,
        };
        let Some((target_archive, target_internal)) = virtual_source(&new_pb) else {
            return Err(CommandError::PathError(
                "Rename must stay inside the archive".into(),
            ));
        };
        if archive != target_archive || !writable(&archive) {
            return Err(CommandError::ArchiveError(
                "This archive format is read-only".into(),
            ));
        }
        let locks = mutation_locks([archive.clone()]).map_err(CommandError::ArchiveError)?;
        let operation = mutation_operation(&quality);
        operation.lock().unwrap().cancel_flag = cancel;
        let _lock = crate::systems::archive_workspace::lock_for_operation(&locks[0], &operation).map_err(CommandError::ArchiveError)?;
        let workspace = ArchiveWorkspace::open_for_operation(archive, &operation).map_err(CommandError::ArchiveError)?;
        let source = workspace.path(&internal).map_err(CommandError::PathError)?;
        let destination = workspace
            .path(&target_internal)
            .map_err(CommandError::PathError)?;
        validate_rename_target(&source, &destination)?;
        fs::rename(source, destination)?;
        return workspace
            .commit(&operation)
            .map_err(CommandError::ArchiveError);
    }
    validate_rename_target(&old_pb, &new_pb)?;
    let old_abs = old_pb.to_string_lossy().to_string();
    let new_abs = new_pb.to_string_lossy().to_string();

    info!("Renaming {:?} to {:?}", old_pb, new_pb);
    fs::rename(old_pb, new_pb)?;

    let _tx_details = TransactionDetails {
        paths: vec![],
        target_dir: None,
        old_path: Some(old_abs),
        new_path: Some(new_abs),
        created_files: None,
        created_dirs: None,
        replaced_files: None,
        overwritten_files: None,
        moved_files: None,
        moved_dirs: None,
        copied_files: None,
        copied_file_hashes: None,
    };
    // History push logic removed

    Ok(())
}

fn validate_rename_target(
    old_path: &std::path::Path,
    new_path: &std::path::Path,
) -> Result<(), CommandError> {
    if old_path.parent() != new_path.parent() {
        return Err(CommandError::PathError(
            "Rename target must stay in the source directory".to_string(),
        ));
    }
    if old_path == new_path {
        return Err(CommandError::PathError(
            "Source and rename target are identical".to_string(),
        ));
    }
    if new_path.exists() {
        return Err(CommandError::IoError(format!(
            "Rename target already exists: {}",
            new_path.display()
        )));
    }
    if new_path.file_name().is_none() {
        return Err(CommandError::PathError(
            "Rename target must have a file name".to_string(),
        ));
    }
    Ok(())
}

pub fn get_file_properties(path: String) -> Result<FileProperties, CommandError> {
    if path.starts_with("::{")
        || path.starts_with("?")
        || (path.starts_with("\\\\") && path.split('\\').filter(|s| !s.is_empty()).count() == 1)
    {
        // Handle shell items (Network devices or Computer roots)
        let mut props = FileProperties {
            // ... (rest initialized in code)
            name: path.split('\\').next_back().unwrap_or(&path).to_string(), // Best effort name
            path: path.clone(),
            parent: "Réseau".to_string(),
            is_dir: false,
            size: 0,
            is_calculated: true,
            created: 0,
            modified: 0,
            accessed: 0,
            original_path: None,
            deleted_time: None,
            folders_count: None,
            files_count: None,
            shortcut: None,
            is_media_device: Some(true),
            has_web_page: Some(false),
            manufacturer: None,
            model_name: None,
            model_number: None,
            serial_number: None,
            mac_address: None,
            ip_address: None,
            unique_id: None,
            web_page_url: None,
            debug_props: None,
        };

        #[cfg(target_os = "windows")]
        unsafe {
            use windows::Win32::Foundation::PROPERTYKEY;
            use windows::Win32::System::Com::{
                CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED,
            };

            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let wide_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            if let Ok(store) = SHGetPropertyStoreFromParsingName::<
                _,
                Option<&IBindCtx>,
                IPropertyStore,
            >(PCWSTR(wide_path.as_ptr()), None, GPS_DEFAULT)
            {
                let mut d_props = Vec::new();
                // Debug out what properties are actually here
                if let Ok(count) = store.GetCount() {
                    for i in 0..count {
                        let mut key = PROPERTYKEY::default();
                        if let Ok(()) = store.GetAt(i, &mut key) {
                            if let Ok(var) = store.GetValue(&key) {
                                if let Ok(pwstr) = windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc(&var) {
                                    let val = pwstr.to_string().unwrap_or_default();
                                    if !val.is_empty() {
                                        d_props.push(format!("[{:?}, {}] = {}", key.fmtid, key.pid, val));
                                    }
                                    windows::Win32::System::Com::CoTaskMemFree(Some(pwstr.as_ptr() as *const _));
                                }
                            }
                        }
                    }
                }
                props.debug_props = Some(d_props);

                let get_prop = |guid_values: (u32, u16, u16, [u8; 8]),
                                pid: u32|
                 -> Option<String> {
                    let key = PROPERTYKEY {
                        fmtid: GUID::from_values(
                            guid_values.0,
                            guid_values.1,
                            guid_values.2,
                            guid_values.3,
                        ),
                        pid,
                    };
                    if let Ok(var) = store.GetValue(&key) {
                        if let Ok(pwstr) =
                            windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc(
                                &var,
                            )
                        {
                            let val = pwstr.to_string().unwrap_or_default();
                            windows::Win32::System::Com::CoTaskMemFree(Some(
                                pwstr.as_ptr() as *const _
                            ));
                            if !val.is_empty() {
                                return Some(val);
                            }
                        }
                    }
                    None
                };

                // PKEY_Device_Manufacturer {A45C2502-DFC5-491D-8F1D-DC141151D4B2}, 7
                props.manufacturer = get_prop(
                    (
                        0xA45C2502,
                        0xDFC5,
                        0x491D,
                        [0x8F, 0x1D, 0xDC, 0x14, 0x11, 0x51, 0xD4, 0xB2],
                    ),
                    7,
                );
                // PKEY_Device_ModelName {A45C2502-DFC5-491D-8F1D-DC141151D4B2}, 11
                props.model_name = get_prop(
                    (
                        0xA45C2502,
                        0xDFC5,
                        0x491D,
                        [0x8F, 0x1D, 0xDC, 0x14, 0x11, 0x51, 0xD4, 0xB2],
                    ),
                    11,
                );
                // PKEY_Device_ModelNumber {A45C2502-DFC5-491D-8F1D-DC141151D4B2}, 12
                props.model_number = get_prop(
                    (
                        0xA45C2502,
                        0xDFC5,
                        0x491D,
                        [0x8F, 0x1D, 0xDC, 0x14, 0x11, 0x51, 0xD4, 0xB2],
                    ),
                    12,
                );
                // PKEY_Device_SerialNumber {A45C2502-DFC5-491D-8F1D-DC141151D4B2}, 15
                props.serial_number = get_prop(
                    (
                        0xA45C2502,
                        0xDFC5,
                        0x491D,
                        [0x8F, 0x1D, 0xDC, 0x14, 0x11, 0x51, 0xD4, 0xB2],
                    ),
                    15,
                );

                // PKEY_Devices_IpAddress {65AD9E1B-0A09-40A4-9780-45C31E310531}, 3
                props.ip_address = get_prop(
                    (
                        0x65AD9E1B,
                        0x0A09,
                        0x40A4,
                        [0x97, 0x80, 0x45, 0xC3, 0x1E, 0x31, 0x05, 0x31],
                    ),
                    3,
                );
                // PKEY_Devices_MacAddress {65AD9E1B-0A09-40A4-9780-45C31E310531}, 2
                props.mac_address = get_prop(
                    (
                        0x65AD9E1B,
                        0x0A09,
                        0x40A4,
                        [0x97, 0x80, 0x45, 0xC3, 0x1E, 0x31, 0x05, 0x31],
                    ),
                    2,
                );
                // PKEY_Devices_DiscoveryId {65AD9E1B-0A09-40A4-9780-45C31E310531}, 10 (Unique ID)
                props.unique_id = get_prop(
                    (
                        0x65AD9E1B,
                        0x0A09,
                        0x40A4,
                        [0x97, 0x80, 0x45, 0xC3, 0x1E, 0x31, 0x05, 0x31],
                    ),
                    10,
                );

                // For name, sometimes PKEY_Device_FriendlyName {A45C2502-DFC5-491D-8F1D-DC141151D4B2}, 14 is better
                if let Some(friendly) = get_prop(
                    (
                        0xA45C2502,
                        0xDFC5,
                        0x491D,
                        [0x8F, 0x1D, 0xDC, 0x14, 0x11, 0x51, 0xD4, 0xB2],
                    ),
                    14,
                ) {
                    props.name = friendly;
                }

                // Check for webpage verb again to be sure
                props.has_web_page =
                    Some(props.ip_address.is_some() || props.name.to_lowercase() == "bbox");
            } else {
                props.debug_props =
                    Some(vec!["SHGetPropertyStoreFromParsingName failed".to_string()]);
            }
            CoUninitialize();
        }

        return Ok(props);
    }

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
        original_path,
        deleted_time,
        folders_count: None,
        files_count: None,
        shortcut,
        is_media_device: Some(false),
        has_web_page: Some(false),
        manufacturer: None,
        model_name: None,
        model_number: None,
        serial_number: None,
        mac_address: None,
        ip_address: None,
        unique_id: None,
        web_page_url: None,
        debug_props: None,
    })
}

#[cfg(target_os = "windows")]
fn get_shortcut_info(path: &std::path::Path) -> Option<crate::models::ShortcutInfo> {
    use windows::core::{Interface, HSTRING, PCWSTR};
    use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    if !path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("lnk"))
    {
        return None;
    }

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let link: IShellLinkW = match CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) {
            Ok(l) => l,
            Err(_) => {
                CoUninitialize();
                return None;
            }
        };

        let persist: IPersistFile = match link.cast() {
            Ok(p) => p,
            Err(_) => {
                CoUninitialize();
                return None;
            }
        };

        let wide_path = HSTRING::from(path.to_string_lossy().as_ref());
        if persist.Load(PCWSTR(wide_path.as_ptr()), STGM_READ).is_err() {
            CoUninitialize();
            return None;
        }

        let mut target_buf = [0u16; 1024];
        let mut find_data = WIN32_FIND_DATAW::default();
        let _ = link.GetPath(&mut target_buf, &mut find_data, 0);
        let target = String::from_utf16_lossy(&target_buf)
            .trim_matches('\0')
            .to_string();

        let mut args_buf = [0u16; 1024];
        let _ = link.GetArguments(&mut args_buf);
        let arguments = String::from_utf16_lossy(&args_buf)
            .trim_matches('\0')
            .to_string();

        let mut dir_buf = [0u16; 1024];
        let _ = link.GetWorkingDirectory(&mut dir_buf);
        let working_dir = String::from_utf16_lossy(&dir_buf)
            .trim_matches('\0')
            .to_string();

        let mut desc_buf = [0u16; 1024];
        let _ = link.GetDescription(&mut desc_buf);
        let description = String::from_utf16_lossy(&desc_buf)
            .trim_matches('\0')
            .to_string();

        let mut icon_buf = [0u16; 260];
        let mut icon_index = 0i32;
        let _ = link.GetIconLocation(&mut icon_buf, &mut icon_index);
        let icon_location = String::from_utf16_lossy(&icon_buf)
            .trim_matches('\0')
            .to_string();

        let run_window = link.GetShowCmd().map(|cmd| cmd.0).unwrap_or(1);

        CoUninitialize();

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

/// Retourne le chemin cible d'un raccourci (.lnk sur Windows, .desktop sur Linux).
pub fn resolve_shortcut_target(path: &str) -> Option<String> {
    get_shortcut_info(std::path::Path::new(path))
        .map(|info| info.target)
        .or_else(|| crate::platform::platform().resolve_shortcut(path))
}

pub async fn get_files_summary(paths: Vec<String>) -> Result<FileSummary, CommandError> {
    let mut total_size = 0;
    let mut files_count = 0;
    let mut folders_count = 0;
    let count = paths.len();

    let mut common_parent: Option<String> = None;
    let mut different_parents = false;

    for (i, p) in paths.iter().enumerate() {
        let pb = PathBuf::from(p);
        let metadata = fs::metadata(&pb)?;

        let parent = pb
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        if i == 0 {
            common_parent = Some(parent);
        } else if let Some(ref cp) = common_parent {
            if cp != &parent {
                different_parents = true;
            }
        }

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

    Ok(FileSummary {
        count,
        total_size,
        files_count,
        folders_count,
        parent_path: if different_parents {
            None
        } else {
            common_parent
        },
    })
}

pub async fn show_system_properties(path: String) -> Result<(), CommandError> {
    let normalized_path = if path.starts_with("::{") || path.starts_with("?") {
        path
    } else {
        validate_path(&path)?.to_string_lossy().into_owned()
    };
    crate::platform::platform().show_properties_dialog(&normalized_path);
    Ok(())
}

pub async fn calculate_folder_size(path: String) -> Result<FolderSizeResult, CommandError> {
    let pb = validate_path(&path)?;

    tokio::task::spawn_blocking(move || {
        if !pb.is_dir() {
            return Err(CommandError::PathError(
                "Path is not a directory".to_string(),
            ));
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
    })
    .await
    .map_err(|e| CommandError::SystemError(format!("Task join error: {}", e)))?
}

pub async fn set_shortcut_info(
    path: String,
    info: crate::models::ShortcutInfo,
) -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    {
        use std::path::PathBuf;
        use windows::core::{Interface, HSTRING, PCWSTR};
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED, STGM_READWRITE,
        };
        use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

        let path_buf = PathBuf::from(&path);

        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| {
                    CommandError::SystemError(format!("CoCreateInstance failed: {}", e))
                })?;

            let persist: IPersistFile = link.cast().map_err(|e| {
                CommandError::SystemError(format!("Cast to IPersistFile failed: {}", e))
            })?;

            let wide_path = HSTRING::from(path_buf.to_string_lossy().as_ref());
            persist
                .Load(PCWSTR(wide_path.as_ptr()), STGM_READWRITE)
                .map_err(|e| CommandError::SystemError(format!("Load failed: {}", e)))?;

            let wide_target = HSTRING::from(info.target);
            link.SetPath(PCWSTR(wide_target.as_ptr()))
                .map_err(|e| CommandError::SystemError(format!("SetPath failed: {}", e)))?;

            let wide_args = HSTRING::from(info.arguments);
            link.SetArguments(PCWSTR(wide_args.as_ptr()))
                .map_err(|e| CommandError::SystemError(format!("SetArguments failed: {}", e)))?;

            let wide_dir = HSTRING::from(info.working_dir);
            link.SetWorkingDirectory(PCWSTR(wide_dir.as_ptr()))
                .map_err(|e| {
                    CommandError::SystemError(format!("SetWorkingDirectory failed: {}", e))
                })?;

            let wide_desc = HSTRING::from(info.description);
            link.SetDescription(PCWSTR(wide_desc.as_ptr()))
                .map_err(|e| CommandError::SystemError(format!("SetDescription failed: {}", e)))?;

            link.SetShowCmd(windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD(
                info.run_window,
            ))
            .map_err(|e| CommandError::SystemError(format!("SetShowCmd failed: {}", e)))?;

            persist
                .Save(PCWSTR(wide_path.as_ptr()), true)
                .map_err(|e| CommandError::SystemError(format!("Save failed: {}", e)))?;

            CoUninitialize();
        }
    }
    Ok(())
}

#[cfg(test)]
mod directory_tests {
    use super::*;

    #[test]
    fn network_server_detection_keeps_share_paths_in_the_file_system() {
        for path in ["\\\\server", "\\\\server\\", "\\\\192.168.1.2", "\\\\PCS", "\\\\PCS\\"] {
            assert!(is_network_server(path));
        }
        for path in ["\\\\server\\share", "\\\\server\\share\\folder", "\\\\PCS\\so39i Agde", "\\\\", "\\\\?\\C:\\", "\\\\.\\pipe", "C:\\", "::{device}"] {
            assert!(!is_network_server(path));
        }
    }

    #[test]
    fn missing_directory_is_an_error_instead_of_an_empty_listing() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("missing").to_string_lossy().into_owned();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        assert!(runtime
            .block_on(list_dir(missing, None, None, None, None))
            .is_err());
    }

    #[test]
    fn archive_directory_listing_keeps_virtual_folder_levels() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("folder");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(source.join("nested/file.txt"), b"data").unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        for format in ["zip", "7z", "tar", "tar.zst"] {
            let archive = directory.path().join(format!("listing.{format}"));
            runtime
                .block_on(crate::commands::archive::compress_to_archive(
                    vec![source.to_string_lossy().into_owned()],
                    archive.to_string_lossy().into_owned(),
                    format.into(),
                    "normal".into(),
                    &crate::commands::archive::ArchiveState(std::sync::atomic::AtomicBool::new(
                        false,
                    )),
                ))
                .unwrap();
            let root = runtime
                .block_on(list_dir(
                    archive.to_string_lossy().into_owned(),
                    None,
                    None,
                    None,
                    None,
                ))
                .unwrap();
            assert_eq!(root.entries.len(), 1);
            assert_eq!(
                root.entries[0].path,
                archive.join("folder").to_string_lossy()
            );
            let nested = runtime
                .block_on(list_dir(
                    archive.join("folder/nested").to_string_lossy().into_owned(),
                    None,
                    None,
                    None,
                    None,
                ))
                .unwrap();
            assert_eq!(nested.entries.len(), 1);
            assert_eq!(
                std::path::PathBuf::from(&nested.entries[0].path),
                archive.join("folder").join("nested").join("file.txt")
            );
        }
    }

    #[test]
    fn cancelled_archive_rename_preserves_original() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("file.txt");
        fs::write(&source, b"source").unwrap();
        let archive = directory.path().join("cancel.7z");
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(crate::commands::archive::compress_to_archive(vec![source.to_string_lossy().into_owned()], archive.to_string_lossy().into_owned(), "7z".into(), "normal".into(), &crate::commands::archive::ArchiveState(std::sync::atomic::AtomicBool::new(false)))).unwrap();
        let original = fs::read(&archive).unwrap();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        assert!(runtime.block_on(rename_item_controlled(archive.join("file.txt").to_string_lossy().into_owned(), archive.join("renamed.txt").to_string_lossy().into_owned(), "normal".into(), cancel)).is_err());
        assert_eq!(fs::read(&archive).unwrap(), original);
    }

    #[test]
    fn rename_target_cannot_escape_or_overwrite() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        let existing = root.path().join("existing.txt");
        let outside = root.path().parent().unwrap().join("outside.txt");
        std::fs::write(&source, b"source").unwrap();
        std::fs::write(&existing, b"existing").unwrap();

        assert!(validate_rename_target(&source, &root.path().join("nested/target.txt")).is_err());
        assert!(validate_rename_target(&source, &existing).is_err());
        assert!(validate_rename_target(&source, &outside).is_err());
        assert_eq!(std::fs::read(&source).unwrap(), b"source");
        assert_eq!(std::fs::read(&existing).unwrap(), b"existing");
    }
}
