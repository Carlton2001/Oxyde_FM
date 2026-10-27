use crate::models::{CommandError, FileEntry, FileProperties, FolderSizeResult};
use crate::utils::path_security::validate_path;
use log::info;
use std::fs;
use std::time::SystemTime;

pub async fn list_dir(
    path: String,
    show_hidden: Option<bool>,
    show_system: Option<bool>,
) -> Result<Vec<FileEntry>, CommandError> {
    list_dir_impl(path, show_hidden, show_system, true).await
}

/// UI preparation supplies its own stable sort in the background.
pub async fn list_dir_unsorted(path: String, show_hidden: Option<bool>, show_system: Option<bool>) -> Result<Vec<FileEntry>, CommandError> {
    list_dir_impl(path, show_hidden, show_system, false).await
}

async fn list_dir_impl(path: String, show_hidden: Option<bool>, show_system: Option<bool>, ordered: bool) -> Result<Vec<FileEntry>, CommandError> {
    let show_hidden = show_hidden.unwrap_or(false);
    let show_system = show_system.unwrap_or(false);

    if path == "trash://" {
        let trash_items = tokio::task::spawn_blocking(crate::commands::ops::list_trash_unsorted)
            .await
            .map_err(|error| CommandError::IoError(error.to_string()))??;
        let entries = trash_items
            .into_iter()
            .map(|t| FileEntry {
                native_kind: None,
                created: None,
                content_stamp: None,
                archive_path: None,
                name: t.name,
                path: t.path,
                is_dir: t.is_dir,
                is_hidden: false,
                is_system: false,
                is_symlink: false,
                is_broken_link: false,
                is_junction: false,
                size: t.size,
                modified: t.modified,
                is_calculated: true,
                folders_count: None,
                files_count: None,
                original_path: Some(t.original_path),
                network_details: None,
                deleted_time: Some(t.deleted_time as i64),
            })
            .collect();
        return Ok(entries);
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
        return Ok(entries);
    }

    tokio::task::spawn_blocking(move || {
        list_dir_blocking(path, show_hidden, show_system, ordered)
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
    show_hidden: bool,
    show_system: bool,
    ordered: bool,
) -> Result<Vec<FileEntry>, CommandError> {
    let path_buf = crate::utils::path_text::decode(&path);
    if let Some((archive, internal)) = crate::utils::archive::resolve_virtual_path(&path_buf) {
        if archive.is_file() {
            let mut entries = crate::commands::archive::list_archive_contents(
                archive.to_string_lossy().into_owned(),
                internal,
            )?;
            if ordered { sort_file_entries(&mut entries); }
            return Ok(entries);
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

            if !crate::utils::visibility::is_visible(is_hidden, is_system, show_hidden, show_system) {
                continue;
            }

            entries.push(crate::models::file_entry::file_entry_from_metadata(&entry.path(), &metadata));
        }
    }

    if ordered { sort_file_entries(&mut entries); }
    Ok(entries)
}

fn sort_file_entries(entries: &mut [FileEntry]) {
    entries.sort_unstable_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| crate::utils::compare_natural(&a.name, &b.name))
    });
}

#[cfg(test)]
#[path = "io/tests/listing_preparation.rs"]
mod listing_preparation_tests;

#[cfg(all(test, unix))]
#[path = "io/tests/invalid_names.rs"]
mod invalid_name_tests;

/// Worker-only local creation: capture the newly created object before closing it.
pub fn create_local_folder(
    path: &std::path::Path,
) -> std::io::Result<Option<crate::models::transaction::HistoryFileIdentity>> {
    if crate::utils::archive::resolve_virtual_path(path).is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Local folder creation requires a filesystem path",
        ));
    }
    crate::systems::new_folder_history::create(path)
}

/// Worker-only relocation using the proof captured by the original rename.
pub fn record_captured_folder_rename(
    history: &crate::models::HistoryManager,
    old: &str,
    new: &str,
    identity: crate::models::transaction::HistoryFileIdentity,
) -> bool {
    history.relocate_created_folder(old, new, identity)
}

/// Worker-only journal relocation; an independently replaced destination is never adopted.
pub fn record_created_folder_rename(
    history: &crate::models::HistoryManager,
    old: &str,
    new: &str,
) -> bool {
    let captured = (|| {
        let path = std::path::Path::new(new);
        let parents = crate::systems::path_parents::HeldParents::new(path)?;
        let file = crate::platform::platform()
            .track_operation_directory(parents.directory(), &parents.resolved_path(path)?)?;
        crate::models::transaction::HistoryFileIdentity::from_file(&file)
    })();
    match captured {
        Ok(identity) => history.relocate_created_folder(old, new, identity),
        Err(_) => false,
    }
}

pub async fn rename_item_controlled(
    old_path: String, new_path: String, quality: String,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), CommandError> {
    rename_item_controlled_with_identity(old_path, new_path, quality, cancel).await.map(|_| ())
}

/// Local renames return a proof captured on their original handle. Archive renames
/// do not create filesystem history and return None.
pub async fn rename_item_controlled_with_identity(
    old_path: String, new_path: String, quality: String,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<Option<crate::models::transaction::HistoryFileIdentity>, CommandError> {
    if cancel.load(std::sync::atomic::Ordering::Relaxed) { return Err(CommandError::IoError("Cancelled".into())); }
    // Renaming is how a name that is not UTF-8 gets fixed, so both sides keep its bytes.
    let old_pb = validate_path(&old_path)?;
    let old_pb = crate::utils::path_text::decode(&old_pb.to_string_lossy());
    let new_pb = validate_path(&new_path)?;
    let new_pb = crate::utils::path_text::decode(&new_pb.to_string_lossy());
    if let Some((archive, internal)) = crate::systems::archive_workspace::virtual_source(&old_pb) {
        return crate::systems::archive_recovery::scope(|| {
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
        workspace.rename(&source, &destination, &mut || {
            crate::systems::file_ops::FileOperationManager::archive_checkpoint(&operation)
                .map_err(std::io::Error::other)
        })?;
        workspace
            .commit(&operation)
            .map(|_| None)
            .map_err(CommandError::ArchiveError)
        });
    }
    validate_rename_shape(&old_pb, &new_pb)?;
    info!("Renaming {:?} to {:?}", old_pb, new_pb);
    let identity = crate::systems::item_rename::rename_captured_with_checkpoint(&old_pb, &new_pb, None, &mut || {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Cancelled",
            ))
        } else {
            Ok(())
        }
    })?;

    Ok(identity)
}

fn validate_rename_target(
    old_path: &std::path::Path,
    new_path: &std::path::Path,
) -> Result<(), CommandError> {
    validate_rename_shape(old_path, new_path)?;
    // Archive edits retain their existing requirement for an absent entry.
    if new_path.exists() {
        return Err(CommandError::IoError(format!(
            "Rename target already exists: {}",
            new_path.display()
        )));
    }
    Ok(())
}

fn validate_rename_shape(
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
            link_target: None,
            is_broken_link: false,
            permissions: None,
            mime_type: None,
        };

        if let Ok(Some(name)) = crate::platform::platform().shell_item_friendly_name(&path) {
            props.name = name;
        }

        return Ok(props);
    }

    // Reading keeps the bytes of a name that is not UTF-8.
    let path_buf = crate::utils::path_text::decode(&validate_path(&path)?.to_string_lossy());
    // Outside Windows a link describes its target, as in Nautilus; a broken link
    // falls back to the link itself.
    #[cfg(not(target_os = "windows"))]
    let link_target = fs::symlink_metadata(&path_buf)?
        .file_type()
        .is_symlink()
        .then(|| fs::read_link(&path_buf))
        .transpose()?
        .map(|target| target.to_string_lossy().into_owned());
    #[cfg(target_os = "windows")]
    let link_target: Option<String> = None;
    let (metadata, is_broken_link) = match fs::metadata(&path_buf) {
        Ok(metadata) => (metadata, false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && link_target.is_some() => {
            (fs::symlink_metadata(&path_buf)?, true)
        }
        Err(error) => return Err(error.into()),
    };

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
    #[cfg(not(target_os = "windows"))]
    let (original_path, deleted_time) = crate::commands::ops::trash_properties_metadata(&path_buf)
        .map_or((None, None), |(original, deleted)| (Some(original), deleted));
    #[cfg(target_os = "windows")]
    let (original_path, deleted_time) = if path.to_lowercase().contains("$recycle.bin") {
        // Try to get trash metadata
        if let Ok(trash_items) = crate::platform::platform().list_trash_items() {
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
                let item_id = normalize(&std::path::PathBuf::from(&item.id));
                item_id == normalized_target
            }) {
                let (original, deleted) = trash_properties_metadata(item);
                (Some(original), deleted)
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
    let permissions = (!is_broken_link)
        .then(|| crate::platform::platform().unix_permissions(&path_buf))
        .flatten();

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
        link_target,
        is_broken_link,
        permissions,
        mime_type: None,
    })
}

/// Type of a listed item in the interface language, where the system has a MIME
/// database. Reads the start of a file whose name says nothing.
pub fn describe_mime_type(path: &str, is_dir: bool, language: crate::models::preferences::Language) -> Option<crate::models::MimeType> {
    crate::platform::platform().mime_type(&crate::utils::path_text::decode(path), is_dir, language.as_str())
}

/// Applies the Permissions tab; the platform checks it still reaches the object read.
pub async fn set_unix_permissions(
    path: String,
    change: crate::models::permissions::PermissionChange,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), crate::models::permissions::PermissionError> {
    use crate::models::permissions::PermissionError;
    let path = validate_path(&path).map_err(|error| PermissionError::Failed(error.to_string()))?;
    let path = crate::utils::path_text::decode(&path.to_string_lossy());
    tokio::task::spawn_blocking(move || crate::platform::platform().set_unix_permissions(&path, &change, &cancel))
        .await
        .map_err(|error| PermissionError::Failed(error.to_string()))?
}

/// Levels Nautilus proposes for enclosed items, from the umask (`022` when unknown).
pub fn enclosed_defaults() -> crate::models::permissions::Levels {
    crate::models::permissions::Levels::from_umask(crate::platform::platform().umask().unwrap_or(0o022))
}

/// Applies the Permissions tab of a selection, one item after the other, each with
/// its own result; stops once `cancel` is set.
pub async fn set_selection_permissions(
    changes: Vec<(String, crate::models::permissions::PermissionChange)>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Vec<(String, Result<(), crate::models::permissions::PermissionError>)> {
    let mut results = Vec::with_capacity(changes.len());
    for (path, change) in changes {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        let result = set_unix_permissions(path.clone(), change, cancel.clone()).await;
        results.push((path, result));
    }
    results
}

#[cfg(target_os = "windows")]
fn trash_properties_metadata(item: &trash::TrashItem) -> (String, Option<i64>) {
    let original = super::ops::original_trash_path(item)
        .to_string_lossy()
        .into_owned();
    let deleted = item
        .time_deleted
        .checked_mul(1000)
        .filter(|time| *time >= 0);
    (original, deleted)
}

fn get_shortcut_info(path: &std::path::Path) -> Option<crate::models::ShortcutInfo> {
    crate::platform::platform().shortcut_info(path)
}

pub async fn calculate_folder_size(path: String) -> Result<FolderSizeResult, CommandError> {
    calculate_folder_size_cancellable(
        path,
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    )
    .await
}

/// Applies the general Turbo preference to all size scans, including Properties.
pub fn set_folder_size_turbo(enabled: bool) {
    crate::systems::folder_sizes::set_turbo(enabled);
}

pub async fn calculate_folder_size_cancellable(
    path: String,
    cancellation: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<FolderSizeResult, CommandError> {
    crate::systems::folder_sizes::calculate(validate_path(&path)?, cancellation).await
}

#[cfg(test)]
use crate::systems::folder_sizes::excluded as size_scan_excluded;

#[cfg(test)]
mod size_scan_tests {
    use super::*;

    #[test]
    fn excludes_system_branches_without_excluding_similar_user_names() {
        for path in [r"C:\SYSTEM VOLUME INFORMATION\data", r"C:\Users\user\Microsoft\Windows\Containers\data"] {
            assert!(size_scan_excluded(std::path::Path::new(path)));
        }
        for path in [r"C:\Windows", r"C:\Program Files", r"C:\System Volume Information backup", r"C:\Microsoft\Windows\Containers backup"] {
            assert!(!size_scan_excluded(std::path::Path::new(path)));
        }
    }

    #[test]
    fn recursive_size_ignores_excluded_content_and_counts_nested_files() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("normal")).unwrap();
        std::fs::create_dir(root.path().join("System Volume Information")).unwrap();
        std::fs::write(root.path().join("normal/file"), b"12345").unwrap();
        std::fs::write(root.path().join("System Volume Information/ignored"), b"123456789").unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(calculate_folder_size(root.path().to_string_lossy().into_owned())).unwrap();
        assert_eq!(result.size, 5);
        assert_eq!(result.files_count, 1);
        assert_eq!(result.folders_count, 1);
    }

    #[test]
    fn concurrent_scans_keep_exact_independent_totals_including_hidden_files() {
        let root = tempfile::tempdir().unwrap();
        for name in ["first", "second"] {
            for index in 0..32 {
                let directory = root.path().join(name).join(format!(".branch-{index}"));
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::write(directory.join(".file"), vec![0; index + 1]).unwrap();
            }
        }
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let results = runtime.block_on(futures::future::join_all(["first", "second"].map(|name| {
            calculate_folder_size(root.path().join(name).to_string_lossy().into_owned())
        })));
        for result in results {
            let result = result.unwrap();
            assert_eq!(result.size, 528);
            assert_eq!(result.files_count, 32);
            assert_eq!(result.folders_count, 32);
        }
    }
}

pub async fn set_shortcut_info(path: String, info: crate::models::ShortcutInfo) -> Result<(), CommandError> {
    tokio::task::spawn_blocking(move || crate::platform::platform().set_shortcut_info(&path, &info))
        .await.map_err(|error| CommandError::SystemError(error.to_string()))?.map_err(CommandError::SystemError)
}

#[cfg(test)]
mod directory_tests {
    use super::*;
    use crate::systems::archive_workspace::mutation_operation;
    use crate::systems::file_ops::FileOperationManager;

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
            .block_on(list_dir(missing, None, None))
            .is_err());
    }

    #[test]
    fn archive_directory_listing_keeps_virtual_folder_levels() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("folder");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(source.join("nested/file.txt"), b"data").unwrap();
        fs::write(source.join("nested/File10.txt"), b"ten").unwrap();
        fs::write(source.join("nested/file2.txt"), b"two").unwrap();
        fs::create_dir(source.join("nested/child")).unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let ordinary = runtime
            .block_on(list_dir(
                source.join("nested").to_string_lossy().into_owned(),
                None,
                None,
            ))
            .unwrap();
        let expected = ["child", "file.txt", "file2.txt", "File10.txt"];
        assert_eq!(
            ordinary.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(),
            expected,
        );
        for format in ["zip", "7z", "tar", "tar.zst"] {
            let archive = directory.path().join(format!("listing.{format}"));
            FileOperationManager::perform_compress(
                &mutation_operation("normal"),
                vec![source.clone()],
                Some(archive.clone()),
            )
            .unwrap();
            let root = runtime
                .block_on(list_dir(
                    archive.to_string_lossy().into_owned(),
                    None,
                    None,
                ))
                .unwrap();
            assert_eq!(root.len(), 1);
            assert_eq!(
                root[0].path,
                archive.join("folder").to_string_lossy()
            );
            let nested = runtime
                .block_on(list_dir(
                    archive.join("folder/nested").to_string_lossy().into_owned(),
                    None,
                    None,
                ))
                .unwrap();
            assert_eq!(
                nested.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(),
                expected,
            );
            assert_eq!(
                std::path::PathBuf::from(&nested[1].path),
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
        FileOperationManager::perform_compress(
            &mutation_operation("normal"),
            vec![source],
            Some(archive.clone()),
        )
        .unwrap();
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

#[cfg(all(test, target_os = "windows"))]
mod trash_properties_tests {
    use super::trash_properties_metadata;

    fn fixture() -> trash::TrashItem {
        trash::TrashItem {
            id: std::env::temp_dir()
                .join("missing-trash-properties-fixture")
                .join("$Rfixture.lnk")
                .into_os_string(),
            name: "Raccourci école".into(),
            original_parent: std::env::temp_dir().join("Documents"),
            time_deleted: 1_791_370_123,
        }
    }

    #[test]
    fn properties_use_restore_name_rules_and_millisecond_deletion_time() {
        let item = fixture();
        let (original, deleted) = trash_properties_metadata(&item);
        assert_eq!(
            original,
            item.original_parent
                .join("Raccourci école.lnk")
                .to_string_lossy()
        );
        assert_eq!(deleted, Some(1_791_370_123_000));
    }

    #[test]
    fn missing_or_overflowing_deletion_times_are_not_published_as_dates() {
        let mut item = fixture();
        for seconds in [-1, i64::MAX, i64::MIN] {
            item.time_deleted = seconds;
            assert_eq!(trash_properties_metadata(&item).1, None);
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn properties_prefer_the_sidecar_original_name_over_the_shell_label() {
        let temp = tempfile::tempdir().unwrap();
        let original = temp.path().join("Documents").join("Rapport école 🦀.txt");
        let wide: Vec<_> = original
            .to_string_lossy()
            .encode_utf16()
            .chain([0])
            .collect();
        let mut metadata = 2u64.to_le_bytes().to_vec();
        metadata.resize(24, 0);
        metadata.extend_from_slice(&(wide.len() as u32).to_le_bytes());
        for unit in wide {
            metadata.extend_from_slice(&unit.to_le_bytes());
        }
        std::fs::write(temp.path().join("$Ifixture.txt"), metadata).unwrap();
        let mut item = fixture();
        item.id = temp.path().join("$Rfixture.txt").into_os_string();
        item.name = "Shell label".into();
        let (name, deleted) = trash_properties_metadata(&item);
        assert_eq!(std::path::Path::new(&name), original);
        assert_eq!(deleted, Some(1_791_370_123_000));
    }
}
