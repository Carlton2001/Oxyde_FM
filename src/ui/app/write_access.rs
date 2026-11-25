//! Subfolders of a listed folder whose permissions refuse the user's writes, and entries
//! the user may not remove, asked after the listing so that a large folder shows without
//! waiting for them.
use super::OxydeApp;
use gpui::*;
use std::collections::HashSet;

impl OxydeApp {
    pub(super) fn request_denied_subfolders(
        &mut self,
        tab_id: String,
        folder: String,
        subfolders: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        if folder.contains("://") {
            return;
        }
        cx.spawn(async move |this, cx| {
            let listed = folder.clone();
            let Ok((denied, undeletable)) = tokio::task::spawn_blocking(move || {
                let platform = oxyde_lib::platform::platform();
                let folder = std::path::Path::new(&listed);
                let denied = if subfolders.is_empty() { Vec::new() } else { platform.denied_subfolders(folder, subfolders) };
                (
                    denied.into_iter().collect::<HashSet<_>>(),
                    platform.undeletable_entries(folder).into_iter().collect::<HashSet<_>>(),
                )
            })
            .await
            else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                // A tab gone elsewhere meanwhile keeps nothing; drops read it on their next move.
                if let Some(tab) = this.state.find_tab_mut(&tab_id).filter(|tab| tab.path == folder) {
                    let changed = tab.undeletable_entries.as_ref() != &undeletable;
                    tab.denied_subfolders = std::sync::Arc::new(denied);
                    tab.undeletable_entries = std::sync::Arc::new(undeletable);
                    // The toolbar reflects the selection's actions.
                    if changed {
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
}
