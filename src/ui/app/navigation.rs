use crate::ui::components::sidebar::sidebar_paths_equal;
use crate::ui::scale::{logical};
use crate::ui::app::OxydeApp;
use crate::ui::components::input::{InputAppearance, TextInput, TextInputOptions};
use crate::ui::i18n::{I18nContext, Message};
use gpui::*;
use oxyde_lib::commands::io::list_dir_unsorted as list_dir;

mod trash_loading;

mod breadcrumb;

impl OxydeApp {
    /// Refresh the file list of a specific tab without disturbing its scroll / selection / filters.
    pub fn refresh_tab_files(
        &mut self,
        pane_id: &str,
        tab_idx: usize,
        path: String,
        cx: &mut Context<Self>,
    ) {
        self.request_tab_files(pane_id, tab_idx, path, false, cx);
    }

    pub(crate) fn follow_active_path(&mut self, cx: &mut Context<Self>) {
        if !self.follows_active_path() || self.sidebar_collapsed || self.state.drives.is_empty() {
            self.sidebar_follow_task = None;
            self.sidebar_follow_path = None;
            return;
        }
        let Some(path) = self.state.find_pane(&self.state.active_pane_id)
            .map(|pane| pane.active_tab().path.clone()) else { return; };
        if self.sidebar_follow_path.as_ref() == Some(&path) { return; }
        self.sidebar_follow_task = None;
        self.sidebar_follow_path = Some(path.clone());
        let ancestors: Vec<String> = std::path::Path::new(&path).ancestors()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.to_string_lossy().into_owned()).collect();
        let Some((root, root_index)) = self.state.drives.iter().find_map(|drive| {
            ancestors.iter().position(|ancestor| sidebar_paths_equal(ancestor, &drive.path))
                .map(|index| (drive.path.clone(), index))
        }) else { return; };
        let targets: Vec<String> = ancestors[..root_index].iter().rev().cloned().collect();
        let cached: std::collections::HashMap<_, _> = std::iter::once(&root)
            .chain(targets.iter()).take(targets.len())
            .filter_map(|parent| self.state.sidebar_nodes.get(parent)
                .map(|nodes| (parent.clone(), nodes.clone())))
            .collect();
        self.sidebar_follow_task = Some(cx.spawn(async move |this, cx| {
            let mut current = root;
            let mut loaded = Vec::new();
            for (index, target) in targets.iter().enumerate() {
                // Refresh the immediate parent so newly created folders can be revealed.
                let nodes = if index + 1 < targets.len() {
                    cached.get(&current).cloned()
                } else { None };
                let nodes = match nodes {
                    Some(nodes) => nodes,
                    None => match oxyde_lib::commands::sidebar::get_sidebar_nodes(current.clone()).await {
                        Ok(nodes) => nodes,
                        Err(error) => {
                            log::warn!("Cannot follow sidebar path {current}: {error:?}");
                            break;
                        }
                    },
                };
                let next = nodes.iter().find(|node| sidebar_paths_equal(&node.path, target))
                    .map(|node| node.path.clone());
                loaded.push((current.clone(), nodes));
                let Some(next) = next else { break; };
                current = next;
            }
            let _ = this.update(cx, |this, cx| {
                if !this.follows_active_path() || this.sidebar_collapsed
                    || this.sidebar_follow_path.as_ref() != Some(&path)
                    || this.state.find_pane(&this.state.active_pane_id)
                        .is_none_or(|pane| pane.active_tab().path != path) {
                    return;
                }
                for (parent, nodes) in loaded {
                    this.state.expanded_paths.insert(parent.clone());
                    this.state.sidebar_nodes.insert(parent, nodes);
                }
                this.state.refresh_sidebar();
                if let Some(index) = sidebar_reveal_index(
                    &this.state.sidebar_items, &path, this.show_hidden, this.show_system, this.show_network(),
                ) {
                    this.sidebar_selected_path = Some(current);
                    let visible: Vec<_> = this.state.sidebar_items.iter().filter(|item| crate::ui::components::sidebar::sidebar_item_visible(item, this.show_hidden, this.show_system, this.show_network())).collect();
                    let top: f32 = visible.iter().take(index).map(|item| crate::ui::components::sidebar::sidebar_item_height(item)).sum();
                    let height = visible.get(index).map(|item| crate::ui::components::sidebar::sidebar_item_height(item)).unwrap_or(0.0);
                    let scroll = this.sidebar_scroll.0.borrow().base_handle.clone();
                    let viewport = scroll.bounds().size.height.as_f32();
                    scroll.set_offset(point(scroll.offset().x, px(-(top + height / 2.0 - viewport / 2.0).max(0.0))));
                }
                this.reconcile_watchers(cx);
                cx.notify();
            });
        }));
    }

    pub(crate) fn sidebar_display_width(&self) -> f32 {
        if self.sidebar_collapsed {
            logical(48.0)
        } else {
            logical(self.sidebar_width)
        }
    }

    pub(crate) fn toggle_sidebar_panel(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        self.pinned_rail_open = false;
        self.context_menus.clear();
        self.save_workspace(cx);
        cx.notify();
    }

    pub(crate) fn resize_sidebar(&mut self, width: f32) {
        let baseline_width = width / logical(1.0);
        resize_sidebar_state(&mut self.sidebar_width, &mut self.sidebar_collapsed, baseline_width);
    }

    pub(crate) fn expand_all_sidebar_path(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(handle) = self.handle.clone() else {
            return;
        };
        let show_hidden = self.show_hidden;
        let show_system = self.show_system;
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                if let Ok(tree) = oxyde_lib::commands::sidebar::get_subtree_nodes(
                    path.clone(),
                    show_hidden,
                    show_system,
                )
                .await
                {
                    handle.update(&mut cx, move |this, cx| {
                        this.state.expanded_paths.insert(path);
                        for (parent, nodes) in &tree {
                            if nodes.iter().any(|node| node.has_subdirs) {
                                this.state.expanded_paths.insert(parent.clone());
                            }
                        }
                        this.state.sidebar_nodes.extend(tree);
                        this.state.refresh_sidebar();
                        this.reconcile_watchers(cx);
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub(crate) fn activate_pane(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        if self.state.find_pane(pane_id).is_some()
            && self.state.active_pane_id != pane_id
            && !self.state.is_viewer_pane(pane_id)
        {
            self.state.active_pane_id = pane_id.to_string();
            self.save_workspace(cx);
        }
    }

    pub fn toggle_sidebar_path(&mut self, path: String, cx: &mut Context<Self>) {
        // A manual tree interaction takes precedence over an in-flight reveal.
        self.sidebar_follow_task = None;
        self.state.toggle_sidebar_path(path.clone());
        self.reconcile_watchers(cx);
        cx.notify();
        self.request_sidebar_nodes(path, false, cx);
    }

    pub(crate) fn request_sidebar_nodes(
        &mut self,
        path: String,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        let is_drive = self.state.drives.iter().any(|drive| drive.path == path);
        if self.ejecting_drive { return; }
        if (!self.state.expanded_paths.contains(&path) && !is_drive)
            || (!force && self.state.sidebar_nodes.contains_key(&path))
            || (!force && self.pending_sidebar_requests.contains_key(&path))
        {
            return;
        }
        self.next_file_request = self.next_file_request.wrapping_add(1);
        let request_id = self.next_file_request;
        self.pending_sidebar_requests
            .insert(path.clone(), request_id);
        let Some(handle) = self.handle.clone() else {
            return;
        };
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = oxyde_lib::commands::sidebar::get_sidebar_nodes(path.clone()).await;
                handle.update(&mut cx, move |this, cx| {
                    if this.pending_sidebar_requests.get(&path) != Some(&request_id) {
                        return;
                    }
                    this.pending_sidebar_requests.remove(&path);
                    match result {
                        Ok(nodes) if this.state.expanded_paths.contains(&path)
                            || this.state.drives.iter().any(|drive| drive.path == path) => {
                            this.state.sidebar_nodes.insert(path, nodes);
                            this.state.refresh_sidebar();
                            this.reconcile_watchers(cx);
                            cx.notify();
                        }
                        Err(error) => {
                            this.state.sidebar_nodes.remove(&path);
                            this.state.refresh_sidebar();
                            cx.notify();
                            log::warn!("Cannot expand sidebar path {}: {error:?}", path);
                        }
                        Ok(_) => {}
                    }
                });
            }
        })
        .detach();
    }

    pub fn navigate_to(&mut self, pane_id: &str, path: String, cx: &mut Context<Self>) {
        if path == "viewer://" { self.open_viewer(cx); return; }
        if self.state.find_pane(pane_id).is_some_and(|pane| pane.active_tab().is_viewer()) {
            self.add_panel_right(cx);
            let target = self.state.active_pane_id.clone();
            self.load_path(&target, path, cx);
            return;
        }
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let active_tab = pane.active_tab_mut();
            if path == active_tab.path {
                return;
            }
            let old_path = active_tab.path.clone();
            active_tab.back_stack.push(old_path);
            active_tab.forward_stack.clear();
            self.load_path(pane_id, path, cx);
        }
    }

    pub fn go_back(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let active_tab = pane.active_tab_mut();
            if let Some(path) = active_tab.back_stack.pop() {
                let current = active_tab.path.clone();
                active_tab.forward_stack.push(current);
                self.load_path(pane_id, path, cx);
            }
        }
    }

    pub fn go_forward(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let active_tab = pane.active_tab_mut();
            if let Some(path) = active_tab.forward_stack.pop() {
                let current = active_tab.path.clone();
                active_tab.back_stack.push(current);
                self.load_path(pane_id, path, cx);
            }
        }
    }

    pub fn add_tab(&mut self, pane_id: &str, path: String, activate: bool, cx: &mut Context<Self>) {
        if path == "viewer://" { self.open_viewer(cx); return; }
        if self.state.find_pane(pane_id).is_some_and(|pane| pane.active_tab().is_viewer()) {
            self.navigate_to(pane_id, path, cx);
            return;
        }
        let tab_idx = self.state.find_pane(pane_id)
            .map(|pane| (pane.active_tab_index + 1).min(pane.tabs.len()));
        self.state.add_tab(pane_id, path, activate);
        if let (Some(pane), Some(tab_idx)) = (self.state.find_pane(pane_id), tab_idx) {
            let path = pane.tabs[tab_idx].path.clone();
            self.request_tab_files(pane_id, tab_idx, path, false, cx);
        }
        self.reconcile_watchers(cx);
        self.save_workspace(cx);
        cx.notify();
    }

    /// Duplicate a tab and populate the new tab's file list.
    ///
    /// `WorkspaceState::duplicate_tab` only updates the synchronous UI state;
    /// directory contents are loaded through `request_tab_files`.
    pub fn duplicate_tab(&mut self, pane_id: &str, tab_idx: usize, cx: &mut Context<Self>) {
        if self.state.find_pane(pane_id).and_then(|pane| pane.tabs.get(tab_idx)).is_some_and(|tab| tab.is_viewer()) {
            self.open_viewer(cx);
            return;
        }
        if let Some(new_tab_idx) = self.state.duplicate_tab(pane_id, tab_idx) {
            let path = self
                .state
                .find_pane(pane_id)
                .map(|pane| pane.tabs[new_tab_idx].path.clone());
            if let Some(path) = path {
                self.request_tab_files(pane_id, new_tab_idx, path, false, cx);
            }
        }

        self.reconcile_watchers(cx);
        self.save_workspace(cx);
        cx.notify();
    }

    pub fn switch_tab(&mut self, pane_id: &str, tab_idx: usize, cx: &mut Context<Self>) {
        if self.breadcrumb_pane_id.as_deref() == Some(pane_id) {
            self.cancel_breadcrumb_edit(cx);
        }
        let mut load_path = None;
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            if tab_idx >= pane.tabs.len() {
                return;
            }
            pane.active_tab_index = tab_idx;
            if matches!(
                pane.active_tab().load_status,
                crate::ui::workspace::TabLoadStatus::Loading
            ) && pane.active_tab().files.is_empty()
                && pane.active_tab().quick_search.is_none()
            {
                load_path = Some(pane.active_tab().path.clone());
            }
            if !pane.active_tab().is_viewer() {
                self.state.active_pane_id = pane_id.to_string();
            }
        }
        if let Some(path) = load_path {
            self.request_tab_files(pane_id, tab_idx, path, false, cx);
        }
        self.save_workspace(cx);
        cx.notify();
    }

    pub fn close_tab(&mut self, pane_id: &str, tab_idx: usize, cx: &mut Context<Self>) {
        self.state.close_tab_id(pane_id, tab_idx);
        let reload = self.state.find_pane(pane_id).and_then(|pane| {
            (pane.active_tab().files.is_empty()
                && matches!(
                    pane.active_tab().load_status,
                    crate::ui::workspace::TabLoadStatus::Loading
                ) && pane.active_tab().quick_search.is_none())
            .then(|| (pane.active_tab_index, pane.active_tab().path.clone()))
        });
        if let Some((index, path)) = reload {
            self.request_tab_files(pane_id, index, path, false, cx);
        }
        self.reconcile_watchers(cx);
        self.save_workspace(cx);
        cx.notify();
    }

    pub fn close_other_tabs(&mut self, pane_id: &str, tab_idx: usize, cx: &mut Context<Self>) {
        self.state.close_other_tabs(pane_id, tab_idx);
        self.reconcile_watchers(cx);
        self.save_workspace(cx);
        cx.notify();
    }

    pub fn load_path(&mut self, pane_id: &str, path: String, cx: &mut Context<Self>) {
        if path == "viewer://" { return; }
        if self.state.find_pane(pane_id).is_some_and(|pane| pane.active_tab().is_viewer()) { return; }
        if self.breadcrumb_pane_id.as_deref() == Some(pane_id) {
            self.cancel_breadcrumb_edit(cx);
        }
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab_idx = pane.active_tab_index;
            let active_tab = pane.active_tab_mut();
            active_tab.size_calculation = None;
            active_tab.show_size_histogram = false;
            active_tab.path = path.clone();
            active_tab.archive_root = None;
            active_tab.search_revision = active_tab.search_revision.wrapping_add(1);
            active_tab.search_input = None;
            active_tab.search_text.clear();
            active_tab.pending_reveal = None;
            if let Some(search) = active_tab.quick_search.take() {
                search.cancellation.0.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            active_tab.clear_files();
            active_tab.filter_types.clear();
            active_tab.filter_name.clear();
            active_tab.filter_original_location.clear();
            active_tab.filter_sizes.clear();
            active_tab.filter_dates.clear();
            active_tab.selected_paths.clear();
            active_tab.selection_anchor = None;
            active_tab.load_status = crate::ui::workspace::TabLoadStatus::Loading;
            active_tab.list_scroll = UniformListScrollHandle::new();
            active_tab.list_scroll_h.set_offset(point(px(0.0), px(0.0)));
            self.request_tab_files(pane_id, tab_idx, path, true, cx);
            self.reconcile_watchers(cx);
            self.save_workspace(cx);
            cx.notify();
        }
    }

    pub(crate) fn request_tab_files(
        &mut self,
        pane_id: &str,
        tab_idx: usize,
        path: String,
        reset_scroll: bool,
        cx: &mut Context<Self>,
    ) {
        if path == "viewer://" { return; }
        if path.starts_with("search://") {
            self.request_quick_search(pane_id, tab_idx, path, cx);
            return;
        }
        let Some(tab_id) = self
            .state
            .find_pane(pane_id)
            .and_then(|pane| pane.tabs.get(tab_idx))
            .filter(|tab| tab.path == path)
            .map(|tab| tab.id.clone())
        else {
            return;
        };
        let Some(handle) = self.handle.clone() else {
            return;
        };
        if let Some(tab) = self.state.find_tab_mut(&tab_id) {
            tab.size_calculation = None;
        }
        self.next_file_request = self.next_file_request.wrapping_add(1);
        let request_id = self.next_file_request;
        self.pending_file_requests
            .insert(tab_id.clone(), request_id);
        let show_hidden = self.show_hidden;
        let show_system = self.show_system;
        let pane_width = self.pane_sizes.get(pane_id).map(|size| size.width).unwrap_or(self.list_width);
        if path == "trash://" {
            self.request_trash_files(tab_id, request_id, pane_width, reset_scroll, cx);
            return;
        }
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let context_path = path.clone();
                let archive_root = tokio::task::spawn_blocking(move || {
                    oxyde_lib::utils::archive::resolve_virtual_path(std::path::Path::new(&context_path))
                        .map(|(archive, _)| archive)
                }).await.unwrap_or(None);
                let result = list_dir(
                    path.clone(),
                    Some(show_hidden),
                    Some(show_system),
                )
                .await;
                let result = match result {
                    Ok(entries) => {
                        let owner = super::file_preparation::Owner::Listing {
                            id: tab_id.clone(), path: path.clone(), request: request_id,
                        };
                        // Network-provider listings did not receive the old natural
                        // pre-sort. Keep their UI ties in provider arrival order.
                        let update = if cfg!(target_os = "windows")
                            && (path == "__network_vincinity__"
                                || oxyde_lib::commands::io::is_network_server(&path))
                        {
                            crate::ui::workspace::FileUpdate::Replace(entries.into())
                        } else {
                            crate::ui::workspace::FileUpdate::Listing(entries.into())
                        };
                        if !super::file_preparation::prepare(&handle, &mut cx, &owner, update).await {
                            return;
                        }
                        Ok(())
                    }
                    Err(error) => Err(error),
                };
                let handle_for_subdirectory_check = handle.clone();
                handle.update(&mut cx, move |this, cx| {
                    if this.pending_file_requests.get(&tab_id) != Some(&request_id) {
                        return;
                    }
                    this.pending_file_requests.remove(&tab_id);
                    // A tab can be moved to another pane while its listing is in flight.
                    // Its stable id, not the pane captured at request time, owns the result.
                    let mut refresh_subdirectory_flags = false;
                    if let Some(tab) = this.state.find_tab_mut(&tab_id) {
                        if tab.path != path {
                            return;
                        }
                        match result {
                            Ok(()) => {
                                tab.archive_root = archive_root;
                                tab.load_status = crate::ui::workspace::TabLoadStatus::Ready;
                                refresh_subdirectory_flags = path != "trash://";
                                if reset_scroll {
                                    tab.list_scroll = UniformListScrollHandle::new();
                                }
                                tab.reveal_pending_item(pane_width);
                            }
                            Err(error) => {
                                tab.clear_files();
                                tab.load_status = crate::ui::errors::path_load_status(&error);
                                log::warn!("Cannot list {}: {error:?}", path);
                            }
                        }
                        cx.notify();
                    }
                    this.reconcile_watchers(cx);
                    if refresh_subdirectory_flags {
                        let handle = handle_for_subdirectory_check.clone();
                        let tab_id = tab_id.clone();
                        let directory = path.clone();
                        cx.spawn(move |_, cx: &mut AsyncApp| {
                            let mut cx = cx.clone();
                            async move {
                                if let Ok(nodes) = oxyde_lib::commands::sidebar::get_sidebar_nodes(
                                    directory.clone(),
                                )
                                .await
                                {
                                    handle.update(&mut cx, |this, cx| {
                                        let tab_is_still_here = this
                                            .state
                                            .find_tab_mut(&tab_id)
                                            .map(|tab| tab.path == directory)
                                            .unwrap_or(false);
                                        if !tab_is_still_here {
                                            return;
                                        }
                                        for node in nodes {
                                            std::sync::Arc::make_mut(&mut this.folder_has_subdirs)
                                                .insert(node.path, node.has_subdirs);
                                        }
                                        cx.notify();
                                    });
                                }
                            }
                        })
                        .detach();
                    }
                });
            }
        })
        .detach();
    }
    pub fn refresh_all(&mut self, cx: &mut Context<Self>) {
        // Refreshing should update filesystem contents without resetting the users'
        // filters, selection, scroll position, or loading state on each tab.
        let mut requests = Vec::new();
        self.state.layout.visit_panes(&mut |pane| {
            requests.push((pane.id.clone(), pane.active_tab_index, pane.active_tab().path.clone()));
        });
        for (pane_id, index, path) in requests {
            self.request_tab_files(&pane_id, index, path, false, cx);
        }
        self.state.refresh_sidebar();
        cx.notify();
    }

    pub fn add_panel_right(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.state.add_panel_right() {
            let path = self.state.find_pane(&id).unwrap().active_tab().path.clone();
            self.request_tab_files(&id, 0, path, false, cx);
            self.reconcile_watchers(cx);
            self.save_workspace(cx);
            cx.notify();
        }
    }

    pub fn refresh_all_tabs(&mut self, cx: &mut Context<Self>) {
        if self.breadcrumb_editing {
            if let Some(input) = self.editing_view.clone() {
                input.update(cx, |input, cx| input.set_path_visibility(self.show_hidden, self.show_system, cx));
            }
        }
        let mut requests = Vec::new();
        self.state.layout.visit_panes(&mut |pane| {
            for (index, tab) in pane.tabs.iter().enumerate() {
                requests.push((pane.id.clone(), index, tab.path.clone()));
            }
        });
        for (pane_id, index, path) in requests {
            self.request_tab_files(&pane_id, index, path, false, cx);
        }
        self.state.refresh_sidebar();
        cx.notify();
    }
}

fn sidebar_reveal_index(
    items: &[crate::ui::workspace::SidebarItem],
    path: &str,
    show_hidden: bool,
    show_system: bool,
    show_network: bool,
) -> Option<usize> {
    use crate::ui::workspace::SidebarItem;
    items.iter()
        .filter(|item| crate::ui::components::sidebar::sidebar_item_visible(item, show_hidden, show_system, show_network))
        .position(|item| match item {
            SidebarItem::Drive { drive, .. } => sidebar_paths_equal(&drive.path, path),
            SidebarItem::Directory { node, .. } => sidebar_paths_equal(&node.path, path),
            _ => false,
        })
}

fn resize_sidebar_state(saved_width: &mut f32, collapsed: &mut bool, requested_width: f32) {
    if requested_width < 120.0 {
        *collapsed = true;
    } else {
        *collapsed = false;
        *saved_width = requested_width.clamp(150.0, 600.0);
    }
}

#[cfg(test)]
mod sidebar_tests {
    use super::{resize_sidebar_state, sidebar_paths_equal};

    #[test]
    fn reveal_uses_visible_rows_instead_of_unfiltered_tree_indices() {
        use crate::ui::{i18n::Message, workspace::SidebarItem};
        let directory = |path: &str, hidden, system| SidebarItem::Directory {
            node: oxyde_lib::models::SidebarNode {
                name: path.into(), path: path.into(), is_hidden: hidden, is_system: system,
                has_subdirs: false,
            },
            depth: 1, is_expanded: false,
        };
        let items = vec![
            SidebarItem::Header(Message::SidebarQuickAccess),
            SidebarItem::Header(Message::SidebarDrives),
            directory("/hidden", true, false),
            directory("/system", false, true),
            directory("/current", false, false),
        ];
        assert_eq!(super::sidebar_reveal_index(&items, "/current", false, false, true), Some(1));
        assert_eq!(super::sidebar_reveal_index(&items, "/current", true, false, true), Some(2));
        assert_eq!(super::sidebar_reveal_index(&items, "/current", true, true, true), Some(3));
        assert_eq!(super::sidebar_reveal_index(&items, "/missing", true, true, true), None);
    }

    #[test]
    fn network_visibility_keeps_reveal_indices_in_sync() {
        use crate::ui::workspace::SidebarItem;
        let items = vec![
            SidebarItem::Pinned { item: oxyde_lib::models::QuickAccessItem {
                name: "Network".into(), path: "__network_vincinity__".into(),
            } },
            SidebarItem::Directory {
                node: oxyde_lib::models::SidebarNode {
                    name: "current".into(), path: "/current".into(), is_hidden: false,
                    is_system: false, has_subdirs: false,
                }, depth: 1, is_expanded: false,
            },
        ];
        assert_eq!(super::sidebar_reveal_index(&items, "/current", false, false, true), Some(1));
        assert_eq!(super::sidebar_reveal_index(&items, "/current", false, false, false), Some(0));
    }

    #[test]
    fn sidebar_path_matching_respects_platform_case_and_separators() {
        assert!(sidebar_paths_equal("/home/user/", "/home/user"));
        assert!(!sidebar_paths_equal("/home/user", "/home/username"));
        #[cfg(target_os = "windows")]
        {
            assert!(sidebar_paths_equal("D:/Music/", "d:\\music"));
            assert!(sidebar_paths_equal("D:/", "d:\\"));
        }
        #[cfg(not(target_os = "windows"))]
        assert!(!sidebar_paths_equal("/home/User", "/home/user"));
    }

    #[test]
    fn dragging_beyond_minimum_collapses_and_preserves_restore_width() {
        let mut width = 260.0;
        let mut collapsed = false;
        resize_sidebar_state(&mut width, &mut collapsed, 140.0);
        assert_eq!(width, 150.0);
        assert!(!collapsed);
        resize_sidebar_state(&mut width, &mut collapsed, 119.0);
        assert!(collapsed);
        assert_eq!(width, 150.0);
        resize_sidebar_state(&mut width, &mut collapsed, 280.0);
        assert!(!collapsed);
        assert_eq!(width, 280.0);
        resize_sidebar_state(&mut width, &mut collapsed, 900.0);
        assert_eq!(width, 600.0);
    }
}
