use super::{
    file_preparation::{self, Owner},
    OxydeApp,
};
use crate::ui::workspace::FileUpdate;
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use gpui::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

struct CancelSizeCalculation(Arc<AtomicBool>);
impl Drop for CancelSizeCalculation {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

async fn calculate(
    path: String,
    revision: u64,
    cancellation: Arc<AtomicBool>,
) -> (
    String,
    u64,
    Result<oxyde_lib::models::FolderSizeResult, oxyde_lib::models::CommandError>,
) {
    let result =
        oxyde_lib::commands::io::calculate_folder_size_cancellable(path.clone(), cancellation)
            .await;
    (path, revision, result)
}

impl OxydeApp {
    pub(crate) fn calculate_sizes(&mut self, cx: &mut Context<Self>) {
        self.context_menus.clear();
        let Some(tab) = self.state.active_tab_mut() else {
            return;
        };
        if tab.path.contains("://")
            || tab.path.starts_with("::{")
            || tab.path == "__network_vincinity__"
            || tab.archive_root.is_some()
        {
            cx.notify();
            return;
        }
        tab.show_size_histogram = true;
        tab.queue_missing_folder_sizes();
        let id = tab.id.clone();
        self.schedule_folder_sizes(&id, cx);
        self.reconcile_watchers(cx);
        cx.notify();
    }

    pub(super) fn schedule_folder_sizes(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.pending_file_requests.contains_key(id) || self.ejecting_drive {
            return;
        }
        let Some(tab) = self.state.find_tab_mut(id) else {
            return;
        };
        if !tab.show_size_histogram
            || tab.size_calculation.is_some()
            || tab.archive_root.is_some()
            || !tab.folder_sizes.has_pending()
            || !matches!(tab.load_status, crate::ui::workspace::TabLoadStatus::Ready)
        {
            return;
        }
        let id = id.to_owned();
        let root = tab.path.clone();
        let generation = tab.folder_sizes.generation;
        let cancellation = Arc::new(AtomicBool::new(false));
        tab.folder_sizes.cancellation = Some(cancellation.clone());
        let cancel_on_drop = CancelSizeCalculation(cancellation.clone());
        let task = cx.spawn(async move |this, cx| {
            let _cancel_on_drop = cancel_on_drop;
            let mut calculations = FuturesUnordered::new();
            // Initial coalescing; subsequent completed scans free a slot
            // immediately, even while another folder takes much longer.
            cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
            loop {
                let capacity = 2 - calculations.len();
                let batch = this.update(cx, |app, cx| {
                    let loading = app.pending_file_requests.contains_key(&id) || app.ejecting_drive;
                    let tab = app.state.find_tab_mut(&id)?;
                    if tab.path != root || tab.folder_sizes.generation != generation { return None; }
                    let ready = matches!(tab.load_status, crate::ui::workspace::TabLoadStatus::Ready);
                    let batch = if loading || !ready { Vec::new() } else { tab.folder_sizes.take_available(capacity) };
                    if batch.is_empty() && calculations.is_empty() {
                        tab.size_calculation = None;
                        cx.notify();
                        None
                    } else { Some(batch) }
                }).ok().flatten();
                let Some(batch) = batch else { return; };
                for (path, revision) in batch {
                    calculations.push(calculate(path, revision, cancellation.clone()));
                }
                // Check for newly queued work while one slow scan occupies
                // only one slot. No second scan of the same folder may start.
                let next = futures::select_biased! {
                    result = calculations.next().fuse() => result,
                    _ = cx.background_executor().timer(std::time::Duration::from_millis(500)).fuse() => continue,
                };
                let Some((path, revision, result)) = next else { return; };
                let owner = Owner::FolderSize {
                    id: id.clone(), path: root.clone(), generation,
                    directory: path.clone(), revision,
                };
                match result {
                    Ok(size) => {
                        let Some(handle) = this.upgrade() else { return; };
                        file_preparation::prepare(&handle, cx, &owner,
                            FileUpdate::FolderSize { path: path.clone(), size }).await;
                    }
                    Err(error) if !cancellation.load(Ordering::Relaxed) => {
                        log::warn!("Cannot calculate size of {path}: {error}");
                    }
                    Err(_) => {}
                }
                let proceed = this.update(cx, |app, _| {
                    let tab = app.state.find_tab_mut(&id)?;
                    if tab.path != root || tab.folder_sizes.generation != generation { return None; }
                    tab.folder_sizes.finish(&path, revision);
                    Some(())
                }).ok().flatten();
                if proceed.is_none() { return; }
            }
        });
        tab.size_calculation = Some(Arc::new(task));
        cx.notify();
    }
}
