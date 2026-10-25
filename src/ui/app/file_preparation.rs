use super::OxydeApp;
use crate::ui::workspace::FileUpdate;
use gpui::{AsyncApp, Context, Entity};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

// The permit belongs to physical blocking work, even if its UI task is dropped.
static PREPARATIONS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

pub(super) enum Owner {
    Listing {
        id: String,
        path: String,
        request: u64,
    },
    Search {
        id: String,
        path: String,
        cancellation: Arc<AtomicBool>,
    },
    Current {
        id: String,
        path: String,
    },
    FolderSizes {
        id: String,
        path: String,
        generation: u64,
        directories: Vec<(String, u64)>,
    },
}
impl Owner {
    fn id(&self) -> &str {
        match self {
            Self::Listing { id, .. }
            | Self::Search { id, .. }
            | Self::Current { id, .. }
            | Self::FolderSizes { id, .. } => id,
        }
    }
    fn matches(&self, app: &mut OxydeApp) -> bool {
        if let Self::Listing { id, request, .. } = self {
            if app.pending_file_requests.get(id) != Some(request) {
                return false;
            }
        }
        let Some(tab) = app.state.find_tab_mut(self.id()) else {
            return false;
        };
        match self {
            Self::FolderSizes {
                path, generation, ..
            } => {
                &tab.path == path
                    && tab.show_size_histogram
                    && tab.folder_sizes.generation == *generation
            }
            Self::Listing { path, .. } | Self::Current { path, .. } => &tab.path == path,
            Self::Search {
                path, cancellation, ..
            } => {
                &tab.path == path
                    && !cancellation.load(Ordering::Relaxed)
                    && tab
                        .quick_search
                        .as_ref()
                        .is_some_and(|search| Arc::ptr_eq(&search.cancellation.0, cancellation))
            }
        }
    }
}

pub(super) async fn prepare(
    handle: &Entity<OxydeApp>,
    cx: &mut AsyncApp,
    owner: &Owner,
    update: FileUpdate,
) -> bool {
    loop {
        let Ok(permit) = PREPARATIONS.acquire().await else {
            return false;
        };
        let request = handle.update(cx, |app, _| {
            if !owner.matches(app) {
                return None;
            }
            let tab = app.state.find_tab_mut(owner.id()).unwrap();
            let mut update = update.clone();
            let mut revisions = Vec::new();
            if let Owner::FolderSizes {
                generation,
                directories,
                ..
            } = owner
            {
                revisions = directories
                    .iter()
                    .filter(|(path, revision)| {
                        tab.folder_sizes
                            .publication_current(*generation, path, *revision)
                    })
                    .cloned()
                    .collect();
                if let FileUpdate::FolderSizes(updates) = &mut update {
                    updates
                        .retain(|(path, _)| revisions.iter().any(|(current, _)| current == path));
                    if updates.is_empty() {
                        return None;
                    }
                }
            }
            Some((tab.preparation(update), revisions))
        });
        let Some((request, revisions)) = request else {
            return false;
        };
        let result = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            request.calculate()
        })
        .await;
        let Ok(result) = result else {
            return false;
        };
        let decision = handle.update(cx, |app, cx| {
            if !owner.matches(app) {
                return None;
            }
            if let Owner::FolderSizes { generation, .. } = owner {
                let tab = app.state.find_tab_mut(owner.id()).unwrap();
                if revisions.iter().any(|(path, revision)| {
                    !tab.folder_sizes
                        .publication_current(*generation, path, *revision)
                }) {
                    // Retry only the still-current subset of the original batch;
                    // a busy folder must not starve unaffected publications.
                    return Some(false);
                }
            }
            let applied = app
                .state
                .find_tab_mut(owner.id())
                .unwrap()
                .apply_preparation(result);
            if applied {
                cx.notify();
            }
            Some(applied)
        });
        match decision {
            Some(true) => return true,
            Some(false) => continue,
            _ => return false,
        }
    }
}

impl OxydeApp {
    pub(crate) fn prepare_current_tab(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(tab) = self.state.find_tab_mut(&id) else {
            return;
        };
        tab.invalidate_preparation();
        let owner = Owner::Current {
            id: id.clone(),
            path: tab.path.clone(),
        };
        let Some(handle) = self.handle.clone() else {
            return;
        };
        self.next_preparation = self.next_preparation.wrapping_add(1);
        let generation = self.next_preparation;
        let key = id.clone();
        let task = cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                prepare(&handle, &mut cx, &owner, FileUpdate::Current).await;
                handle.update(&mut cx, |app, _| {
                    if app
                        .pending_preparations
                        .get(&key)
                        .is_some_and(|(current, _)| *current == generation)
                    {
                        app.pending_preparations.remove(&key);
                    }
                });
            }
        });
        self.pending_preparations.insert(id, (generation, task));
    }
}
