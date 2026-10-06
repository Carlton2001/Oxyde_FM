use super::OxydeApp;
use gpui::*;
use futures::{stream::FuturesUnordered, StreamExt};

impl OxydeApp {
    pub(crate) fn calculate_sizes(&mut self, cx: &mut Context<Self>) {
        self.context_menus.clear();
        let Some(tab) = self.state.active_tab_mut() else { return; };
        if tab.path.contains("://") || tab.path.starts_with("::{") || tab.path == "__network_vincinity__" {
            cx.notify();
            return;
        }
        tab.show_size_histogram = true;
        if tab.size_calculation.is_some() {
            cx.notify();
            return;
        }
        let tab_id = tab.id.clone();
        let root = tab.path.clone();
        let paths: Vec<_> = tab.files.iter()
            .filter(|file| file.is_dir && !file.is_calculated && !file.is_symlink && !file.is_junction)
            .map(|file| file.path.clone()).collect();
        let task = cx.spawn(async move |this, cx| {
            // Poll all folder scans concurrently and apply each result as soon
            // as it finishes, regardless of the order of the displayed rows.
            let mut calculations: FuturesUnordered<_> = paths.into_iter().map(|path| async move {
                let result = oxyde_lib::commands::io::calculate_folder_size(path.clone()).await;
                (path, result)
            }).collect();
            while let Some((path, result)) = calculations.next().await {
                let proceed = this.update(cx, |this, cx| {
                    let Some(tab) = this.state.find_tab_mut(&tab_id) else { return false; };
                    if tab.path != root { return false; }
                    match result {
                        Ok(size) => {
                            if let Some(file) = tab.files.iter_mut().find(|file| file.path == path) {
                                file.size = size.size;
                                file.files_count = Some(size.files_count as u64);
                                file.folders_count = Some(size.folders_count as u64);
                                file.is_calculated = true;
                            }
                            tab.sort_files();
                            tab.update_cache();
                        }
                        Err(error) => log::warn!("Cannot calculate size of {path}: {error}"),
                    }
                    cx.notify();
                    true
                }).unwrap_or(false);
                if !proceed { return; }
            }
            let _ = this.update(cx, |this, cx| {
                if let Some(tab) = this.state.find_tab_mut(&tab_id) {
                    tab.size_calculation = None;
                }
                cx.notify();
            });
        });
        self.state.active_tab_mut().unwrap().size_calculation = Some(std::sync::Arc::new(task));
        cx.notify();
    }
}
