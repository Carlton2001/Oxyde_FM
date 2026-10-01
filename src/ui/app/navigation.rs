use crate::ui::app::OxydeApp;
use gpui::*;
use oxyde_lib::commands::io::list_dir;

impl OxydeApp {
    pub(crate) fn sidebar_display_width(&self) -> f32 {
        if self.sidebar_collapsed {
            48.0
        } else {
            self.sidebar_width
        }
    }

    pub(crate) fn toggle_sidebar_panel(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        self.context_menus.clear();
        self.save_workspace(cx);
        cx.notify();
    }

    pub(crate) fn resize_sidebar(&mut self, width: f32) {
        resize_sidebar_state(&mut self.sidebar_width, &mut self.sidebar_collapsed, width);
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
                        this.reconcile_watchers();
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub(crate) fn activate_pane(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        if self.state.find_pane(pane_id).is_some() && self.state.active_pane_id != pane_id {
            self.state.active_pane_id = pane_id.to_string();
            self.save_workspace(cx);
        }
    }

    pub fn toggle_sidebar_path(&mut self, path: String, cx: &mut Context<Self>) {
        self.state.toggle_sidebar_path(path.clone());
        self.reconcile_watchers();
        cx.notify();
        self.request_sidebar_nodes(path, false, cx);
    }

    pub(crate) fn request_sidebar_nodes(
        &mut self,
        path: String,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.state.expanded_paths.contains(&path)
            || (!force && self.state.sidebar_nodes.contains_key(&path))
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
                        Ok(nodes) if this.state.expanded_paths.contains(&path) => {
                            this.state.sidebar_nodes.insert(path, nodes);
                            this.state.refresh_sidebar();
                            this.reconcile_watchers();
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
        let tab_idx = self.state.find_pane(pane_id)
            .map(|pane| (pane.active_tab_index + 1).min(pane.tabs.len()));
        self.state.add_tab(pane_id, path, activate);
        if let (Some(pane), Some(tab_idx)) = (self.state.find_pane(pane_id), tab_idx) {
            let path = pane.tabs[tab_idx].path.clone();
            self.request_tab_files(pane_id, tab_idx, path, false, cx);
        }
        self.reconcile_watchers();
        self.save_workspace(cx);
        cx.notify();
    }

    /// Duplicate a tab and populate the new tab's file list.
    ///
    /// `WorkspaceState::duplicate_tab` only updates the synchronous UI state;
    /// directory contents are loaded through `request_tab_files`.
    pub fn duplicate_tab(&mut self, pane_id: &str, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(new_tab_idx) = self.state.duplicate_tab(pane_id, tab_idx) {
            let path = self
                .state
                .find_pane(pane_id)
                .map(|pane| pane.tabs[new_tab_idx].path.clone());
            if let Some(path) = path {
                self.request_tab_files(pane_id, new_tab_idx, path, false, cx);
            }
        }

        self.reconcile_watchers();
        self.save_workspace(cx);
        cx.notify();
    }

    pub fn switch_tab(&mut self, pane_id: &str, tab_idx: usize, cx: &mut Context<Self>) {
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
            {
                load_path = Some(pane.active_tab().path.clone());
            }
            self.state.active_pane_id = pane_id.to_string();
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
                ))
            .then(|| (pane.active_tab_index, pane.active_tab().path.clone()))
        });
        if let Some((index, path)) = reload {
            self.request_tab_files(pane_id, index, path, false, cx);
        }
        self.reconcile_watchers();
        self.save_workspace(cx);
        cx.notify();
    }

    pub fn close_other_tabs(&mut self, pane_id: &str, tab_idx: usize, cx: &mut Context<Self>) {
        self.state.close_other_tabs(pane_id, tab_idx);
        self.reconcile_watchers();
        self.save_workspace(cx);
        cx.notify();
    }

    pub fn load_path(&mut self, pane_id: &str, path: String, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab_idx = pane.active_tab_index;
            let active_tab = pane.active_tab_mut();
            active_tab.path = path.clone();
            active_tab.files.clear();
            active_tab.cached_filtered_files.clear();
            active_tab.filter_types.clear();
            active_tab.filter_name.clear();
            active_tab.filter_sizes.clear();
            active_tab.filter_dates.clear();
            active_tab.selected_paths.clear();
            active_tab.selection_anchor = None;
            active_tab.load_status = crate::ui::workspace::TabLoadStatus::Loading;
            active_tab.list_scroll = UniformListScrollHandle::new();
            active_tab.list_scroll_h.set_offset(point(px(0.0), px(0.0)));
            self.request_tab_files(pane_id, tab_idx, path, true, cx);
            self.reconcile_watchers();
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
        self.next_file_request = self.next_file_request.wrapping_add(1);
        let request_id = self.next_file_request;
        self.pending_file_requests
            .insert(tab_id.clone(), request_id);
        let show_hidden = self.show_hidden;
        let show_system = self.show_system;
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = list_dir(
                    path.clone(),
                    None,
                    Some(show_hidden),
                    Some(show_system),
                    None,
                )
                .await;
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
                            Ok(response) => {
                                tab.files = response.entries;
                                tab.sort_files();
                                tab.update_cache();
                                tab.load_status = crate::ui::workspace::TabLoadStatus::Ready;
                                refresh_subdirectory_flags = path != "trash://";
                                if reset_scroll {
                                    tab.list_scroll = UniformListScrollHandle::new();
                                }
                            }
                            Err(error) => {
                                tab.files.clear();
                                tab.cached_filtered_files.clear();
                                tab.load_status = crate::ui::workspace::TabLoadStatus::Error(
                                    crate::ui::errors::path_load_message(&error),
                                );
                                log::warn!("Cannot list {}: {error:?}", path);
                            }
                        }
                        cx.notify();
                    }
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
                                            this.folder_has_subdirs
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
        self.refresh_all_tabs(cx);
    }

    pub fn refresh_all_tabs(&mut self, cx: &mut Context<Self>) {
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
    use super::resize_sidebar_state;

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
