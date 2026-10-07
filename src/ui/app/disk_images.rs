use super::OxydeApp;
use crate::ui::components::paths_match;
use crate::ui::i18n::Message;
use gpui::*;

impl OxydeApp {
    pub(crate) fn mounted_image_for_drive(&self, path: &str) -> Option<String> {
        self.mounted_disk_images
            .iter()
            .find(|image| image.roots.iter().any(|root| paths_match(root, path)))
            .map(|image| image.image_path.clone())
    }

    pub(super) fn refresh_disk_images(&mut self, cx: &mut Context<Self>) {
        let epoch = self.drive_refresh_epoch;
        self.disk_image_refresh_task = Some(cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(|| {
                oxyde_lib::platform::platform().mounted_disk_images()
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                if this.ejecting_drive || this.drive_refresh_epoch != epoch {
                    return;
                }
                if let Ok(Ok(images)) = result {
                    this.mounted_disk_images = images;
                    cx.notify();
                }
            });
        }));
    }

    pub(crate) fn open_directory_item(
        &mut self,
        pane_id: &str,
        path: String,
        new_tab: bool,
        cx: &mut Context<Self>,
    ) {
        let image = std::path::Path::new(&path);
        if oxyde_lib::utils::disk_image::needs_mount_for_browsing(image) {
            self.open_disk_image(pane_id.to_string(), path, new_tab, cx);
        } else if oxyde_lib::utils::disk_image::is_disk_image(image) {
            // Prefer virtual ISO9660 browsing. Windows can mount UDF-only images
            // that the retained Tauri reader cannot decode.
            let pane = pane_id.to_string();
            let origin = self
                .state
                .find_pane(pane_id)
                .map(|pane| (pane.active_tab().id.clone(), pane.active_tab().path.clone()));
            cx.spawn(async move |this, cx| {
                let image_path = path.clone();
                let readable = tokio::task::spawn_blocking(move || {
                    oxyde_lib::commands::archive::list_archive_contents(image_path, String::new())
                        .is_ok()
                })
                .await
                .unwrap_or(false);
                let _ = this.update(cx, |this, cx| {
                    if !new_tab
                        && this.state.find_pane(&pane).map(|pane| {
                            (pane.active_tab().id.clone(), pane.active_tab().path.clone())
                        }) != origin
                    {
                        return;
                    }
                    if readable || !cfg!(target_os = "windows") {
                        if new_tab {
                            this.add_tab(&pane, path, false, cx);
                        } else {
                            this.navigate_to(&pane, path, cx);
                        }
                    } else {
                        this.open_disk_image(pane, path, new_tab, cx);
                    }
                });
            })
            .detach();
        } else if new_tab {
            self.add_tab(pane_id, path, false, cx);
        } else {
            self.navigate_to(pane_id, path, cx);
        }
    }

    pub(crate) fn open_disk_image(
        &mut self,
        pane: String,
        path: String,
        new_tab: bool,
        cx: &mut Context<Self>,
    ) {
        self.change_disk_image(path, false, Some((pane, new_tab)), cx);
    }

    pub(crate) fn change_disk_image(
        &mut self,
        path: String,
        unmount: bool,
        open: Option<(String, bool)>,
        cx: &mut Context<Self>,
    ) {
        self.context_menus.clear();
        if self.ejecting_drive {
            cx.notify();
            return;
        }
        if unmount {
            let roots = self
                .mounted_disk_images
                .iter()
                .find(|image| paths_match(&image.image_path, &path))
                .map(|image| image.roots.as_slice())
                .unwrap_or_default();
            if image_operations_in_progress(&self.file_ops.list_operations(), roots) {
                self.toast(super::ToastKind::Error, Message::DiskImageBusy, String::new(), 0, cx);
                cx.notify();
                return;
            }
        }
        self.ejecting_drive = true;
        self.drive_refresh_epoch = self.drive_refresh_epoch.wrapping_add(1);
        self.drive_details_task = None;
        self.disk_image_refresh_task = None;
        self.sidebar_follow_task = None;
        self.pending_sidebar_requests.clear();
        self.pending_watchers.clear();
        let watchers = if unmount {
            std::mem::take(&mut self._path_watchers)
        } else {
            Default::default()
        };
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let platform = oxyde_lib::platform::platform();
                for (watched_path, watcher) in watchers {
                    if let Err(error) = watcher.release(&watched_path) {
                        return (
                            Err(error),
                            platform.list_drives(false),
                            platform.mounted_disk_images(),
                        );
                    }
                }
                let outcome = if unmount {
                    platform.unmount_disk_image(&path).map(|_| Vec::new())
                } else {
                    platform.mount_disk_image(&path)
                };
                (
                    outcome,
                    platform.list_drives(false),
                    platform.mounted_disk_images(),
                )
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                this.ejecting_drive = false;
                let outcome = match result {
                    Ok((outcome, drives, images)) => {
                        if let Ok(images) = images {
                            this.mounted_disk_images = images;
                        }
                        this.apply_drive_snapshot(drives, cx);
                        outcome
                    }
                    Err(error) => Err(error.to_string()),
                };
                match outcome {
                    Ok(roots) => {
                        this.toast(super::ToastKind::Success, if unmount { Message::ToastUnmounted } else { Message::ToastMounted }, String::new(), 0, cx);
                        if let Some((pane, new_tab)) = open {
                            if let Some(root) = roots.into_iter().next() {
                                this.open_directory_item(&pane, root, new_tab, cx);
                            } else {
                                this.toast(super::ToastKind::Error, Message::DiskImageNoVolume, String::new(), 0, cx);
                            }
                        }
                    }
                    Err(error) => {
                        this.toast(super::ToastKind::Error,
                            if unmount {
                                Message::DiskImageUnmountFailed
                            } else if open.is_some() {
                                Message::DiskImageOpenFailed
                            } else {
                                Message::DiskImageMountFailed
                            },
                            error, 0, cx);
                        this.refresh_disk_images(cx);
                    }
                }
                this.state.refresh_sidebar();
                this.reconcile_watchers(cx);
                cx.notify();
            });
        })
        .detach();
    }
}

fn image_operations_in_progress(
    operations: &[oxyde_lib::systems::file_ops::FileOperation],
    roots: &[String],
) -> bool {
    use oxyde_lib::systems::file_ops::OpStatus;
    operations.iter().any(|operation| {
        !matches!(
            operation.status,
            OpStatus::Completed | OpStatus::Cancelled | OpStatus::Error(_)
        ) && operation
            .sources
            .iter()
            .chain(operation.destination.iter())
            .any(|path| {
                roots
                    .iter()
                    .any(|root| super::drive_watchers::path_on_drive(&path.to_string_lossy(), root))
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxyde_lib::systems::file_ops::{FileOpType, FileOperation, OpStatus};
    #[::core::prelude::v1::test]
    fn unmount_waits_for_source_and_destination_operations_only_on_its_volume() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("mounted");
        let roots = vec![root.to_string_lossy().into_owned()];
        let mut operation = FileOperation::new(
            FileOpType::Copy,
            vec![root.join("file")],
            Some(temp.path().join("output")),
            None,
        );
        assert!(image_operations_in_progress(&[operation.clone()], &roots));
        operation.status = OpStatus::Completed;
        assert!(!image_operations_in_progress(&[operation.clone()], &roots));
        operation.status = OpStatus::Paused;
        operation.sources = vec![temp.path().join("other")];
        assert!(!image_operations_in_progress(&[operation.clone()], &roots));
        operation.destination = Some(root.join("target"));
        assert!(image_operations_in_progress(&[operation], &roots));
    }
}
