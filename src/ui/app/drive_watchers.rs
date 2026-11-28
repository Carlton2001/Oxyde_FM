use super::{drive_watchers, OxydeApp, ToastKind};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::paths::paths_match;
use gpui::*;
mod absences;
mod changes;
pub use changes::{DirectoryWatcher, WatcherChanges};
pub(super) use changes::{take_size_changes, SizeChanges};
use std::collections::{HashMap, HashSet};
#[cfg(test)]
use std::sync::{Arc, Mutex};

pub(super) fn path_on_drive(path: &str, root: &str) -> bool {
    std::path::Path::new(path)
        .ancestors()
        .any(|ancestor| paths_match(&ancestor.to_string_lossy(), root))
}

/// Whether the innermost mount holding `path` refuses writes. Under Linux, the list
/// hides most mounts: a read-only `/` leaves the writable `/home` or `/var/home` mounted
/// on it writable, and a link of `/` leads to where it points.
fn on_read_only_volume(path: &str, protection: &oxyde_lib::platform::WriteProtection) -> bool {
    let linked = protection.links.iter()
        .filter_map(|(link, target)| {
            let rest = std::path::Path::new(path).strip_prefix(link).ok()?;
            let target = std::path::Path::new(target);
            let resolved = if rest.as_os_str().is_empty() { target.to_path_buf() } else { target.join(rest) };
            Some((link.len(), resolved))
        })
        .max_by_key(|(length, _)| *length)
        .map(|(_, resolved)| resolved.to_string_lossy().into_owned());
    let path = linked.as_deref().unwrap_or(path);
    protection.mounts.iter()
        .filter(|(point, _)| path_on_drive(path, point))
        .max_by_key(|(point, _)| std::path::Path::new(point).components().count())
        .is_some_and(|(_, read_only)| *read_only)
}

fn fixed_archive(root: &std::path::Path) -> bool {
    !oxyde_lib::utils::archive::ArchiveFormat::from_path(root)
        .is_some_and(oxyde_lib::utils::archive::ArchiveFormat::writable)
}

/// Wait before creating again a watcher that failed, unless the drives change.
const WATCHER_RETRY: std::time::Duration = std::time::Duration::from_secs(60);

/// Whether `path` is the folder whose permissions refuse writes, or an entry directly in
/// it, which cannot be renamed, moved or removed either.
fn in_denied_folder(denied: Option<&str>, path: &str) -> bool {
    denied.is_some_and(|folder| {
        paths_match(path, folder)
            || std::path::Path::new(path).parent().is_some_and(|parent| paths_match(&parent.to_string_lossy(), folder))
    })
}

impl OxydeApp {
    /// Whether nothing can be written at `path` shown in `pane_id`: its volume
    /// refuses writes, the pane browses an archive Oxyde cannot rewrite, or the
    /// permissions of the folder it shows refuse the user.
    pub(crate) fn is_read_only_location(&self, pane_id: &str, path: &str) -> bool {
        let tab = self.state.find_pane(pane_id).map(|pane| pane.active_tab());
        let in_fixed_archive = tab.and_then(|tab| tab.archive_root.as_deref()).is_some_and(fixed_archive);
        let denied = tab.is_some_and(|tab| {
            in_denied_folder(tab.denied_folder.as_deref(), path) || tab.undeletable_entries.contains(path)
        });
        in_fixed_archive || denied || on_read_only_volume(path, &self.write_protection)
    }

    /// Whether nothing can be written into the folder `path`, wherever it is
    /// shown: its volume refuses writes, it lies in an archive a tab browses
    /// and Oxyde cannot rewrite, or a tab listing it, or its parent, found its
    /// permissions refuse the user. A folder no tab lists, as in the sidebar, is not
    /// checked here: the operation reports it.
    pub(crate) fn is_read_only_path(&self, path: &str) -> bool {
        let mut refused = false;
        self.state.layout.visit_panes(&mut |pane| {
            refused |= pane.tabs.iter().any(|tab| {
                tab.archive_root.as_deref()
                    .is_some_and(|root| fixed_archive(root) && path_on_drive(path, &root.to_string_lossy()))
                    || tab.denied_folder.as_deref().is_some_and(|folder| paths_match(path, folder))
                    || tab.denied_subfolders.contains(path)
            });
        });
        refused || on_read_only_volume(path, &self.write_protection)
    }
}

/// The innermost drive holding `path`: under Linux, `/` holds every other mount.
fn drive_of<'a>(path: &str, drives: &'a [oxyde_lib::models::DriveInfo]) -> Option<&'a oxyde_lib::models::DriveInfo> {
    drives
        .iter()
        .filter(|drive| path_on_drive(path, &drive.path))
        .max_by_key(|drive| std::path::Path::new(&drive.path).components().count())
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
                    paths_match(&old.path, &root.path) && old.drive_type == root.drive_type
                })
                .cloned()
                .unwrap_or(root)
        })
        .collect()
}

impl OxydeApp {
    /// Unmounts or ejects a drive offered by `Volumes::actions`.
    pub(crate) fn release_drive(&mut self, path: String, eject: bool, cx: &mut Context<Self>) {
        self.context_menus.clear();
        let volumes = oxyde_lib::platform::platform().volumes();
        if self.ejecting_drive || !self.state.drives.iter().any(|drive| {
            let actions = volumes.actions(drive);
            drive.path == path && if eject { actions.eject } else { actions.unmount }
        }) {
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
                use oxyde_lib::platform::VolumeError;
                // Release watches and wait for their handles to close: an open handle keeps
                // the volume busy.
                for (watched_path, watcher) in watchers {
                    // A watch already gone (deleted folder) does not keep the volume busy.
                    if let Err(error) = watcher.release(&watched_path) {
                        log::warn!("Releasing watcher on {watched_path} failed: {error}");
                    }
                }
                let volumes = oxyde_lib::platform::platform().volumes();
                if eject { volumes.eject(&path)? } else { volumes.unmount(&path)? }
                Ok::<_, VolumeError>((oxyde_lib::commands::system::get_drives(Some(false)), volumes.unmounted()))
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.ejecting_drive = false;
                let error = match result {
                    Ok(Ok((drives, unmounted))) => {
                        // An unmounted partition comes back as a volume to mount.
                        this.state.unmounted_volumes = unmounted;
                        this.apply_drive_snapshot(drives, cx);
                        None
                    },
                    // The user dismissed the authentication dialog.
                    Ok(Err(oxyde_lib::platform::VolumeError::Cancelled)) => None,
                    Ok(Err(oxyde_lib::platform::VolumeError::Busy(holders))) if !holders.is_empty() => {
                        let mut args = crate::ui::i18n::FluentArgs::new();
                        args.set("applications", holders.join(", "));
                        Some(cx.i18n().text_with_args(Message::DriveHolders, &args).to_string())
                    }
                    Ok(Err(error)) => Some(error.to_string()),
                    Err(error) => Some(error.to_string()),
                };
                if let Some(error) = error {
                    let message = if eject { Message::EjectDriveFailed } else { Message::UnmountDriveFailed };
                    this.toast(ToastKind::Error, message, error, 0, cx);
                }
                this.reconcile_watchers(cx);
                cx.notify();
            });
        }).detach();
    }

    /// Mounts a sidebar volume in the background, then opens it in the pane active at the request.
    pub(crate) fn mount_volume(&mut self, id: String, cx: &mut Context<Self>) {
        self.context_menus.clear();
        if self.mounting_volume.is_some() || self.ejecting_drive {
            return;
        }
        self.mounting_volume = Some(id.clone());
        let origin_pane = self.state.active_pane_id.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            use oxyde_lib::platform::VolumeError;
            let result = tokio::task::spawn_blocking(move || {
                let volumes = oxyde_lib::platform::platform().volumes();
                let path = volumes.mount(&id)?;
                Ok::<_, VolumeError>((path, oxyde_lib::commands::system::get_drives(Some(false)), volumes.unmounted()))
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.mounting_volume = None;
                let failure = match result {
                    Ok(Ok((path, drives, unmounted))) => {
                        this.drive_refresh_epoch = this.drive_refresh_epoch.wrapping_add(1);
                        this.state.unmounted_volumes = unmounted;
                        this.apply_drive_snapshot(drives, cx);
                        this.reconcile_watchers(cx);
                        this.sidebar_selected_path = Some(path.clone());
                        // The pane that asked, unless it was closed meanwhile.
                        let pane = if this.state.find_pane(&origin_pane).is_some() {
                            origin_pane
                        } else {
                            this.state.active_pane_id.clone()
                        };
                        this.navigate_to(&pane, path, cx);
                        None
                    }
                    // The user dismissed the authentication dialog.
                    Ok(Err(VolumeError::Cancelled)) => None,
                    Ok(Err(VolumeError::WindowsHibernated)) => Some((Message::DriveMountHibernated, String::new())),
                    Ok(Err(error)) => Some((Message::DriveMountFailed, error.to_string())),
                    Err(error) => Some((Message::DriveMountFailed, error.to_string())),
                };
                if let Some((message, detail)) = failure {
                    this.toast(ToastKind::Error, message, detail, 0, cx);
                }
                cx.notify();
            });
        }).detach();
    }

    fn start_drive_refresh_task(&mut self, cx: &mut Context<Self>) {
        let h = self.handle.clone().expect("handle set");
        self._drive_refresh_task = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let revision = std::sync::Arc::new(oxyde_lib::utils::change_signal::ChangeSignal::new());
                let registration_revision = revision.clone();
                let registration = tokio::task::spawn_blocking(move || {
                    oxyde_lib::platform::platform().volumes().watch(registration_revision)
                }).await;
                let _registration = match registration {
                    Ok(Ok(guard)) => guard,
                    Ok(Err(error)) => { log::warn!("Drive notifications unavailable: {error}"); None }
                    Err(error) => { log::warn!("Drive notification worker failed: {error}"); None }
                };
                // A drive missing for a moment keeps its tabs and their history.
                let mut absences = absences::DriveAbsences::default();
                let mut schedule = drive_watchers::DriveRefreshSchedule::new(
                    _registration.is_some(), std::time::Instant::now());
                loop {
                    let paused = h.update(&mut cx, |this, _| this.ejecting_drive);
                    let observed = revision.value();
                    if !paused && schedule.due(observed, std::time::Instant::now()) {
                        let details_due = schedule.details_due;
                        let (epoch, policy_revision) = h.update(&mut cx, |this, _| {
                            (this.drive_refresh_epoch, this.trash_delete_policy.revision())
                        });
                        if let Ok((roots, unmounted, policies)) = tokio::task::spawn_blocking(|| {
                            let roots = oxyde_lib::platform::platform().volumes().roots();
                            let unmounted = oxyde_lib::platform::platform().volumes().unmounted();
                            super::drag::volumes::replace_mount_points(
                                oxyde_lib::platform::platform().volumes().mount_points());
                            let policies = super::trash_settings::read_drive_policy(&roots);
                            let protection = oxyde_lib::platform::platform().volumes().write_protection(&roots);
                            (roots, unmounted, (policies, protection))
                        }).await {
                            h.update(&mut cx, |this, cx| {
                                if this.ejecting_drive || this.drive_refresh_epoch != epoch
                                    || revision.value() != observed { return; }
                                let mut roots = roots;
                                let missing: Vec<_> = this.state.drives.iter()
                                    .filter(|old| !roots.iter().any(|root| paths_match(&root.path, &old.path)))
                                    .cloned()
                                    .collect();
                                roots.extend(absences.still_shown(missing, |old| old.path.as_str(), std::time::Instant::now()));
                                let changed = roots.len() != this.state.drives.len()
                                    || roots.iter().any(|root| !this.state.drives.iter()
                                        .any(|old| old.path == root.path && old.drive_type == root.drive_type));
                                if changed {
                                    this.drive_refresh_epoch = this.drive_refresh_epoch.wrapping_add(1);
                                }
                                // apply_drive_roots rebuilds the sidebar.
                                this.state.unmounted_volumes = unmounted;
                                this.apply_drive_roots(roots, cx);
                                let (policies, protection) = policies;
                                if this.trash_delete_policy.replace(policy_revision, policies) {
                                    cx.notify();
                                }
                                if this.write_protection != protection {
                                    this.write_protection = protection;
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
                    // Woken by a notification or when the schedule is due; a release in
                    // progress, which defers every survey, is checked again shortly.
                    let wait = if paused {
                        std::time::Duration::from_millis(100)
                    } else {
                        schedule.next.saturating_duration_since(std::time::Instant::now())
                    };
                    let timer = cx.background_executor().timer(wait);
                    futures::future::select(std::pin::pin!(timer), std::pin::pin!(revision.changed())).await;
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
                        oxyde_lib::platform::platform().file_clipboard().has_file_format()
                    }).await.unwrap_or(false);
                    h.update(&mut cx, |this, cx| {
                        // Files another file manager copied, from the types its offer announced.
                        let clipboard_has_files = clipboard_has_files
                            || oxyde_lib::platform::platform().file_clipboard().external_files_types()
                                .iter().any(|mime_type| cx.clipboard_offers(mime_type));
                        if this.clipboard_has_files != clipboard_has_files {
                            this.clipboard_has_files = clipboard_has_files;
                            cx.notify();
                        }
                    });
                    // Changes wait in the queue while a drive is being released.
                    let ejecting = h.update(&mut cx, |this, _| this.ejecting_drive);
                    let changed = if ejecting {
                        HashMap::new()
                    } else if let Ok(mut q) = changes.lock() {
                        std::mem::take(&mut *q)
                    } else {
                        HashMap::new()
                    };
                    if !changed.is_empty() {
                        h.update(&mut cx, |this, cx| {
                            if this.ejecting_drive { return; }
                            // Refresh only affected expanded paths off the UI thread.
                            for (path, change) in &changed {
                                if !change.listing { continue; }
                                if this.state.expanded_paths.contains(path)
                                    || this.state.drives.iter().any(|drive| &drive.path == path) {
                                    this.request_sidebar_nodes(path.clone(), true, cx);
                                }
                            }

                            // Reload every affected tab (active = full refresh, inactive = files only).
                            let mut reloads = Vec::new();
                            let mut size_updates = Vec::new();
                            this.state.layout.visit_panes_mut(&mut |p| {
                                for (idx, tab) in p.tabs.iter_mut().enumerate() {
                                    let watched = tab.watched_path();
                                    if let Some(change) = changed.get(&watched) {
                                        tab.invalidate_folder_sizes(if change.all_sizes { None } else { Some(&change.children) });
                                        size_updates.push(tab.id.clone());
                                        if change.listing {
                                            reloads.push((p.id.clone(), idx, tab.path.clone(), tab.id.clone()));
                                        }
                                    }
                                }
                            });
                            for (pane_id, tab_idx, path, id) in reloads {
                                if this.pending_file_requests.contains_key(&id) {
                                    this.watcher_pending_listings.insert(id);
                                } else {
                                    this.request_tab_listing(&pane_id, tab_idx, path, false, cx);
                                }
                            }
                            for id in size_updates {
                                this.schedule_folder_sizes(&id, cx);
                            }
                            // A published size can become provisional even while
                            // another scan already owns this tab's scheduler.
                            cx.notify();
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
                tokio::task::spawn_blocking(|| oxyde_lib::platform::platform().volumes().list(false))
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
                                .find(|detail| paths_match(&current.path, &detail.path))
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
                    .any(|new| paths_match(&new.path, &old.path))
            })
            .map(|drive| drive.path.clone())
            .collect();
        let retained = |path: &str| !removed.iter().any(|root| path_on_drive(path, root));
        self.state.expanded_paths.retain(|path| retained(path));
        self.state.sidebar_nodes.retain(|path, _| retained(path));
        std::sync::Arc::make_mut(&mut self.folder_has_subdirs).retain(|path, _| retained(path));
        self.pending_sidebar_requests
            .retain(|path, _| retained(path));
        // An « Expand all » walk on a removed drive must not bring its folders back.
        self.sidebar_bulk_expands.retain(|root, cancel| {
            let keep = retained(root);
            if !keep {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            keep
        });
        if self
            .sidebar_selected_path
            .as_deref()
            .is_some_and(|path| !retained(path))
        {
            self.sidebar_selected_path = None;
        }
        self.state.drives = drives;
        // Inserted media or a new drive: try the failed watchers again at once.
        self.failed_watchers.clear();
        self.redirect_removed_drive_tabs(&removed, cx);
        self.state.refresh_sidebar();
        cx.notify();
    }

    pub fn reconcile_watchers(&mut self, cx: &mut Context<Self>) {
        if self.ejecting_drive || !self.drive_metadata_ready {
            return;
        }
        let mut needed = HashSet::new();
        let mut recursive_paths = HashSet::new();
        let mut tab_ids = HashSet::new();
        self.state.layout.visit_panes(&mut |pane| {
            tab_ids.extend(pane.tabs.iter().map(|tab| tab.id.clone()));
            recursive_paths.extend(pane.tabs.iter()
                .filter(|tab| tab.show_size_histogram && tab.archive_root.is_none())
                .map(|tab| tab.path.clone()));
            needed.extend(
                pane.tabs
                    .iter()
                    .filter(|tab| {
                        // Virtual locations have no directory to watch: the attempt would
                        // fail and be logged again at every reconciliation.
                        !tab.is_viewer()
                            && !tab.path.is_empty()
                            && !tab.path.contains("://")
                            && tab.path != "__network_vincinity__"
                    })
                    .map(|tab| tab.watched_path()),
            );
        });
        self.watcher_pending_listings.retain(|id| tab_ids.contains(id));
        needed.extend(self.state.expanded_paths.iter().cloned());
        needed.extend(self.state.drives.iter().map(|drive| drive.path.clone()));
        // Not a directory: only its distributions can be watched.
        needed.remove(oxyde_lib::platform::WSL_ROOT);
        let wanted: HashMap<_, _> = needed
            .into_iter()
            .map(|path| {
                let polling = drive_of(&path, &self.state.drives)
                    .is_some_and(|drive| oxyde_lib::platform::platform().volumes().polls(drive));
                let recursive = recursive_paths.contains(&path);
                (path, (polling, recursive))
            })
            .collect();
        self.pending_watchers
            .retain(|path, (_, polling, recursive)| wanted.get(path) == Some(&(*polling, *recursive)));
        let obsolete: Vec<_> = self
            ._path_watchers
            .iter()
            .filter(|(path, watcher)| wanted.get(*path) != Some(&(watcher.polling, watcher.recursive)))
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
        self.failed_watchers.retain(|path, _| wanted.contains_key(path));
        for (path, (polling, recursive)) in wanted {
            if self._path_watchers.contains_key(&path) || self.pending_watchers.contains_key(&path)
            {
                continue;
            }
            if self.failed_watchers.get(&path).is_some_and(|failed| failed.elapsed() < WATCHER_RETRY) {
                continue;
            }
            self.next_watcher_request = self.next_watcher_request.wrapping_add(1);
            let request = self.next_watcher_request;
            self.pending_watchers
                .insert(path.clone(), (request, polling, recursive));
            let changes = self.watcher_changes.clone();
            // Folders of one polled volume share its polling thread.
            let volume = drive_of(&path, &self.state.drives).filter(|_| polling).map(|drive| drive.path.clone());
            cx.spawn(async move |this, cx| {
                let worker_path = path.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move { DirectoryWatcher::create(&worker_path, volume.as_deref(), recursive, changes) })
                    .await;
                let error = result.as_ref().err().map(ToString::to_string);
                let mut result = result.ok();
                let _ = this.update(cx, |this, cx| {
                    if !this.ejecting_drive
                        && this.pending_watchers.get(&path) == Some(&(request, polling, recursive))
                    {
                        this.pending_watchers.remove(&path);
                        let installed = result.is_some();
                        if let Some(watcher) = result.take() {
                            this._path_watchers.insert(path.clone(), watcher);
                            this.failed_watchers.remove(&path);
                        } else if let Some(error) = &error {
                            // Logged once until the path can be watched again.
                            if this.failed_watchers.insert(path.clone(), std::time::Instant::now()).is_none() {
                                log::warn!("Cannot watch {path}: {error}");
                            }
                        }
                        {
                            let mut ids = Vec::new();
                            this.state.layout.visit_panes(&mut |pane| {
                                ids.extend(pane.tabs.iter()
                                    .filter(|tab| tab.path == path && tab.show_size_histogram)
                                    .map(|tab| tab.id.clone()));
                            });
                            for id in ids {
                                this.resume_folder_sizes_after_watcher(&id, installed && recursive, cx);
                            }
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
            tab.reset_folder_sizes();
            tab.path = fallback.to_owned();
            tab.search_revision = tab.search_revision.wrapping_add(1);
            tab.search_input = None;
            tab.search_text.clear();
            tab.pending_reveal = None;
            if let Some(search) = tab.quick_search.take() {
                search.cancellation.0.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            tab.clear_files();
            tab.selected_paths.clear();
            tab.selection_anchor = None;
            tab.load_status = crate::ui::workspace::TabLoadStatus::Loading;
            requests.push((pane.id.clone(), index, tab.path.clone()));
        }
    });
    requests
}

#[cfg(all(test, not(target_os = "windows")))]
#[path = "drive_watchers/tests/polling.rs"]
mod polling_tests;

#[cfg(test)]
#[path = "drive_watchers/tests/read_only.rs"]
mod read_only_tests;

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
            file_system: None,
            volume_bytes: 0,
        };
        let mut root = known.clone();
        root.total_bytes = 0;
        root.media_type = None;
        let mut usb = root.clone();
        usb.path = "E:\\".into();
        usb.drive_type = "removable".into();
        let snapshot = merge_drive_roots(vec![root, usb.clone()], std::slice::from_ref(&known));
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
            content_warnings: Default::default(),
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
        let changes = Arc::new(Mutex::new(HashMap::new()));
        let watcher = DirectoryWatcher::create(&path, Some(&path), false, changes.clone()).unwrap();
        changes.lock().unwrap().clear();
        std::fs::create_dir(directory.path().join("new-folder")).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
        while changes.lock().unwrap().is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(changes.lock().unwrap().contains_key(&path));
        watcher.release(&path).unwrap();
        directory.close().unwrap();
    }
}
