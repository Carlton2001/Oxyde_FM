//! Subfolders of a listed folder whose permissions refuse the user's writes, asked after
//! the listing so that a large folder shows without waiting for them.
use super::OxydeApp;
use gpui::*;

impl OxydeApp {
    pub(super) fn request_denied_subfolders(
        &mut self,
        tab_id: String,
        folder: String,
        subfolders: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        if subfolders.is_empty() {
            return;
        }
        cx.spawn(async move |this, cx| {
            let listed = folder.clone();
            let Ok(denied) = tokio::task::spawn_blocking(move || {
                oxyde_lib::platform::platform()
                    .denied_subfolders(std::path::Path::new(&listed), subfolders)
                    .into_iter()
                    .collect::<std::collections::HashSet<_>>()
            })
            .await
            else {
                return;
            };
            let _ = this.update(cx, |this, _| {
                // A tab gone elsewhere meanwhile keeps nothing; drops read it on their next move.
                if let Some(tab) = this.state.find_tab_mut(&tab_id).filter(|tab| tab.path == folder) {
                    tab.denied_subfolders = std::sync::Arc::new(denied);
                }
            });
        })
        .detach();
    }
}
