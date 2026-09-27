use crate::ui::app::OxydeApp;
use gpui::*;
use oxyde_lib::commands::io::list_dir;

impl OxydeApp {
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
        self.state.add_tab(pane_id, path, activate);
        if let Some(pane) = self.state.find_pane(pane_id) {
            let tab_idx = pane.tabs.len() - 1;
            let path = pane.tabs[tab_idx].path.clone();
            self.request_tab_files(pane_id, tab_idx, path, false, cx);
        }
        self.reconcile_watchers();
        cx.notify();
    }

    pub fn switch_tab(&mut self, pane_id: &str, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            pane.active_tab_index = tab_idx;
            self.state.active_pane_id = pane_id.to_string();
            cx.notify();
        }
    }

    pub fn close_tab(&mut self, pane_id: &str, tab_idx: usize, cx: &mut Context<Self>) {
        self.state.close_tab_id(pane_id, tab_idx);
        self.reconcile_watchers();
        cx.notify();
    }

    pub fn load_path(&mut self, pane_id: &str, path: String, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab_idx = pane.active_tab_index;
            let active_tab = pane.active_tab_mut();
            active_tab.path = path.clone();
            active_tab.files.clear();
            active_tab.filter_types.clear();
            active_tab.filter_name.clear();
            active_tab.selected_paths.clear();
            active_tab.list_scroll = UniformListScrollHandle::new();
            self.request_tab_files(pane_id, tab_idx, path, true, cx);
            self.reconcile_watchers();
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
        let pane_id = pane_id.to_string();
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
                handle.update(&mut cx, move |this, cx| {
                    if this.pending_file_requests.get(&tab_id) != Some(&request_id) {
                        return;
                    }
                    this.pending_file_requests.remove(&tab_id);
                    if let Ok(response) = result {
                        if let Some(tab) = this.state.find_pane_mut(&pane_id).and_then(|pane| {
                            pane.tabs
                                .iter_mut()
                                .find(|tab| tab.id == tab_id && tab.path == path)
                        }) {
                            tab.files = response.entries;
                            tab.sort_files();
                            tab.update_cache();
                            if reset_scroll {
                                tab.list_scroll = UniformListScrollHandle::new();
                            }
                            cx.notify();
                        }
                    } else if let Err(error) = result {
                        log::warn!("Cannot list {}: {error:?}", path);
                    }
                });
            }
        })
        .detach();
    }
    pub fn refresh_all(&mut self, cx: &mut Context<Self>) {
        // Refresh Sidebar (Drives, Pinned, etc)
        self.state.refresh_sidebar();

        // Refresh all open panes (their active tabs)
        let mut reloads = Vec::new();
        self.state.layout.visit_panes(&mut |p| {
            reloads.push((p.id.clone(), p.active_tab().path.clone()));
        });

        for (pane_id, path) in reloads {
            self.load_path(&pane_id, path, cx);
        }

        cx.notify();
    }
}
