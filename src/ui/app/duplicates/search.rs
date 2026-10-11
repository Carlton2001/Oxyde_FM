use super::DuplicateWindow;
use gpui::{px, point, Context};
use oxyde_lib::commands::duplicates::{find_duplicates, DuplicateSearchOptions, DuplicateSearchState};
use std::sync::Arc;

impl DuplicateWindow {
    pub(super) fn search(&mut self, cx: &mut Context<Self>) {
        if self.running || self.selected.is_empty() || !self.options.iter().any(|v| *v) {
            return;
        }
        self.running = true;
        self.searched = true;
        self.error = None;
        self.limit = 100;
        self.results_scroll.set_offset(point(px(0.0), px(0.0)));
        self.cancellation = Arc::new(DuplicateSearchState::new());
        let progress_token = self.cancellation.clone();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(150))
                .await;
            let keep_running = this
                .update(cx, |this, cx| {
                    if !this.running || !Arc::ptr_eq(&this.cancellation, &progress_token) {
                        return false;
                    }
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !keep_running {
                break;
            }
        })
        .detach();
        let cancellation = self.cancellation.clone();
        let paths = self.selected.iter().cloned().collect();
        let options = DuplicateSearchOptions {
            by_name: self.options[0],
            by_size: self.options[1],
            by_content: self.options[2],
        };
        let runtime = tokio::runtime::Handle::current();
        let worker = cx
            .background_executor()
            .spawn(async move { runtime.block_on(find_duplicates(&cancellation, paths, options)) });
        cx.spawn(async move |this, cx| {
            let result = worker.await;
            let _ = this.update(cx, |this, cx| {
                this.running = false;
                if !this
                    .cancellation
                    .0
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    match result {
                        Ok(groups) => {
                            this.groups = Arc::new(groups);
                            this.filtered.clear();
                            this.update_filter(cx);
                            this.collapsed.clear();
                        }
                        Err(e) => this.error = Some(e.to_string()),
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn update_filter(&mut self, cx: &mut Context<Self>) {
        let groups = self.groups.clone();
        let query = self.query.clone();
        self.filtering = true;
        self.filter_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(150))
                .await;
            let snapshot = groups.clone();
            let needle = query.trim().to_string();
            let indices = cx
                .background_executor()
                .spawn(async move {
                    snapshot
                        .iter()
                        .enumerate()
                        .filter(|(_, group)| {
                            needle.is_empty()
                                || group.files.iter().any(|file| {
                                    file.path.to_lowercase().contains(&needle)
                                        || file.name.to_lowercase().contains(&needle)
                                })
                        })
                        .map(|(index, _)| index)
                        .collect()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if Arc::ptr_eq(&this.groups, &groups) && this.query == query {
                    this.filtered = indices;
                    this.filtering = false;
                    cx.notify();
                }
            });
        }));
    }
}
