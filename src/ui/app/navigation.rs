use gpui::*;
use oxyde_lib::commands::io::list_dir;
use crate::ui::app::OxydeApp;

impl OxydeApp {
    pub fn navigate_to(&mut self, pane_id: &str, path: String, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let active_tab = pane.active_tab_mut();
            if path == active_tab.path {
                return;
            }
            let old_path = active_tab.path.clone();
            active_tab.back_stack.push(old_path);
            active_tab.forward_stack.clear();
            self.load_path_sync(pane_id, path, cx);
        }
    }

    pub fn go_back(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let active_tab = pane.active_tab_mut();
            if let Some(path) = active_tab.back_stack.pop() {
                let current = active_tab.path.clone();
                active_tab.forward_stack.push(current);
                self.load_path_sync(pane_id, path, cx);
            }
        }
    }

    pub fn go_forward(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let active_tab = pane.active_tab_mut();
            if let Some(path) = active_tab.forward_stack.pop() {
                let current = active_tab.path.clone();
                active_tab.back_stack.push(current);
                self.load_path_sync(pane_id, path, cx);
            }
        }
    }

    pub fn add_tab(&mut self, pane_id: &str, path: String, activate: bool, cx: &mut Context<Self>) {
        self.state.add_tab(pane_id, path, activate);
        cx.notify();
    }

    pub fn switch_tab(&mut self, pane_id: &str, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            pane.active_tab_index = tab_idx;
            self.state.active_pane_id = pane_id.to_string();
            self.tooltip_request = None;
            self.tooltip_timer = None;
            cx.notify();
        }
    }

    pub fn close_tab(&mut self, pane_id: &str, tab_idx: usize, cx: &mut Context<Self>) {
        self.state.close_tab_id(pane_id, tab_idx);
        self.tooltip_request = None;
        self.tooltip_timer = None;
        cx.notify();
    }

    pub fn load_path_sync(&mut self, pane_id: &str, path: String, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let active_tab = pane.active_tab_mut();
            active_tab.path = path.clone();
            active_tab.files.clear();
            active_tab.filter_types.clear();
            active_tab.filter_name.clear();
            active_tab.selected_paths.clear();
            active_tab.list_scroll = UniformListScrollHandle::new(); 
            self.tooltip_request = None;
            self.tooltip_timer = None;
            cx.notify();
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            if let Ok(response) = rt.block_on(list_dir(path, None, None, None, None)) {
                if let Some(pane) = self.state.find_pane_mut(pane_id) {
                    let tab = pane.active_tab_mut();
                    tab.files = response.entries;
                    tab.sort_files();
                    tab.update_cache();
                    tab.list_scroll = UniformListScrollHandle::new();                }
            }
            cx.notify();
        }
    }
}
