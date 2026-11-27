use super::windows_onedrive::{open, string, subkey_names};
use crate::models::{FileEntry, SidebarNode};
use windows::core::w;
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

/// `\\wsl.localhost` came with Windows 11; the WSL built into Windows 10 (LTSC
/// 2021 included) answers only `\\wsl$`, which every version still accepts.
fn share_root() -> &'static str {
    let build = open(HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        .and_then(|key| string(key.0, w!("CurrentBuildNumber")))
        .and_then(|build| build.trim().parse::<u32>().ok());
    if build.is_some_and(|build| build < 22000) { super::WSL_LEGACY_ROOT } else { super::WSL_ROOT }
}

fn node(root: &str, name: String) -> Option<SidebarNode> {
    if name.is_empty() || name.contains(['\\', '/']) {
        return None;
    }
    // Reaching a share starts its distribution: stay on the registered names.
    Some(SidebarNode {
        path: format!(r"{root}\{name}"),
        name,
        is_hidden: false,
        is_system: false,
        has_subdirs: true,
    })
}

/// Registered distributions, as Explorer lists them under "Linux", running or not.
pub(super) fn distributions() -> Vec<SidebarNode> {
    let Some(root) = open(HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Lxss") else {
        return Vec::new();
    };
    let share = share_root();
    let mut nodes: Vec<_> = subkey_names(root.0).into_iter()
        .filter_map(|id| open(root.0, &id))
        .filter_map(|key| string(key.0, w!("DistributionName")))
        .filter_map(|name| node(share, name))
        .collect();
    nodes.sort_by_key(|node| node.name.to_lowercase());
    nodes
}

/// The WSL provider refuses to enumerate its root, unlike a network server.
pub(super) fn entries() -> Vec<FileEntry> {
    distributions().into_iter().map(|node| FileEntry {
        native_kind: None,
        created: None,
        content_stamp: None,
        archive_path: None,
        network_details: None,
        name: node.name,
        path: node.path,
        is_dir: true,
        is_hidden: false,
        is_system: false,
        is_symlink: false,
        is_broken_link: false,
        is_junction: false,
        is_cloud_placeholder: false,
        cloud_status: None,
        size: 0,
        modified: 0,
        is_calculated: true,
        folders_count: None,
        files_count: None,
        original_path: None,
        deleted_time: None,
        mode: None,
    }).collect()
}

#[cfg(test)]
#[path = "windows_wsl/tests/names.rs"]
mod tests;
