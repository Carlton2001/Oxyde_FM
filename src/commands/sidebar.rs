use crate::models::{CommandError, SidebarNode};
use crate::utils::path_security::validate_path;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// How an entry is a folder of the tree: `Some(true)` for a link to one (a junction, a
/// folder link, or a symbolic link to a folder under Unix), as in the listing.
fn tree_folder(path: &Path, metadata: &fs::Metadata, is_reparse_point: bool) -> Option<bool> {
    if crate::models::file_entry::is_directory_link(metadata, is_reparse_point) {
        return Some(true);
    }
    if metadata.is_dir() {
        return Some(is_reparse_point);
    }
    // Followed once, for the link itself: a dangling one or a link to a file is not a folder.
    (metadata.is_symlink() && cfg!(unix) && fs::metadata(path).is_ok_and(|target| target.is_dir())).then_some(true)
}

/// Whether `path` holds a folder, a link to one included, to show its arrow.
fn holds_folder(path: &Path) -> bool {
    fs::read_dir(path).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => true,
            Ok(kind) => kind.is_symlink() && cfg!(unix) && entry.path().is_dir(),
            Err(_) => false,
        })
    })
}

pub async fn get_sidebar_nodes(path: String) -> Result<Vec<SidebarNode>, CommandError> {
    get_sidebar_nodes_known(path, std::collections::HashMap::new()).await
}

/// As `get_sidebar_nodes`, but a folder found in `known` keeps the arrow given there
/// instead of being opened again: each opening is a round trip on a network share.
pub async fn get_sidebar_nodes_known(
    path: String,
    known: std::collections::HashMap<String, bool>,
) -> Result<Vec<SidebarNode>, CommandError> {
    tokio::task::spawn_blocking(move || get_sidebar_nodes_blocking(path, &known))
        .await
        .map_err(|e| CommandError::IoError(e.to_string()))?
}

fn get_sidebar_nodes_blocking(
    path: String,
    known: &std::collections::HashMap<String, bool>,
) -> Result<Vec<SidebarNode>, CommandError> {
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
        let node_path = entry.path();
        if tree_folder(&node_path, &metadata, is_reparse_point).is_none() {
            continue;
        }
        let node_path_text = node_path.to_string_lossy().to_string();
        let node_has_subdirs = known.get(&node_path_text).copied().unwrap_or_else(|| holds_folder(&node_path));

        nodes.push(SidebarNode {
            name,
            path: node_path_text,
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
            let node_path = entry.path();
            let Some(link) = tree_folder(&node_path, &metadata, is_reparse_point) else {
                continue;
            };
            let node_has_subdirs = holds_folder(&node_path);

            nodes.push((SidebarNode {
                name,
                path: node_path.to_string_lossy().to_string(),
                is_hidden,
                is_system,
                has_subdirs: node_has_subdirs,
            }, link));
        }

        nodes.sort_by_cached_key(|(node, _)| node.name.to_lowercase());

        // Push subdirs to stack before moving nodes into result. A link keeps its
        // arrow but is not walked into, which could loop; it opens on its own.
        for (node, link) in &nodes {
            if node.has_subdirs
                && !link
                && (show_hidden || !node.is_hidden)
                && (show_system || !node.is_system)
                && !is_bulk_expand_excluded(&node.path, &node.name)
            {
                stack.push(std::path::PathBuf::from(&node.path));
            }
        }

        result.insert(current_path_str, nodes.into_iter().map(|(node, _)| node).collect());
    }

    Ok(result)
}

fn is_bulk_expand_excluded(path: &str, name: &str) -> bool {
    crate::platform::platform().bulk_expand_excluded(path, name)
}

#[cfg(test)]
#[path = "sidebar/tests/expand_all.rs"]
mod expand_all_tests;

#[cfg(all(test, unix))]
#[path = "sidebar/tests/links.rs"]
mod links_tests;
