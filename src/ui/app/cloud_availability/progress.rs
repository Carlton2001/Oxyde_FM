use super::super::{OxydeApp, ToastKind};
use crate::ui::i18n::Message;
use crate::ui::paths::{path_within, paths_match};
use gpui::{AsyncApp, Context};
use oxyde_lib::models::{CloudAvailability, CloudStatus, FileEntry};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    io::ErrorKind,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

const CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const FIRST_CHECK: Duration = Duration::from_millis(500);
const MAX_CHECK_INTERVAL: Duration = Duration::from_secs(4);
const LISTING_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy)]
struct Badge {
    request: u64,
    availability: CloudAvailability,
    status: CloudStatus,
}

impl Badge {
    fn displayed_status(self, file: &FileEntry) -> CloudStatus {
        file.cloud_status
            .filter(|status| {
                !file.is_dir
                    && self.status == CloudStatus::Syncing
                    && self.availability.is_confirmed_by(*status)
            })
            .unwrap_or(self.status)
    }
}

#[derive(Clone, Default)]
pub(crate) struct CloudBadges(HashMap<String, Badge>);

impl CloudBadges {
    pub(crate) fn status(&self, file: &FileEntry) -> Option<CloudStatus> {
        self.0
            .get(&file.path)
            .map(|badge| badge.displayed_status(file))
            .or(file.cloud_status)
    }

    pub(crate) fn entry<'a>(&self, file: &'a FileEntry) -> Cow<'a, FileEntry> {
        let Some(badge) = self.0.get(&file.path) else {
            return Cow::Borrowed(file);
        };
        let status = badge.displayed_status(file);
        let mut file = file.clone();
        file.cloud_status = Some(status);
        // A preview may bring an online-only file back while the listing still
        // has its old metadata. Defer thumbnail and tooltip reads meanwhile.
        file.is_cloud_placeholder |=
            matches!(status, CloudStatus::Syncing | CloudStatus::OnlineOnly);
        Cow::Owned(file)
    }

    fn start(
        &mut self,
        paths: Vec<String>,
        request: u64,
        availability: CloudAvailability,
    ) -> Vec<String> {
        paths
            .into_iter()
            .filter(|path| {
                if self.0.get(path).is_some_and(|badge| {
                    badge.status == CloudStatus::Syncing && badge.availability == availability
                }) {
                    return false;
                }
                self.0.insert(
                    path.clone(),
                    Badge {
                        request,
                        availability,
                        status: CloudStatus::Syncing,
                    },
                );
                true
            })
            .collect()
    }

    fn release(&mut self, paths: &[String], request: u64) {
        for path in paths {
            if self
                .0
                .get(path)
                .is_some_and(|badge| badge.request == request)
            {
                self.0.remove(path);
            }
        }
    }
}

impl OxydeApp {
    pub(crate) fn cloud_badge_snapshot(&self) -> Arc<CloudBadges> {
        self.cloud_badges.clone()
    }

    pub(in crate::ui::app) fn change_cloud_availability(
        &mut self,
        paths: Vec<String>,
        availability: CloudAvailability,
        cx: &mut Context<Self>,
    ) {
        self.context_menus.clear();
        self.next_cloud_change = self.next_cloud_change.wrapping_add(1);
        let request = self.next_cloud_change;
        let paths = Arc::make_mut(&mut self.cloud_badges).start(paths, request, availability);
        cx.notify();
        if paths.is_empty() {
            return;
        }
        cx.spawn(async move |this, cx| {
            let deadline = Instant::now() + CONFIRMATION_TIMEOUT;
            let targets = paths.clone();
            let launch = tokio::task::spawn_blocking(move || {
                let platform = oxyde_lib::platform::platform();
                targets
                    .into_iter()
                    .filter(
                        |path| match platform.set_cloud_availability(path, availability) {
                            Ok(()) => false,
                            Err(error) => {
                                log::warn!("Cloud availability change failed for {path}: {error}");
                                true
                            }
                        },
                    )
                    .collect::<Vec<_>>()
            });
            let (failed, timed_out) = match before_deadline(launch, deadline, cx).await {
                Some(Ok(failed)) => (failed, false),
                Some(Err(error)) => {
                    log::warn!("Cloud availability task failed: {error}");
                    (paths.clone(), false)
                }
                None => (Vec::new(), true),
            };
            if !failed.is_empty()
                && this
                    .update(cx, |this, cx| {
                        this.release_cloud_badges(&failed, request, cx);
                        this.refresh_cloud_views(&failed, cx);
                        this.toast(
                            ToastKind::Error,
                            Message::CloudAvailabilityFailed,
                            String::new(),
                            0,
                            cx,
                        );
                    })
                    .is_err()
            {
                return;
            }
            let failed: HashSet<_> = failed.into_iter().collect();
            let mut remaining: HashMap<_, _> = paths
                .into_iter()
                .filter(|path| !failed.contains(path))
                .map(|path| (path, deadline))
                .collect();
            let mut interval = FIRST_CHECK;
            let mut refreshed_while_waiting = false;
            while !timed_out && !remaining.is_empty() {
                if this
                    .update(cx, |this, _| {
                        remaining.retain(|path, _| {
                            this.cloud_badges
                                .0
                                .get(path)
                                .is_some_and(|badge| badge.request == request)
                        });
                    })
                    .is_err()
                {
                    return;
                }
                if remaining.is_empty() {
                    break;
                }
                let targets = remaining.keys().cloned().collect::<Vec<_>>();
                let sample =
                    tokio::task::spawn_blocking(move || read_confirmations(targets, availability));
                let Some(Ok(checks)) =
                    before_deadline(sample, Instant::now() + CONFIRMATION_TIMEOUT, cx).await
                else {
                    break;
                };
                let now = Instant::now();
                let mut finished = Vec::new();
                for (path, check) in checks {
                    match check {
                        Confirmation::Ready(status) => {
                            remaining.remove(&path);
                            finished.push((path, status));
                        }
                        Confirmation::Pending => {
                            // Keep monitoring as long as the provider reports unfinished work.
                            remaining.insert(path, now + CONFIRMATION_TIMEOUT);
                        }
                        Confirmation::Unknown if remaining[&path] <= now => {
                            remaining.remove(&path);
                            finished.push((path, None));
                        }
                        Confirmation::Unknown => {}
                    }
                }
                if !finished.is_empty() {
                    if this
                        .update(cx, |this, cx| {
                            this.publish_cloud_badges(finished, request, cx)
                        })
                        .is_err()
                    {
                        return;
                    }
                }
                if !remaining.is_empty() {
                    if !refreshed_while_waiting {
                        let paths = remaining.keys().cloned().collect::<Vec<_>>();
                        if this
                            .update(cx, |this, cx| this.refresh_cloud_views(&paths, cx))
                            .is_err()
                        {
                            return;
                        }
                        refreshed_while_waiting = true;
                    }
                    cx.background_executor().timer(interval).await;
                    interval = (interval * 2).min(MAX_CHECK_INTERVAL);
                }
            }
            if !remaining.is_empty() {
                let paths = remaining.into_keys().collect::<Vec<_>>();
                let _ = this.update(cx, |this, cx| {
                    this.release_cloud_badges(&paths, request, cx);
                    this.refresh_cloud_views(&paths, cx);
                });
            }
        })
        .detach();
    }

    fn release_cloud_badges(&mut self, paths: &[String], request: u64, cx: &mut Context<Self>) {
        Arc::make_mut(&mut self.cloud_badges).release(paths, request);
        cx.notify();
    }

    fn publish_cloud_badges(
        &mut self,
        finished: Vec<(String, Option<CloudStatus>)>,
        request: u64,
        cx: &mut Context<Self>,
    ) {
        let badges = &mut Arc::make_mut(&mut self.cloud_badges).0;
        let mut confirmed = Vec::new();
        let mut refresh = Vec::new();
        for (path, status) in finished {
            let Some(badge) = badges
                .get_mut(&path)
                .filter(|badge| badge.request == request)
            else {
                continue;
            };
            if let Some(status) = status {
                badge.status = status;
                confirmed.push(path.clone());
            } else {
                badges.remove(&path);
            }
            refresh.push(path);
        }
        let snapshots = self.refresh_cloud_views(&refresh, cx);
        cx.notify();
        if confirmed.is_empty() {
            return;
        }
        // Keep the confirmed status visible until the refreshed tab snapshots replace the old ones.
        cx.spawn(async move |this, cx| {
            let deadline = Instant::now() + LISTING_TIMEOUT;
            while Instant::now() < deadline {
                let Ok(waiting) = this.update(cx, |this, _| {
                    snapshots.iter().any(|(id, files)| {
                        this.pending_file_requests.contains_key(id)
                            || this
                                .state
                                .find_tab_mut(id)
                                .is_some_and(|tab| Arc::ptr_eq(&tab.files, files))
                    })
                }) else {
                    return;
                };
                if !waiting {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
            }
            let _ = this.update(cx, |this, cx| {
                this.release_cloud_badges(&confirmed, request, cx)
            });
        })
        .detach();
    }

    fn refresh_cloud_views(
        &mut self,
        paths: &[String],
        cx: &mut Context<Self>,
    ) -> Vec<(String, Arc<Vec<FileEntry>>)> {
        if paths.is_empty() {
            return Vec::new();
        }
        let targets: HashSet<_> = paths.iter().map(String::as_str).collect();
        let mut requests = Vec::new();
        self.state.layout.visit_panes(&mut |pane| {
            for (index, tab) in pane.tabs.iter().enumerate() {
                if tab
                    .files
                    .iter()
                    .any(|file| targets.contains(file.path.as_str()))
                    || paths
                        .iter()
                        .any(|path| cloud_location_affected(&tab.path, path))
                {
                    requests.push((
                        pane.id.clone(),
                        index,
                        tab.path.clone(),
                        tab.id.clone(),
                        tab.files.clone(),
                    ));
                }
            }
        });
        requests
            .into_iter()
            .map(|(pane, index, path, id, files)| {
                // Coalesce with a refresh the filesystem watcher already started.
                if self.pending_file_requests.contains_key(&id) {
                    self.watcher_pending_listings.insert(id.clone());
                } else {
                    self.request_tab_listing(&pane, index, path, false, cx);
                }
                (id, files)
            })
            .collect()
    }
}

fn cloud_location_affected(location: &str, target: &str) -> bool {
    let target = Path::new(target);
    path_within(Path::new(location), target)
        || target
            .parent()
            .and_then(Path::to_str)
            .is_some_and(|parent| paths_match(location, parent))
}

async fn before_deadline<T>(
    task: tokio::task::JoinHandle<T>,
    deadline: Instant,
    cx: &AsyncApp,
) -> Option<Result<T, tokio::task::JoinError>> {
    let timer = cx
        .background_executor()
        .timer(deadline.saturating_duration_since(Instant::now()));
    match futures::future::select(task, timer).await {
        futures::future::Either::Left((result, _)) => Some(result),
        futures::future::Either::Right(_) => None,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Confirmation {
    Ready(Option<CloudStatus>),
    Pending,
    Unknown,
}

fn read_confirmations(
    paths: Vec<String>,
    availability: CloudAvailability,
) -> Vec<(String, Confirmation)> {
    let platform = oxyde_lib::platform::platform();
    let mut probes = HashMap::new();
    paths
        .into_iter()
        .map(|path| {
            let check = (|| {
                let disk_path = oxyde_lib::utils::path_text::decode(&path);
                let parent = disk_path.parent()?;
                let metadata = match std::fs::symlink_metadata(&disk_path) {
                    Ok(metadata) => metadata,
                    Err(error) if error.kind() == ErrorKind::NotFound => {
                        return Some(Confirmation::Ready(None));
                    }
                    Err(_) => return None,
                };
                let probe = probes
                    .entry(parent.to_owned())
                    .or_insert_with(|| platform.cloud_status_probe(parent));
                let status = probe.as_ref()?(&disk_path, &metadata)?;
                Some(check_confirmation(availability, status, || {
                    if metadata.is_dir() {
                        platform.cloud_folder_contents_ready(&disk_path, availability)
                    } else {
                        Some(true)
                    }
                }))
            })()
            .unwrap_or(Confirmation::Unknown);
            (path, check)
        })
        .collect()
}

fn check_confirmation(
    availability: CloudAvailability,
    status: CloudStatus,
    contents_ready: impl FnOnce() -> Option<bool>,
) -> Confirmation {
    if matches!(status, CloudStatus::Syncing | CloudStatus::SyncPending) {
        return Confirmation::Pending;
    }
    if !availability.is_confirmed_by(status) {
        return Confirmation::Unknown;
    }
    match contents_ready() {
        Some(true) => Confirmation::Ready(Some(status)),
        Some(false) => Confirmation::Pending,
        None => Confirmation::Unknown,
    }
}

#[cfg(test)]
#[path = "tests/progress.rs"]
mod tests;
