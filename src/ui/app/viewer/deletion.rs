//! Keep deletion behind the actual release of native files, including work
//! already running when its UI task is cancelled.
use super::{content::Content, OxydeApp, Viewer};
use futures::{
    future::{BoxFuture, Shared},
    FutureExt,
};
use gpui::*;
use std::path::{Path, PathBuf};

type Completion = futures::channel::oneshot::Sender<()>;
type Pending = Shared<BoxFuture<'static, ()>>;

#[derive(Default)]
pub(super) struct Jobs(Vec<(String, Pending)>);

impl Jobs {
    pub(super) fn register(&mut self, path: &str) -> Completion {
        self.0
            .retain(|(_, job)| job.clone().now_or_never().is_none());
        let (done, finished) = futures::channel::oneshot::channel();
        self.0.push((
            path.to_owned(),
            async move {
                let _ = finished.await;
            }
            .boxed()
            .shared(),
        ));
        done
    }

    fn pending(&self, paths: &[PathBuf]) -> Vec<Pending> {
        self.0
            .iter()
            .filter(|(path, _)| contains(paths, path))
            .map(|(_, job)| job.clone())
            .collect()
    }
}

fn contains(paths: &[PathBuf], path: &str) -> bool {
    // Path components keep a sibling such as `folder-other` out of the match.
    // Windows explorer paths may differ in case or separator spelling.
    #[cfg(target_os = "windows")]
    let normalize =
        |path: &Path| PathBuf::from(path.to_string_lossy().replace('/', "\\").to_lowercase());
    #[cfg(not(target_os = "windows"))]
    let normalize = |path: &Path| path.to_path_buf();
    let path = normalize(Path::new(path));
    paths.iter().any(|root| path.starts_with(normalize(root)))
}

impl Viewer {
    pub(super) fn load_work<T, E>(
        &mut self,
        work: impl FnOnce() -> Result<T, E> + Send + 'static,
    ) -> impl std::future::Future<Output = Result<(T, Completion), String>> + Send + 'static
    where
        T: Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        let completion = self
            .jobs
            .register(self.file.as_ref().map_or("", |file| file.path.as_str()));
        crate::ui::preview_jobs::load_viewer(move || {
            // Carry the completion with the result: an unpublished PDF session
            // must be dropped before a deletion can pass this fence.
            work().map(|value| (value, completion))
        })
    }

    pub(super) fn deletion_blocks(&self, path: &str) -> bool {
        self.deletions.values().any(|paths| contains(paths, path))
    }
}

impl OxydeApp {
    pub(in super::super) fn suspend_viewers_for_deletion(
        &mut self,
        id: &str,
        paths: &[PathBuf],
        cx: &mut Context<Self>,
    ) -> impl std::future::Future<Output = ()> + Send + use<> {
        let mut pending = Vec::new();
        for viewer in self.viewers.values() {
            viewer.update(cx, |viewer, cx| {
                viewer.deletions.insert(id.to_owned(), paths.to_vec());
                // load_current also checks this block when an asynchronous
                // selection resolution publishes after deletion starts.
                if viewer
                    .file
                    .as_ref()
                    .is_some_and(|file| contains(paths, &file.path))
                {
                    viewer.load_current(cx);
                }
                pending.extend(viewer.jobs.pending(paths));
                cx.notify();
            });
        }
        async move {
            futures::future::join_all(pending).await;
        }
    }

    pub(in super::super) fn resume_viewers_after_deletion(
        &mut self,
        id: &str,
        reload: bool,
        cx: &mut Context<Self>,
    ) {
        for viewer in self.viewers.values() {
            viewer.update(cx, |viewer, cx| {
                let Some(paths) = viewer.deletions.remove(id) else {
                    return;
                };
                if !reload
                    && viewer
                        .file
                        .as_ref()
                        .is_some_and(|file| contains(&paths, &file.path))
                {
                    // Do not reopen a deleted file while the directory refresh
                    // is still preparing the new selection in the background.
                    viewer.file = None;
                    viewer.neighbors = (None, None);
                    viewer.load_current(cx);
                    cx.notify();
                } else if reload
                    && viewer
                        .file
                        .as_ref()
                        .is_some_and(|file| !viewer.deletion_blocks(&file.path))
                    && matches!(viewer.content, Content::None)
                    && !viewer.native_media
                {
                    viewer.load_current(cx);
                    cx.notify();
                }
            });
        }
    }
}

#[cfg(test)]
#[path = "tests/deletion.rs"]
mod tests;
