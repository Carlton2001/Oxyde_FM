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
        let already_active = tab.show_size_histogram;
        if !already_active {
            tab.show_size_histogram = true;
            tab.queue_missing_folder_sizes();
        }
        let id = tab.id.clone();
        if already_active {
            self.refresh_folder_size_requests(&id, cx);
        }
        // Install the recursive watcher before letting the first two scans run.
        // Otherwise its conservative gap check dirties those scans immediately.
        self.reconcile_watchers(cx);
        self.start_folder_sizes(&id, false, cx);
        cx.notify();
    }

    pub(super) fn schedule_folder_sizes(&mut self, id: &str, cx: &mut Context<Self>) {
        self.start_folder_sizes(id, true, cx);
    }

    pub(super) fn refresh_folder_size_requests(&mut self, id: &str, cx: &mut Context<Self>) {
        use super::drive_watchers::{take_size_changes, SizeChanges};
        let Some(tab) = self
            .state
            .find_tab_mut(id)
            .filter(|tab| tab.show_size_histogram)
        else {
            return;
        };
        let root = tab.path.clone();
        let reliable = self
            ._path_watchers
            .get(&root)
            .is_some_and(|watcher| watcher.reliable_size_events());
        // A click may precede the watcher's 500 ms delivery tick. Do not miss
        // those changes or deliver them twice after this manual refresh.
        let changes = take_size_changes(&self.watcher_changes, &root);
        let mut affected = Vec::new();
        self.state.layout.visit_panes_mut(&mut |pane| {
            for tab in &mut pane.tabs {
                if tab.path != root || !tab.show_size_histogram {
                    continue;
                }
                match &changes {
                    SizeChanges::None => continue,
                    SizeChanges::All => tab.invalidate_folder_sizes(None),
                    SizeChanges::Children(paths) => tab.invalidate_folder_sizes(Some(paths)),
                }
                affected.push(tab.id.clone());
            }
        });
        if let Some(tab) = self.state.find_tab_mut(id) {
            // Without a healthy recursive native watcher, silence does not
            // establish freshness (notably between polling snapshots).
            if !reliable {
                tab.invalidate_folder_sizes(None);
            }
            tab.refresh_folder_sizes();
        }
        for other in affected.into_iter().filter(|other| other != id) {
            self.schedule_folder_sizes(&other, cx);
        }
    }

    pub(super) fn resume_folder_sizes_after_watcher(
        &mut self,
        id: &str,
        installed: bool,
        cx: &mut Context<Self>,
    ) {
        if installed {
            if let Some(tab) = self.state.find_tab_mut(id) {
                tab.reconcile_folder_size_watcher();
            }
        }
        // A failed installation must not strand the initial calculation.
        self.start_folder_sizes(id, false, cx);
        cx.notify();
    }

    fn start_folder_sizes(&mut self, id: &str, debounce: bool, cx: &mut Context<Self>) {
        if self.pending_file_requests.contains_key(id) || self.ejecting_drive {
            return;
        }
        let Some(tab) = self.state.find_tab_mut(id) else {
            return;
        };
        if self.pending_watchers.contains_key(&tab.path) {
            return;
        }
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
            // Explicit activation starts now. Watcher bursts still coalesce.
            if debounce {
                cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
            }
            loop {
                let capacity = 2 - calculations.len();
                let batch = this.update(cx, |app, cx| {
                    let loading = app.pending_file_requests.contains_key(&id) || app.ejecting_drive;
                    let tab = app.state.find_tab_mut(&id)?;
                    if tab.path != root || tab.folder_sizes.generation != generation { return None; }
                    let ready = matches!(tab.load_status, crate::ui::workspace::TabLoadStatus::Ready);
                    let batch = if loading || !ready { Vec::new() } else { tab.folder_sizes.take_available(capacity) };
                    if batch.is_empty() && calculations.is_empty() && !tab.folder_sizes.has_pending() {
                        tab.size_calculation = None;
                        cx.notify();
                        None
                    } else { Some(batch) }
                }).ok().flatten();
                let Some(batch) = batch else { return; };
                for (path, revision) in batch {
                    calculations.push(calculate(path, revision, cancellation.clone()));
                }
                if calculations.is_empty() {
                    // A hot folder's follow-up can be delayed while all physical
                    // slots are free. Keep the task alive until it becomes due.
                    cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
                    continue;
                }
                // Check for newly queued work while one slow scan occupies
                // only one slot. No second scan of the same folder may start.
                let next = futures::select_biased! {
                    result = calculations.next().fuse() => result,
                    _ = cx.background_executor().timer(std::time::Duration::from_millis(500)).fuse() => continue,
                };
                let Some((path, revision, result)) = next else { return; };
                let proceed = this.update(cx, |app, cx| {
                    let tab = app.state.find_tab_mut(&id)?;
                    if tab.path != root || tab.folder_sizes.generation != generation { return None; }
                    match result {
                        Ok(size) => tab.folder_sizes.complete(path, revision, size),
                        Err(error) => {
                            tab.folder_sizes.finish(&path, revision);
                            if !cancellation.load(Ordering::Relaxed) {
                                log::warn!("Cannot calculate size of {path}: {error}");
                            }
                        }
                    }
                    // Publishing has its own task: it never occupies a scan slot.
                    app.publish_folder_sizes(&id, cx);
                    Some(())
                }).ok().flatten();
                if proceed.is_none() { return; }
            }
        });
        tab.size_calculation = Some(Arc::new(task));
        cx.notify();
    }

    fn publish_folder_sizes(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(tab) = self.state.find_tab_mut(id) else {
            return;
        };
        if tab.folder_sizes.publication.is_some() || !tab.folder_sizes.has_ready() {
            return;
        }
        let id = id.to_owned();
        let root = tab.path.clone();
        let generation = tab.folder_sizes.generation;
        let task = cx.spawn(async move |this, cx| {
            loop {
                // Coalesce up to 128 results before each publication batch.
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
                let batch = this
                    .update(cx, |app, _| {
                        let tab = app.state.find_tab_mut(&id)?;
                        if tab.path != root || tab.folder_sizes.generation != generation {
                            return None;
                        }
                        let batch = tab.folder_sizes.ready_batch();
                        if batch.is_empty() {
                            tab.folder_sizes.publication = None;
                            None
                        } else {
                            Some(batch)
                        }
                    })
                    .ok()
                    .flatten();
                let Some(batch) = batch else {
                    return;
                };
                let directories = batch
                    .iter()
                    .map(|(path, revision, _)| (path.clone(), *revision))
                    .collect();
                let owner = Owner::FolderSizes {
                    id: id.clone(),
                    path: root.clone(),
                    generation,
                    directories,
                };
                let update = FileUpdate::FolderSizes(
                    batch
                        .into_iter()
                        .map(|(path, _, size)| (path, size))
                        .collect(),
                );
                let Some(handle) = this.upgrade() else {
                    return;
                };
                let applied = file_preparation::prepare(&handle, cx, &owner, update).await;
                let proceed = this
                    .update(cx, |app, cx| {
                        let tab = app.state.find_tab_mut(&id)?;
                        if tab.path != root || tab.folder_sizes.generation != generation {
                            return None;
                        }
                        if applied {
                            if let Owner::FolderSizes { directories, .. } = &owner {
                                tab.folder_sizes.published(directories);
                                cx.notify();
                            }
                        }
                        // Navigation/removal still reject obsolete measurements;
                        // ordinary changes leave the value visible as provisional.
                        Some(())
                    })
                    .ok()
                    .flatten();
                if proceed.is_none() {
                    return;
                }
            }
        });
        tab.folder_sizes.publication = Some(Arc::new(task));
    }
}
