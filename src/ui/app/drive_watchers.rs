use super::{drive_watchers, OxydeApp, ToastKind};
use crate::ui::i18n::Message;
use crate::ui::components::sidebar::sidebar_paths_equal;
use gpui::*;
use notify::{Config, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

pub struct DirectoryWatcher {
    polling: bool,
    watcher: Box<dyn Watcher + Send>,
}

impl DirectoryWatcher {
    fn create(
        path: &str,
        polling: bool,
        changes: Arc<Mutex<VecDeque<String>>>,
    ) -> notify::Result<Self> {
        let watched_path = path.to_owned();
        let callback = move |result: notify::Result<notify::Event>| {
            if result.is_ok() {
                if let Ok(mut queue) = changes.lock() {
                    if !queue.contains(&watched_path) {
                        queue.push_back(watched_path.clone());
                    }
                }
            }
        };
        // USB polling retains snapshots, not open directory handles. All scans,
        // including the initial snapshot, run on a background worker.
        let mut watcher: Box<dyn Watcher + Send> = if polling {
            Box::new(PollWatcher::new(
                callback,
                Config::default().with_poll_interval(std::time::Duration::from_secs(2)),
            )?)
        } else {
            Box::new(RecommendedWatcher::new(callback, Config::default())?)
        };
        watcher.watch(std::path::Path::new(path), RecursiveMode::NonRecursive)?;
        Ok(Self { polling, watcher })
    }

    pub(super) fn release(mut self, path: &str) -> Result<(), String> {
        self.watcher
            .unwatch(std::path::Path::new(path))
            .map_err(|error| error.to_string())?;
        // On Windows unwatch is queued. configure's reply acts as a FIFO barrier:
        // even Ok(false) means that the preceding unwatch has finished closing handles.
        // PollWatcher unwatch already waits for any current snapshot to finish.
        self.watcher
            .configure(Config::default())
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}

pub(super) fn path_on_drive(path: &str, root: &str) -> bool {
    std::path::Path::new(path)
        .ancestors()
        .any(|ancestor| sidebar_paths_equal(&ancestor.to_string_lossy(), root))
}

pub(super) struct DriveRefreshSchedule {
    revision: u64,
    next: std::time::Instant,
    retries: usize,
    fallback: std::time::Duration,
    pub(super) details_due: bool,
}

impl DriveRefreshSchedule {
    pub(super) fn new(native: bool, now: std::time::Instant) -> Self {
        Self {
            revision: 0,
            next: now,
            retries: 0,
            fallback: std::time::Duration::from_secs(if native { 30 } else { 2 }),
            details_due: false,
        }
    }

    /// Interface arrival can precede drive-letter assignment: retry briefly.
    pub(super) fn due(&mut self, revision: u64, now: std::time::Instant) -> bool {
        let event = self.revision != revision;
        let periodic = self.retries == 0;
        if event {
            self.revision = revision;
            self.retries = 5;
            self.next = now;
        }
        if now < self.next {
            return false;
        }
        self.details_due = event || periodic;
        let delay = if self.retries > 0 {
            let delays = [150, 350, 750, 1500, 3000];
            let delay = std::time::Duration::from_millis(delays[5 - self.retries]);
            self.retries -= 1;
            delay
        } else {
            self.fallback
        };
        self.next = now + delay;
        true
    }
}

fn merge_drive_roots(
    roots: Vec<oxyde_lib::models::DriveInfo>,
    current: &[oxyde_lib::models::DriveInfo],
) -> Vec<oxyde_lib::models::DriveInfo> {
    roots
        .into_iter()
        .map(|root| {
            current
                .iter()
                .find(|old| {
                    sidebar_paths_equal(&old.path, &root.path) && old.drive_type == root.drive_type
                })
                .cloned()
                .unwrap_or(root)
        })
        .collect()
}

impl OxydeApp {
    pub(crate) fn eject_usb_drive(&mut self, path: String, cx: &mut Context<Self>) {
        self.context_menus.clear();
        if self.ejecting_drive || !self.state.drives.iter()
            .any(|drive| drive.path == path && drive.is_usb()) {
            cx.notify();
            return;
        }
        self.ejecting_drive = true;
        self.drive_refresh_epoch = self.drive_refresh_epoch.wrapping_add(1);
        self.drive_details_task = None;
        self.sidebar_follow_task = None;
        self.pending_sidebar_requests.clear();
        self.pending_watchers.clear();
        let watchers = std::mem::take(&mut self._path_watchers);
        self.reconcile_watchers(cx);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                // Release watches and wait for their handles to close before PnP.
                for (watched_path, watcher) in watchers {
                    watcher.release(&watched_path)?;
                }
                oxyde_lib::platform::platform().eject_drive(&path)?;
                Ok::<_, String>(oxyde_lib::commands::system::get_drives(Some(false)))
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.ejecting_drive = false;
                let error = match result {
                    Ok(Ok(drives)) => {
                        this.apply_drive_snapshot(drives, cx);
                        None
                    },
                    Ok(Err(error)) => Some(error),
                    Err(error) => Some(error.to_string()),
                };
                if let Some(error) = error {
                    this.toast(ToastKind::Error, Message::EjectDriveFailed, error, 0, cx);
                }
                this.reconcile_watchers(cx);
                cx.notify();
            });
        }).detach();
    }

    fn start_drive_refresh_task(&mut self, cx: &mut Context<Self>) {
        let h = self.handle.clone().expect("handle set");
        self._drive_refresh_task = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                use std::sync::atomic::{AtomicU64, Ordering};
                let revision = std::sync::Arc::new(AtomicU64::new(0));
                let registration_revision = revision.clone();
                let registration = tokio::task::spawn_blocking(move || {
                    oxyde_lib::platform::platform().watch_drive_changes(registration_revision)
                }).await;
                let _registration = match registration {
                    Ok(Ok(guard)) => guard,
                    Ok(Err(error)) => { log::warn!("Drive notifications unavailable: {error}"); None }
                    Err(error) => { log::warn!("Drive notification worker failed: {error}"); None }
                };
                let mut schedule = drive_watchers::DriveRefreshSchedule::new(
                    _registration.is_some(), std::time::Instant::now());
                loop {
                    let paused = h.update(&mut cx, |this, _| this.ejecting_drive);
                    let observed = revision.load(Ordering::Acquire);
                    if !paused && schedule.due(observed, std::time::Instant::now()) {
                        let details_due = schedule.details_due;
                        let (epoch, policy_revision) = h.update(&mut cx, |this, _| {
                            (this.drive_refresh_epoch, this.trash_delete_policy.revision())
                        });
                        if let Ok((roots, policies)) = tokio::task::spawn_blocking(|| {
                            let roots = oxyde_lib::platform::platform().list_drive_roots();
                            let policies = super::trash_settings::read_drive_policy(&roots);
                            (roots, policies)
                        }).await {
                            h.update(&mut cx, |this, cx| {
                                if this.ejecting_drive || this.drive_refresh_epoch != epoch
                                    || revision.load(Ordering::Acquire) != observed { return; }
                                let changed = roots.len() != this.state.drives.len()
                                    || roots.iter().any(|root| !this.state.drives.iter()
                                        .any(|old| old.path == root.path && old.drive_type == root.drive_type));
                                if changed {
                                    this.drive_refresh_epoch = this.drive_refresh_epoch.wrapping_add(1);
                                }
                                this.apply_drive_roots(roots, cx);
                                if this.trash_delete_policy.replace(policy_revision, policies) {
                                    cx.notify();
                                }
                                if changed || details_due {
                                    this.refresh_disk_images(cx);
                                }
                                if changed || details_due || this.drive_details_task.is_none() {
                                    this.request_drive_details(cx);
                                }
                                let roots: Vec<_> = this.state.drives.iter()
                                    .map(|drive| drive.path.clone()).collect();
                                for root in roots {
                                    this.request_sidebar_nodes(root, false, cx);
                                }
                                this.reconcile_watchers(cx);
                            });
                        }
                    }
                    cx.background_executor().timer(std::time::Duration::from_millis(100)).await;
                }
            }
        }));
    }
    pub fn start_watcher_task(&mut self, cx: &mut Context<Self>) {
        self.start_drive_refresh_task(cx);
        self.start_shell_features_task(cx);
        self.start_calendar_task(cx);
        self.refresh_disk_images(cx);
        let changes = self.watcher_changes.clone();
        let h = self.handle.clone().expect("handle set");
        self._watcher_task = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(500))
                        .await;
                    let clipboard_has_files = tokio::task::spawn_blocking(|| {
                        oxyde_lib::platform::platform().clipboard_has_file_format()
                    }).await.unwrap_or(false);
                    h.update(&mut cx, |this, cx| {
                        if this.clipboard_has_files != clipboard_has_files {
                            this.clipboard_has_files = clipboard_has_files;
                            cx.notify();
                        }
                    });
                    let changed: Vec<String> = if let Ok(mut q) = changes.lock() {
                        q.drain(..).collect()
                    } else {
                        vec![]
                    };
                    if !changed.is_empty() {
                        h.update(&mut cx, |this, cx| {
                            if this.ejecting_drive { return; }
                            // Refresh only affected expanded paths off the UI thread.
                            for path in &changed {
                                if this.state.expanded_paths.contains(path)
                                    || this.state.drives.iter().any(|drive| &drive.path == path) {
                                    this.request_sidebar_nodes(path.clone(), true, cx);
                                }
                            }

                            // Reload every affected tab (active = full refresh, inactive = files only).
                            let mut reloads: Vec<(String, usize, String)> = Vec::new();
                            this.state.layout.visit_panes(&mut |p| {
                                for (idx, tab) in p.tabs.iter().enumerate() {
                                    let watched =
                                        oxyde_lib::utils::archive::split_virtual_path(&tab.path)
                                            .and_then(|(archive, _)| {
                                                archive.parent().map(|parent| {
                                                    parent.to_string_lossy().into_owned()
                                                })
                                            })
                                            .unwrap_or_else(|| tab.path.clone());
                                    if changed.contains(&watched) {
                                        reloads.push((p.id.clone(), idx, tab.path.clone()));
                                    }
                                }
                            });
                            for (pane_id, tab_idx, path) in reloads {
                                this.refresh_tab_files(&pane_id, tab_idx, path, cx);
                            }
                        });
                    }
                }
            }
        }));
    }

    pub(super) fn apply_drive_roots(
        &mut self,
        roots: Vec<oxyde_lib::models::DriveInfo>,
        cx: &mut Context<Self>,
    ) {
        let drives = merge_drive_roots(roots, &self.state.drives);
        self.apply_drive_snapshot(drives, cx);
    }

    pub(super) fn request_drive_details(&mut self, cx: &mut Context<Self>) {
        let epoch = self.drive_refresh_epoch;
        self.drive_details_task = Some(cx.spawn(async move |this, cx| {
            if let Ok(details) =
                tokio::task::spawn_blocking(|| oxyde_lib::platform::platform().list_drives(false))
                    .await
            {
                let _ = this.update(cx, |this, cx| {
                    if this.ejecting_drive || this.drive_refresh_epoch != epoch {
                        return;
                    }
                    // Details may arrive late. They can enrich current mounts, never
                    // restore a disappeared drive or remove a newly discovered one.
                    let drives = this
                        .state
                        .drives
                        .iter()
                        .map(|current| {
                            details
                                .iter()
                                .find(|detail| sidebar_paths_equal(&current.path, &detail.path))
                                .cloned()
                                .unwrap_or_else(|| current.clone())
                        })
                        .collect();
                    this.apply_drive_snapshot(drives, cx);
                    this.reconcile_watchers(cx);
                });
            }
        }));
    }

    pub(super) fn apply_drive_snapshot(
        &mut self,
        drives: Vec<oxyde_lib::models::DriveInfo>,
        cx: &mut Context<Self>,
    ) {
        self.drive_metadata_ready = true;
        if self.state.drives == drives {
            return;
        }
        let removed: Vec<_> = self
            .state
            .drives
            .iter()
            .filter(|old| {
                !drives
                    .iter()
                    .any(|new| sidebar_paths_equal(&new.path, &old.path))
            })
            .map(|drive| drive.path.clone())
            .collect();
        let retained = |path: &str| !removed.iter().any(|root| path_on_drive(path, root));
        self.state.expanded_paths.retain(|path| retained(path));
        self.state.sidebar_nodes.retain(|path, _| retained(path));
        self.folder_has_subdirs.retain(|path, _| retained(path));
        self.pending_sidebar_requests
            .retain(|path, _| retained(path));
        if self
            .sidebar_selected_path
            .as_deref()
            .is_some_and(|path| !retained(path))
        {
            self.sidebar_selected_path = None;
        }
        self.state.drives = drives;
        self.redirect_removed_drive_tabs(&removed, cx);
        self.state.refresh_sidebar();
        cx.notify();
    }

    pub fn reconcile_watchers(&mut self, cx: &mut Context<Self>) {
        if self.ejecting_drive || !self.drive_metadata_ready {
            return;
        }
        let mut needed = HashSet::new();
        self.state.layout.visit_panes(&mut |pane| {
            needed.extend(
                pane.tabs
                    .iter()
                    .filter(|tab| !tab.path.is_empty() && !tab.path.starts_with("search://"))
                    .map(|tab| oxyde_lib::utils::archive::split_virtual_path(&tab.path)
                        .and_then(|(archive, _)| archive.parent().map(|parent| parent.to_string_lossy().into_owned()))
                        .unwrap_or_else(|| tab.path.clone())),
            );
        });
        needed.extend(self.state.expanded_paths.iter().cloned());
        needed.extend(self.state.drives.iter().map(|drive| drive.path.clone()));
        let wanted: HashMap<_, _> = needed
            .into_iter()
            .map(|path| {
                let polling = self.state.drives.iter().any(|drive| {
                    (drive.is_usb() || drive.media_type.is_none())
                        && path_on_drive(&path, &drive.path)
                });
                (path, polling)
            })
            .collect();
        self.pending_watchers
            .retain(|path, (_, polling)| wanted.get(path) == Some(polling));
        let obsolete: Vec<_> = self
            ._path_watchers
            .iter()
            .filter(|(path, watcher)| wanted.get(*path) != Some(&watcher.polling))
            .map(|(path, _)| path.clone())
            .collect();
        for path in obsolete {
            if let Some(watcher) = self._path_watchers.remove(&path) {
                cx.background_executor()
                    .spawn(async move {
                        let _ = watcher.release(&path);
                    })
                    .detach();
            }
        }
        for (path, polling) in wanted {
            if self._path_watchers.contains_key(&path) || self.pending_watchers.contains_key(&path)
            {
                continue;
            }
            self.next_watcher_request = self.next_watcher_request.wrapping_add(1);
            let request = self.next_watcher_request;
            self.pending_watchers
                .insert(path.clone(), (request, polling));
            let changes = self.watcher_changes.clone();
            cx.spawn(async move |this, cx| {
                let worker_path = path.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move { DirectoryWatcher::create(&worker_path, polling, changes) })
                    .await;
                let mut result = result.ok();
                let _ = this.update(cx, |this, _| {
                    if !this.ejecting_drive
                        && this.pending_watchers.get(&path) == Some(&(request, polling))
                    {
                        this.pending_watchers.remove(&path);
                        if let Some(watcher) = result.take() {
                            this._path_watchers.insert(path.clone(), watcher);
                        }
                    }
                });
                if let Some(watcher) = result {
                    cx.background_executor()
                        .spawn(async move {
                            let _ = watcher.release(&path);
                        })
                        .await;
                }
            })
            .detach();
        }
    }

    pub(super) fn redirect_removed_drive_tabs(
        &mut self,
        removed: &[String],
        cx: &mut Context<Self>,
    ) {
        if removed.is_empty() {
            return;
        }
        let fallback = dirs::home_dir()
            .map(|path| path.to_string_lossy().into_owned())
            .filter(|path| !removed.iter().any(|root| path_on_drive(path, root)))
            .or_else(|| self.state.drives.first().map(|drive| drive.path.clone()))
            .unwrap_or_else(|| std::path::MAIN_SEPARATOR.to_string());
        let requests = redirect_tabs(&mut self.state, removed, &fallback);
        for (pane, index, path) in requests {
            self.request_tab_files(&pane, index, path, true, cx);
        }
        self.sidebar_follow_path = None;
        self.save_workspace(cx);
    }
}

fn redirect_tabs(
    state: &mut super::WorkspaceState,
    removed: &[String],
    fallback: &str,
) -> Vec<(String, usize, String)> {
    let affected = |path: &str| {
        let search_root = super::search::parse_search_path(path).map(|(root, _)| root);
        let path = search_root.as_deref().unwrap_or(path);
        removed.iter().any(|root| path_on_drive(path, root))
    };
    let mut requests = Vec::new();
    state.layout.visit_panes_mut(&mut |pane| {
        for (index, tab) in pane.tabs.iter_mut().enumerate() {
            tab.back_stack.retain(|path| !affected(path));
            tab.forward_stack.retain(|path| !affected(path));
            if !affected(&tab.path) {
                continue;
            }
            tab.path = fallback.to_owned();
            tab.search_revision = tab.search_revision.wrapping_add(1);
            tab.search_input = None;
            tab.search_text.clear();
            tab.pending_reveal = None;
            if let Some(search) = tab.quick_search.take() {
                search.cancellation.0.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            tab.files.clear();
            tab.cached_filtered_files.clear();
            tab.selected_paths.clear();
            tab.selection_anchor = None;
            tab.load_status = crate::ui::workspace::TabLoadStatus::Loading;
            requests.push((pane.id.clone(), index, tab.path.clone()));
        }
    });
    requests
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::workspace::{LayoutNode, PaneState, TabState};

    #[::core::prelude::v1::test]
    fn native_notifications_refresh_without_waiting_for_periodic_check() {
        let now = std::time::Instant::now();
        let mut schedule = DriveRefreshSchedule::new(true, now);
        assert!(schedule.due(0, now));
        assert!(!schedule.due(0, now + std::time::Duration::from_secs(2)));
        assert!(schedule.due(1, now + std::time::Duration::from_secs(2)));
        assert!(schedule.details_due);
        assert!(!schedule.due(1, now + std::time::Duration::from_millis(2100)));
        assert!(schedule.due(1, now + std::time::Duration::from_millis(2200)));
        assert!(!schedule.details_due);
        // A second notification immediately restarts the retry burst.
        assert!(schedule.due(2, now + std::time::Duration::from_millis(2250)));
    }

    #[::core::prelude::v1::test]
    fn periodic_checks_continue_when_native_notifications_are_unavailable() {
        let now = std::time::Instant::now();
        let mut schedule = DriveRefreshSchedule::new(false, now);
        assert!(schedule.due(0, now));
        assert!(!schedule.due(0, now + std::time::Duration::from_secs(1)));
        assert!(schedule.due(0, now + std::time::Duration::from_secs(2)));
    }

    #[::core::prelude::v1::test]
    fn quick_discovery_keeps_existing_details_and_adds_new_mounts_immediately() {
        let known = oxyde_lib::models::DriveInfo {
            path: "C:\\".into(),
            label: "System".into(),
            drive_type: "fixed".into(),
            total_bytes: 100,
            free_bytes: 50,
            media_type: Some("SSD".into()),
            physical_id: Some("Disk 0".into()),
            remote_path: None,
        };
        let mut root = known.clone();
        root.total_bytes = 0;
        root.media_type = None;
        let mut usb = root.clone();
        usb.path = "E:\\".into();
        usb.drive_type = "removable".into();
        let snapshot = merge_drive_roots(vec![root, usb.clone()], &[known.clone()]);
        assert_eq!(snapshot, vec![known, usb]);
        // Absence from the quick snapshot removes the mount without waiting for details.
        assert!(merge_drive_roots(Vec::new(), &snapshot).is_empty());
    }

    #[::core::prelude::v1::test]
    fn removal_redirects_only_affected_tabs_and_cleans_their_history() {
        let root = std::env::temp_dir()
            .join("oxyde-removed-drive")
            .to_string_lossy()
            .into_owned();
        let fallback = std::env::temp_dir()
            .join("oxyde-fallback")
            .to_string_lossy()
            .into_owned();
        let mut first = TabState::new(
            "usb".into(),
            format!("{root}{}folder", std::path::MAIN_SEPARATOR),
        );
        first.back_stack = vec![root.clone(), fallback.clone()];
        first.filter_name = "keep filter".into();
        first.selected_paths.insert(first.path.clone());
        let mut pane = PaneState::new(first);
        pane.tabs
            .push(TabState::new("other".into(), fallback.clone()));
        let mut state = super::super::WorkspaceState::new();
        state.layout = LayoutNode::Pane(Box::new(pane));
        let requests = redirect_tabs(&mut state, &[root], &fallback);
        assert_eq!(requests.len(), 1);
        state.layout.visit_panes(&mut |pane| {
            assert_eq!(pane.tabs[0].id, "usb");
            assert_eq!(pane.tabs[0].path, fallback);
            assert_eq!(pane.tabs[0].back_stack, vec![fallback.clone()]);
            assert_eq!(pane.tabs[0].filter_name, "keep filter");
            assert!(pane.tabs[0].selected_paths.is_empty());
            assert_eq!(pane.tabs[1].id, "other");
            assert_eq!(pane.tabs[1].path, fallback);
        });
    }

    #[::core::prelude::v1::test]
    fn path_matching_does_not_confuse_neighboring_roots() {
        let root = std::env::temp_dir().join("usb");
        assert!(path_on_drive(
            &root.join("folder").to_string_lossy(),
            &root.to_string_lossy()
        ));
        assert!(!path_on_drive(
            &root.with_file_name("usb-other").to_string_lossy(),
            &root.to_string_lossy()
        ));
    }

    #[::core::prelude::v1::test]
    fn removing_search_root_cancels_and_redirects_its_tab() {
        let root = std::env::temp_dir().join("search-drive").to_string_lossy().into_owned();
        let token = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut tab = TabState::new("search".into(), super::super::search::search_path(&root, "us"));
        tab.search_text = "us".into();
        tab.quick_search = Some(super::super::search::QuickSearchState {
            root: root.clone(), query: "us".into(), running: true, limited: false,
            archive_warnings: Default::default(),
            cancellation: Arc::new(super::super::search::SearchCancellation(token.clone())),
        });
        tab.back_stack.push(tab.path.clone());
        let mut state = super::super::WorkspaceState::new();
        state.layout = LayoutNode::Pane(Box::new(PaneState::new(tab)));
        assert_eq!(redirect_tabs(&mut state, &[root], "fallback").len(), 1);
        assert!(token.load(std::sync::atomic::Ordering::Relaxed));
        let tab = state.find_tab_mut("search").unwrap();
        assert_eq!(tab.path, "fallback");
        assert!(tab.quick_search.is_none());
        assert!(tab.search_text.is_empty());
        assert!(tab.back_stack.is_empty());
    }

    #[::core::prelude::v1::test]
    fn usb_polling_detects_changes_and_can_be_released_before_removal() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().to_string_lossy().into_owned();
        let changes = Arc::new(Mutex::new(VecDeque::new()));
        let watcher = DirectoryWatcher::create(&path, true, changes.clone()).unwrap();
        changes.lock().unwrap().clear();
        std::fs::create_dir(directory.path().join("new-folder")).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
        while changes.lock().unwrap().is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(changes.lock().unwrap().contains(&path));
        watcher.release(&path).unwrap();
        directory.close().unwrap();
    }
}
