use crate::models::{CommandError, SidebarNode, FsChangeEvent, SidebarWatcherState};
use crate::utils::path_security::validate_path;
use std::fs;
use tauri::{AppHandle, Emitter, State};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use log::error;

#[tauri::command]
pub async fn update_sidebar_watchers(
    app: AppHandle,
    state: State<'_, SidebarWatcherState>,
    paths: Vec<String>,
) -> Result<(), CommandError> {
    let mut watcher_lock = state.watcher.lock().map_err(|_| CommandError::SystemError("Failed to lock sidebar watcher".to_string()))?;
    
    // Stop all previous watches by dropping the old watcher
    *watcher_lock = None;
    
    if paths.is_empty() {
        return Ok(());
    }

    let app_clone = app.clone();
    let mut new_watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| match res {
            Ok(event) => {
                let kind = format!("{:?}", event.kind);
                if kind.contains("Access") { return; }
                
                let paths: Vec<String> = event.paths.iter()
                    .map(|p| p.to_string_lossy().replace("\\\\?\\", "").to_string())
                    .collect();
                
                if !paths.is_empty() {
                    let _ = app_clone.emit("fs-change", FsChangeEvent { kind, paths });
                }
            }
            Err(e) => error!("Sidebar watcher error: {}", e),
        },
        Config::default(),
    ).map_err(|e| CommandError::SystemError(format!("Failed to create sidebar watcher: {}", e)))?;

    for path in paths {
        if let Ok(pb) = validate_path(&path) {
            if pb.is_dir() {
                let _ = new_watcher.watch(&pb, RecursiveMode::NonRecursive);
            }
        }
    }

    *watcher_lock = Some(new_watcher);
    Ok(())
}

#[tauri::command]
pub async fn get_sidebar_nodes(path: String) -> Result<Vec<SidebarNode>, CommandError> {
    let pb = validate_path(&path)?;
    
    if !pb.is_dir() {
        return Err(CommandError::PathError(format!("Path is not a directory: {}", path)));
    }

    let entries = fs::read_dir(&pb).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut nodes = Vec::new();

    for entry in entries.filter_map(|e| e.ok()) {
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        if !metadata.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        let (is_hidden, is_system, _) = crate::utils::get_file_attributes(&metadata, &name);
        
        // Efficiently check for subdirectories
        // Detect if directory is protected (Access Denied)
        let mut is_protected = false;
        let mut node_has_subdirs = false;

        let node_path = entry.path();
        match fs::read_dir(&node_path) {
            Ok(sub_entries) => {
                node_has_subdirs = sub_entries.filter_map(|e| e.ok()).any(|sub_entry| {
                    sub_entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                });
            }
            Err(e) => {
                let kind = e.kind();
                if kind == std::io::ErrorKind::PermissionDenied || kind == std::io::ErrorKind::NotFound {
                    is_protected = true;
                }
            }
        }

        nodes.push(SidebarNode {
            name,
            path: node_path.to_string_lossy().to_string(),
            is_hidden,
            is_system,
            is_readonly: metadata.permissions().readonly(),
            is_protected,
            has_subdirs: node_has_subdirs,
        });
    }

    // Sort by name case-insensitive
    nodes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(nodes)
}

#[tauri::command]
pub async fn get_subtree_nodes(path: String) -> Result<std::collections::HashMap<String, Vec<SidebarNode>>, CommandError> {
    let pb = validate_path(&path)?;
    if !pb.is_dir() {
        return Err(CommandError::PathError(format!("Path is not a directory: {}", path)));
    }

    let mut result = std::collections::HashMap::new();
    let mut stack = vec![pb];
    
    while let Some(current_pb) = stack.pop() {
        let current_path_str = current_pb.to_string_lossy().to_string();
        
        let entries = match fs::read_dir(&current_pb) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let mut nodes = Vec::new();
        for entry in entries.filter_map(|e| e.ok()) {
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            if !metadata.is_dir() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            let (is_hidden, is_system, _) = crate::utils::get_file_attributes(&metadata, &name);
            
            let node_path = entry.path();
            let mut node_has_subdirs = false;
            let mut is_protected = false;

            match fs::read_dir(&node_path) {
                Ok(sub_entries) => {
                    node_has_subdirs = sub_entries.filter_map(|e| e.ok()).any(|se| {
                        se.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                    });
                }
                Err(e) => {
                    let kind = e.kind();
                    if kind == std::io::ErrorKind::PermissionDenied || kind == std::io::ErrorKind::NotFound {
                        is_protected = true;
                    }
                }
            }

            nodes.push(SidebarNode {
                name,
                path: node_path.to_string_lossy().to_string(),
                is_hidden,
                is_system,
                is_readonly: metadata.permissions().readonly(),
                is_protected,
                has_subdirs: node_has_subdirs,
            });
        }
        
        nodes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        
        // Push subdirs to stack before moving nodes into result
        for node in &nodes {
            if node.has_subdirs {
                stack.push(std::path::PathBuf::from(&node.path));
            }
        }
        
        result.insert(current_path_str, nodes);
    }

    Ok(result)
}
