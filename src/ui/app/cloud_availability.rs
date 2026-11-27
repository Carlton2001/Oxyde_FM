use super::context_menus::{NativeMenuLoad, NATIVE_MENU_TIMEOUT};
use super::{ContextMenuContent, OxydeApp};
use crate::ui::components::dropdown::{DropdownItem, IconName};
use crate::ui::i18n::{I18n, I18nContext, Message};
use gpui::*;
use oxyde_lib::models::{CloudAvailability, CloudStatus, FileEntry};
use std::rc::Rc;

mod progress;
pub(super) use progress::CloudBadges;

impl OxydeApp {
    pub(crate) fn cloud_availability_action(&self, paths: &[String]) -> Option<CloudAvailability> {
        selection_action(&self.cloud_action_files(paths))
    }

    fn cloud_action_files(&self, paths: &[String]) -> Vec<FileEntry> {
        let mut files = self.files_for_action(paths, false);
        for file in &mut files {
            file.cloud_status = self.cloud_badges.status(file);
        }
        files
    }

    pub(crate) fn cloud_availability_menu_item(
        i18n: &I18n,
        handle: Entity<Self>,
        paths: Vec<String>,
        availability: CloudAvailability,
        enabled: bool,
    ) -> DropdownItem {
        let (message, icon) = match availability {
            CloudAvailability::Offline => (Message::CloudKeepOffline, IconName::CloudDownload),
            CloudAvailability::OnlineOnly => (Message::CloudMakeOnlineOnly, IconName::CloudUpload),
        };
        let mut item = DropdownItem::new(i18n.text(message).to_string(), move |_, app| {
            if !enabled {
                return;
            }
            handle.update(app, |this, cx| {
                this.change_cloud_availability(paths.clone(), availability, cx);
            });
        })
        .icon(icon);
        item.is_dimmed = !enabled;
        item
    }

    pub(crate) fn resolve_cloud_availability_menu(
        &mut self,
        clicked: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.context_menus.first().filter(|menu| {
            menu.content
                .items()
                .is_some_and(|items| items.iter().any(is_cloud_availability_item))
        }) else {
            return;
        };
        let menu_scroll = menu.scroll_handle.clone();
        let paths = self.context_action_paths(clicked);
        let files = self.cloud_action_files(&paths);
        let Some(preferred) = selection_action(&files) else {
            return;
        };
        let Some(load) = NativeMenuLoad::start() else {
            return;
        };
        let also_try_offline = files
            .iter()
            .all(|file| file.cloud_status == Some(CloudStatus::LocallyAvailable));
        let timer = cx.background_executor().timer(NATIVE_MENU_TIMEOUT);
        cx.spawn(async move |this, cx| {
            let targets = paths.clone();
            let task = tokio::task::spawn_blocking(move || {
                let _load = load;
                let platform = oxyde_lib::platform::platform();
                common_available_action(&targets, preferred, also_try_offline, |path| {
                    platform
                        .cloud_availability_commands(path)
                        .map_err(|error| {
                            log::debug!(
                                "Cannot read cloud availability commands for {path}: {error}"
                            );
                        })
                        .ok()
                })
            });
            let futures::future::Either::Left((Ok(Some(action)), _)) =
                futures::future::select(task, timer).await
            else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                let Some(menu) = this
                    .context_menus
                    .first_mut()
                    .filter(|menu| Rc::ptr_eq(&menu.scroll_handle.0, &menu_scroll.0))
                else {
                    return;
                };
                let ContextMenuContent::Items(items) = &mut menu.content else {
                    return;
                };
                let Some(item) = items
                    .iter_mut()
                    .find(|item| is_cloud_availability_item(item))
                else {
                    return;
                };
                *item =
                    Self::cloud_availability_menu_item(cx.i18n(), cx.entity(), paths, action, true);
                cx.notify();
            });
        })
        .detach();
    }
}

fn is_cloud_availability_item(item: &DropdownItem) -> bool {
    matches!(
        item.icon,
        Some(IconName::CloudUpload | IconName::CloudDownload)
    )
}

fn common_available_action(
    paths: &[String],
    preferred: CloudAvailability,
    also_try_offline: bool,
    mut available: impl FnMut(&str) -> Option<Vec<CloudAvailability>>,
) -> Option<CloudAvailability> {
    // A OneDrive folder may keep its own directory metadata locally while the
    // provider considers its contents online-only and disables Free up space.
    let mut choices = vec![preferred];
    if also_try_offline {
        choices.push(CloudAvailability::Offline);
    }
    for path in paths {
        let enabled = available(path)?;
        choices.retain(|choice| enabled.contains(choice));
        if choices.is_empty() {
            break;
        }
    }
    choices.first().copied()
}

fn selection_action(files: &[FileEntry]) -> Option<CloudAvailability> {
    let action_for = |file: &FileEntry| match file.cloud_status {
        Some(CloudStatus::OnlineOnly) => Some(CloudAvailability::Offline),
        Some(CloudStatus::LocallyAvailable | CloudStatus::AlwaysAvailable) => {
            Some(CloudAvailability::OnlineOnly)
        }
        _ => None,
    };
    let action = action_for(files.first()?)?;
    files
        .iter()
        .all(|file| action_for(file) == Some(action))
        .then_some(action)
}

#[cfg(test)]
#[path = "cloud_availability/tests/selection.rs"]
mod tests;
