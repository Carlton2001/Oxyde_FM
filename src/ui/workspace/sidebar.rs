use super::{SidebarItem, WorkspaceState};
use crate::ui::i18n::Message;
use oxyde_lib::models::QuickAccessItem;

impl WorkspaceState {
    pub fn refresh_sidebar(&mut self) {
        let mut new_items = Vec::new();

        // As in Nautilus, the home folder heads the bookmarks without being one of them.
        #[cfg(not(target_os = "windows"))]
        let home = dirs::home_dir().map(|home| QuickAccessItem { name: String::new(), path: home.to_string_lossy().into_owned() });
        #[cfg(target_os = "windows")]
        let home: Option<QuickAccessItem> = None;
        if home.is_some() || !self.pinned_items.is_empty() {
            new_items.push(SidebarItem::Header(Message::SidebarQuickAccess));
            for item in home.iter().chain(&self.pinned_items) {
                new_items.push(SidebarItem::Pinned { item: item.clone() });
            }
        }

        for item in &self.onedrive_items {
            new_items.push(SidebarItem::OneDrive { item: item.clone() });
        }
        new_items.push(SidebarItem::Header(Message::SidebarDrives));

        for drive in &self.drives {
            let is_expanded = self.expanded_paths.contains(&drive.path);
            new_items.push(SidebarItem::Drive {
                drive: drive.clone(),
                is_expanded,
            });

            if is_expanded {
                self.populate_subdir(&drive.path, 1, &mut new_items);
            }
        }
        for volume in &self.unmounted_volumes {
            new_items.push(SidebarItem::Volume { volume: volume.clone() });
        }

        new_items.push(SidebarItem::Header(Message::SidebarLocations));
        #[cfg(target_os = "windows")]
        new_items.push(SidebarItem::Pinned {
            item: QuickAccessItem {
                name: "Réseau".to_string(),
                path: "__network_vincinity__".to_string(),
            },
        });
        new_items.push(SidebarItem::Pinned {
            item: QuickAccessItem {
                name: "Corbeille".to_string(),
                path: "trash://".to_string(),
            },
        });

        self.sidebar_items = new_items;
    }

    fn populate_subdir(&self, path: &str, depth: usize, items: &mut Vec<SidebarItem>) {
        if let Some(nodes) = self.sidebar_nodes.get(path) {
            for node in nodes {
                let is_expanded = self.expanded_paths.contains(&node.path);
                let path_copy = node.path.clone();
                items.push(SidebarItem::Directory {
                    node: node.clone(),
                    depth,
                    is_expanded,
                });

                if is_expanded {
                    self.populate_subdir(&path_copy, depth + 1, items);
                }
            }
        }
    }

    pub fn toggle_sidebar_path(&mut self, path: String) {
        if self.expanded_paths.contains(&path) {
            self.expanded_paths.remove(&path);
        } else {
            self.expanded_paths.insert(path);
        }
        self.refresh_sidebar();
    }

    pub fn _collapse_all_sidebar_path(&mut self, path: String) {
        let norm_path = path.replace("\\", "/");
        let target_prefix = if norm_path.ends_with("/") {
            norm_path.clone()
        } else {
            format!("{}/", norm_path)
        };
        self.expanded_paths.retain(|p| {
            let norm_p = p.replace("\\", "/");
            norm_p != norm_path && !norm_p.starts_with(&target_prefix)
        });
        self.refresh_sidebar();
    }
}

#[cfg(test)]
#[path = "tests/onedrive.rs"]
mod onedrive_tests;
