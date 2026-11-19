use crate::models::{CommandError, SidebarNode};
use crate::utils::path_security::validate_path;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub async fn get_sidebar_nodes(path: String) -> Result<Vec<SidebarNode>, CommandError> {
    tokio::task::spawn_blocking(move || get_sidebar_nodes_blocking(path))
        .await
        .map_err(|e| CommandError::IoError(e.to_string()))?
}

fn get_sidebar_nodes_blocking(path: String) -> Result<Vec<SidebarNode>, CommandError> {
    let pb = validate_path(&path)?;

    if !pb.is_dir() {
        return Err(CommandError::PathError(format!(
            "Path is not a directory: {}",
            path
        )));
    }

    let entries = fs::read_dir(&pb).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut nodes = Vec::new();
    let attributes = crate::utils::visibility::DirectoryAttributes::load(&pb);

    for entry in entries.filter_map(|e| e.ok()) {
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let name = entry.file_name().to_string_lossy().to_string();
        let (is_hidden, is_system, is_reparse_point) = attributes.of(&metadata, &name);
        // A junction or a folder link is a folder of the tree, as in the listing.
        if !metadata.is_dir() && !crate::models::file_entry::is_directory_link(&metadata, is_reparse_point) {
            continue;
        }

        // Efficiently check for subdirectories
        let mut node_has_subdirs = false;

        let node_path = entry.path();
        if let Ok(sub_entries) = fs::read_dir(&node_path) {
            node_has_subdirs = sub_entries
                .filter_map(|e| e.ok())
                .any(|sub_entry| sub_entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false));
        }

        nodes.push(SidebarNode {
            name,
            path: node_path.to_string_lossy().to_string(),
            is_hidden,
            is_system,
            has_subdirs: node_has_subdirs,
        });
    }

    // Sort by name case-insensitive
    nodes.sort_by_cached_key(|node| node.name.to_lowercase());

    Ok(nodes)
}

/// Reads the folders under `path` for « Expand all ». Setting `cancel` stops the walk
/// between two folders and returns an error.
pub async fn get_subtree_nodes(
    path: String,
    show_hidden: bool,
    show_system: bool,
    cancel: Arc<AtomicBool>,
) -> Result<std::collections::HashMap<String, Vec<SidebarNode>>, CommandError> {
    // The path checks touch the file system, which may stall on a disconnected share.
    tokio::task::spawn_blocking(move || {
        let pb = validate_path(&path)?;
        if !pb.is_dir() {
            return Err(CommandError::PathError(format!(
                "Path is not a directory: {}",
                path
            )));
        }
        get_subtree_nodes_blocking(pb, show_hidden, show_system, &cancel)
    })
        .await
        .map_err(|error| CommandError::IoError(error.to_string()))?
}

/// Folders read by one « Expand all », so a root of the whole disk stays bounded.
const MAX_BULK_FOLDERS: usize = 20_000;

fn get_subtree_nodes_blocking(
    pb: std::path::PathBuf,
    show_hidden: bool,
    show_system: bool,
    cancel: &AtomicBool,
) -> Result<std::collections::HashMap<String, Vec<SidebarNode>>, CommandError> {
    let cancelled = || CommandError::Other("Expand all cancelled".into());
    let mut result = std::collections::HashMap::new();
    let mut stack = vec![pb];

    while let Some(current_pb) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Err(cancelled());
        }
        // « Expand all » on a huge tree returns what it has found so far.
        if result.len() >= MAX_BULK_FOLDERS {
            break;
        }
        let current_path_str = current_pb.to_string_lossy().to_string();

        let entries = match fs::read_dir(&current_pb) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let mut nodes = Vec::new();
        let attributes = crate::utils::visibility::DirectoryAttributes::load(&current_pb);
        for entry in entries.filter_map(|e| e.ok()) {
            // Each folder also lists its children: a large one takes a while.
            if cancel.load(Ordering::Relaxed) {
                return Err(cancelled());
            }
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let name = entry.file_name().to_string_lossy().to_string();
            let (is_hidden, is_system, is_reparse_point) =
                attributes.of(&metadata, &name);
            if !metadata.is_dir() && !crate::models::file_entry::is_directory_link(&metadata, is_reparse_point) {
                continue;
            }

            let node_path = entry.path();
            let mut node_has_subdirs = false;

            if !is_reparse_point {
                if let Ok(sub_entries) = fs::read_dir(&node_path) {
                    node_has_subdirs = sub_entries
                        .filter_map(|e| e.ok())
                        .any(|se| se.file_type().map(|ft| ft.is_dir()).unwrap_or(false));
                }
            }

            nodes.push(SidebarNode {
                name,
                path: node_path.to_string_lossy().to_string(),
                is_hidden,
                is_system,
                has_subdirs: node_has_subdirs,
            });
        }

        nodes.sort_by_cached_key(|node| node.name.to_lowercase());

        // Push subdirs to stack before moving nodes into result
        for node in &nodes {
            if node.has_subdirs
                && (show_hidden || !node.is_hidden)
                && (show_system || !node.is_system)
                && !is_bulk_expand_excluded(&node.path, &node.name)
            {
                stack.push(std::path::PathBuf::from(&node.path));
            }
        }

        result.insert(current_path_str, nodes);
    }

    Ok(result)
}

fn is_bulk_expand_excluded(path: &str, name: &str) -> bool {
    crate::platform::platform().bulk_expand_excluded(path, name)
}

#[cfg(test)]
#[path = "sidebar/tests/expand_all.rs"]
mod expand_all_tests;
