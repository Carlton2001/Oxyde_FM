use crate::models::{CommandError, SidebarNode};
use crate::utils::path_security::validate_path;
use std::fs;

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
