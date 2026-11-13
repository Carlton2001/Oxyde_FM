//! The drives' folder tree in the places, as the main sidebar unfolds it (`workspace/
//! sidebar.rs`, `navigation.rs`): a drive or a folder holding folders shows a chevron,
//! its folders are listed off the UI thread when it unfolds, and with the main
//! window's path tracking the tree unfolds to the folder open. Its state is the
//! window's own: what unfolds here leaves the main sidebar as it is.
use super::FileChooserWindow;
use crate::ui::components::sidebar::folder_row::{self, FolderRow};
use crate::ui::components::tooltip::hover::SharedTooltipExt;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::paths::paths_match;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::SidebarNode;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct Tree {
    /// The drives and folders unfolded.
    expanded: HashSet<String>,
    /// Each listed folder's folders, starting from those the main window knows.
    nodes: HashMap<String, Vec<SidebarNode>>,
    loading: HashSet<String>,
    /// The main window's path tracking, when the window opened.
    follow: bool,
    follow_task: Option<Task<()>>,
}

impl Tree {
    pub(super) fn new(nodes: HashMap<String, Vec<SidebarNode>>, follow: bool) -> Self {
        Self { nodes, follow, ..Default::default() }
    }
}

/// A folder of the tree as laid out: its node, its depth and whether it is unfolded.
pub(super) struct TreeFolder {
    pub(super) node: SidebarNode,
    pub(super) depth: usize,
    pub(super) expanded: bool,
}

impl FileChooserWindow {
    fn shows_node(&self, node: &SidebarNode) -> bool {
        (self.show_hidden || !node.is_hidden) && (self.show_system || !node.is_system)
    }

    /// Whether `path`, a drive or a folder, holds folders to unfold.
    pub(super) fn tree_expandable(&self, path: &str) -> bool {
        self.tree.nodes.get(path).is_some_and(|nodes| nodes.iter().any(|node| self.shows_node(node)))
    }

    pub(super) fn tree_expanded(&self, path: &str) -> bool {
        self.tree.expanded.contains(path)
    }

    /// The folders shown under the drive `root`, unfolded ones followed by their own.
    pub(super) fn tree_folders(&self, root: &str) -> Vec<TreeFolder> {
        let mut folders = Vec::new();
        if self.tree.expanded.contains(root) {
            self.push_folders(root, 1, &mut folders);
        }
        folders
    }

    fn push_folders(&self, parent: &str, depth: usize, folders: &mut Vec<TreeFolder>) {
        for node in self.tree.nodes.get(parent).into_iter().flatten().filter(|node| self.shows_node(node)) {
            let expanded = self.tree.expanded.contains(&node.path);
            folders.push(TreeFolder { node: node.clone(), depth, expanded });
            if expanded {
                self.push_folders(&node.path, depth + 1, folders);
            }
        }
    }

    /// The chevron's press, or a double press on the row.
    pub(super) fn toggle_tree(&mut self, path: String, cx: &mut Context<Self>) {
        // A hand on the tree stops an unfolding under way.
        self.tree.follow_task = None;
        if !self.tree.expanded.remove(&path) {
            self.tree.expanded.insert(path.clone());
            if !self.tree.nodes.contains_key(&path) {
                self.load_tree(path, cx);
            }
        }
        cx.notify();
    }

    /// Lists `path`'s folders off the UI thread.
    fn load_tree(&mut self, path: String, cx: &mut Context<Self>) {
        if !self.tree.loading.insert(path.clone()) {
            return;
        }
        cx.spawn(async move |this, cx| {
            let result = oxyde_lib::commands::sidebar::get_sidebar_nodes(path.clone()).await;
            let _ = this.update(cx, |this, cx| {
                this.tree.loading.remove(&path);
                match result {
                    Ok(nodes) => { this.tree.nodes.insert(path, nodes); }
                    Err(error) => log::warn!("The file chooser cannot unfold {path}: {error:?}"),
                }
                cx.notify();
            });
        }).detach();
    }

    /// The drives whose folders are not known yet, to tell which have a chevron.
    pub(super) fn load_drive_folders(&mut self, cx: &mut Context<Self>) {
        let missing: Vec<String> = self.places.drives.iter().map(|drive| drive.path.clone())
            .filter(|path| !self.tree.nodes.contains_key(path)).collect();
        for path in missing {
            self.load_tree(path, cx);
        }
    }

    /// The folder open was listed again with `nodes`: the tree shows its folders as they are.
    pub(super) fn update_tree(&mut self, folder: &str, nodes: &[SidebarNode]) {
        if let Some(known) = self.tree.nodes.get_mut(folder) {
            *known = nodes.to_vec();
        }
        // Its chevron, in its parent's list, follows a first folder made or the last removed.
        for node in self.tree.nodes.values_mut().flatten().filter(|node| paths_match(&node.path, folder)) {
            node.has_subdirs = !nodes.is_empty();
        }
    }

    /// With path tracking, unfolds the drive holding the folder open down to its
    /// parent, as the main window's `follow_sidebar_path`, and scrolls it into view.
    pub(super) fn follow_tree(&mut self, cx: &mut Context<Self>) {
        if !self.tree.follow {
            return;
        }
        let folder = self.folder.clone();
        let ancestors: Vec<String> = std::path::Path::new(&folder).ancestors()
            .filter(|path| !path.as_os_str().is_empty())
            .map(|path| path.to_string_lossy().into_owned()).collect();
        // The deepest drive holding it.
        let Some((root, root_index)) = self.places.drives.iter()
            .filter_map(|drive| ancestors.iter().position(|ancestor| paths_match(ancestor, &drive.path)).map(|index| (drive.path.clone(), index)))
            .min_by_key(|(_, index)| *index) else { return };
        let targets: Vec<String> = ancestors[..root_index].iter().rev().cloned().collect();
        let cached: HashMap<String, Vec<SidebarNode>> = std::iter::once(&root).chain(targets.iter()).take(targets.len())
            .filter_map(|parent| self.tree.nodes.get(parent).map(|nodes| (parent.clone(), nodes.clone())))
            .collect();
        self.tree.follow_task = Some(cx.spawn(async move |this, cx| {
            let mut current = root;
            let mut loaded = Vec::new();
            for (index, target) in targets.iter().enumerate() {
                // The folder's own parent is listed again, for a folder just made there.
                let known = if index + 1 < targets.len() { cached.get(&current).cloned() } else { None };
                let nodes = match known {
                    Some(nodes) => nodes,
                    None => match oxyde_lib::commands::sidebar::get_sidebar_nodes(current.clone()).await {
                        Ok(nodes) => nodes,
                        Err(error) => {
                            log::warn!("The file chooser cannot follow {current}: {error:?}");
                            break;
                        }
                    },
                };
                let next = nodes.iter().find(|node| paths_match(&node.path, target)).map(|node| node.path.clone());
                loaded.push((current.clone(), nodes));
                let Some(next) = next else { break };
                current = next;
            }
            let _ = this.update(cx, |this, cx| {
                if this.folder != folder {
                    return;
                }
                for (parent, nodes) in loaded {
                    this.tree.expanded.insert(parent.clone());
                    this.tree.nodes.insert(parent, nodes);
                }
                this.reveal_in_sidebar(&folder);
                cx.notify();
            });
        }));
    }

    /// A folder of the tree, which opens on a press and unfolds on a double press.
    pub(super) fn render_tree_folder(&self, folder: &TreeFolder, owner: Option<&Entity<super::OxydeApp>>, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let node = &folder.node;
        let path = node.path.clone();
        let expandable = node.has_subdirs;
        let row = FolderRow {
            path: &path,
            name: &node.name,
            depth: folder.depth,
            expandable,
            expanded: folder.expanded,
            selected: paths_match(&self.folder, &path),
            dimmed: node.is_hidden || node.is_system,
            use_system_icons: self.system_icons,
        };
        let this = cx.entity().downgrade();
        let toggled = path.clone();
        row.render(SharedString::from(format!("file-chooser-folder-{path}")), theme,
                move |_, cx| { let _ = this.update(cx, |this, cx| this.toggle_tree(toggled.clone(), cx)); })
            .w_full()
            .when_some(owner.cloned(), |row, owner| row.shared_tooltip(owner, super::sidebar::REGION, format!("directory:{path}"), cx,
                Tooltip::with_meta(node.name.clone(), path.clone())))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                if event.click_count == 2 && expandable {
                    this.toggle_tree(path.clone(), cx);
                } else {
                    this.show_folder(path.clone(), cx);
                }
            }))
            .into_any_element()
    }

    /// The width of the tree's widest folder, for the edge's double click.
    pub(super) fn tree_width(&self, labels: &mut crate::ui::components::sidebar::measure::LabelWidths) -> f32 {
        self.places.drives.iter().flat_map(|drive| self.tree_folders(&drive.path))
            .map(|folder| folder_row::chrome(folder.depth) + labels.width(&folder.node.name, false))
            .fold(0.0, f32::max)
    }
}
