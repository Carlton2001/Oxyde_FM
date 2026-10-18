use super::*;
use oxyde_lib::models::{FileEntry, TrashEntry};

enum Update {
    Batch(Vec<TrashEntry>, bool),
    Finished(Result<(), String>),
}

impl OxydeApp {
    pub(super) fn request_trash_files(
        &mut self,
        tab_id: String,
        request_id: u64,
        pane_width: f32,
        reset_scroll: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(handle) = self.handle.clone() else { return; };
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                // Bound queued data and stop the worker when this receiver is dropped.
                let (sender, mut receiver) = tokio::sync::mpsc::channel(2);
                tokio::task::spawn_blocking(move || {
                    let mut first = true;
                    let result = oxyde_lib::platform::platform().stream_trash_entries(&mut |batch| {
                        let replace = first;
                        first = false;
                        sender.blocking_send(Update::Batch(batch, replace)).is_ok()
                    });
                    let result = if let Err(error) = result {
                        log::warn!("Progressive trash listing failed, retrying complete listing: {error}");
                        oxyde_lib::commands::ops::list_trash()
                            .map_err(|error| error.to_string())
                            .map(|entries| { let _ = sender.blocking_send(Update::Batch(entries, true)); })
                    } else { result };
                    let _ = sender.blocking_send(Update::Finished(result));
                });
                let started = std::time::Instant::now();
                let mut first = true;
                while let Some(mut update) = receiver.recv().await {
                    if let Update::Batch(entries, replace) = &mut update {
                        let files: Vec<_> = std::mem::take(entries).into_iter().map(file_entry).collect();
                        let update = if *replace { crate::ui::workspace::FileUpdate::Replace(files.into()) } else { crate::ui::workspace::FileUpdate::Append(files.into()) };
                        let owner = super::super::file_preparation::Owner::Listing { id: tab_id.clone(), path: "trash://".into(), request: request_id };
                        if !super::super::file_preparation::prepare(&handle, &mut cx, &owner, update).await { break; }
                    }
                    let keep_loading = handle.update(&mut cx, |this, cx| {
                        if this.pending_file_requests.get(&tab_id) != Some(&request_id) {
                            return false;
                        }
                        let Some(tab) = this.state.find_tab_mut(&tab_id) else { return false; };
                        if tab.path != "trash://" { return false; }
                        match update {
                            Update::Batch(_, _) => {

                                if first {
                                    log::info!(target: "oxyde_lib::trash_listing",
                                        "stage=first_batch items={} elapsed_s={:.6}",
                                        tab.files.len(), started.elapsed().as_secs_f64());
                                    if reset_scroll { tab.list_scroll = UniformListScrollHandle::new(); }
                                    first = false;
                                }
                            }
                            Update::Finished(result) => {
                                match result {
                                    Ok(()) => {
                                        tab.load_status = crate::ui::workspace::TabLoadStatus::Ready;
                                        tab.reveal_pending_item(pane_width);
                                    }
                                    Err(error) => {
                                        tab.clear_files();
                                        tab.load_status = crate::ui::errors::path_load_status(
                                            &oxyde_lib::models::CommandError::TrashError(error.clone()));
                                        log::warn!("Cannot list trash: {error}");
                                    }
                                }
                                this.pending_file_requests.remove(&tab_id);
                                cx.notify();
                                return false;
                            }
                        }
                        cx.notify();
                        true
                    });
                    if !keep_loading { break; }
                }
            }
        }).detach();
    }
}

fn file_entry(entry: TrashEntry) -> FileEntry {
    FileEntry {
        native_kind: None,
        created: None,
        content_stamp: None,
        archive_path: None,
        name: entry.name, path: entry.path, is_dir: entry.is_dir,
        size: entry.size, modified: entry.modified,
        original_path: Some(entry.original_path), deleted_time: Some(entry.deleted_time as i64),
        is_hidden: false, is_system: false, is_symlink: false, is_junction: false,
        is_calculated: true, folders_count: None, files_count: None, network_details: None,
    }
}
