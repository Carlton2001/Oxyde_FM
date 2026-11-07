//! What later launches and FileManager1 callers ask the running Oxyde to show: new tabs
//! in the main window, items selected in their folder, or their Properties.
use super::OxydeApp;
use futures::StreamExt;
use gpui::Context;
use oxyde_lib::platform::{InstanceMessage, InstanceRequest};

/// Items grouped by folder, folders in the order their first item came.
pub(super) fn by_folder(items: Vec<String>) -> Vec<(String, Vec<String>)> {
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for item in items {
        let Some(folder) = std::path::Path::new(&item).parent().map(|folder| folder.to_string_lossy().into_owned()) else {
            continue;
        };
        match groups.iter_mut().find(|(known, _)| *known == folder) {
            Some((_, members)) => members.push(item),
            None => groups.push((folder, vec![item])),
        }
    }
    groups
}

impl OxydeApp {
    /// The session saved at the last exit, with each received location in a new tab
    /// after it, the last one active.
    pub(crate) fn add_startup_locations(&mut self, locations: &[String]) {
        let pane = self.state.active_pane_id.clone();
        for location in locations {
            self.state.add_tab(&pane, location.clone(), true);
        }
    }

    pub(crate) fn receive_instance_requests(
        &mut self,
        mut requests: futures::channel::mpsc::UnboundedReceiver<InstanceMessage>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            while let Some(message) = requests.next().await {
                let token = message.activation_token.clone();
                let Ok((window, properties)) = this.update(cx, |this, cx| this.show_instance_request(message.request, cx)) else {
                    break;
                };
                let Some(window) = window else { continue };
                // Properties windows open from the main window, outside the update above.
                let this = this.clone();
                let _ = window.update(cx, move |_, window, cx| {
                    window.activate_window_with_token(token);
                    if !properties.is_empty() {
                        let _ = this.update(cx, |this, cx| this.open_properties_for_paths(properties, window, cx));
                    }
                });
            }
        }).detach();
    }

    /// Opens the tabs a request names, and returns the main window to bring forward with
    /// the items whose Properties open there.
    fn show_instance_request(&mut self, request: InstanceRequest, cx: &mut Context<Self>) -> (Option<gpui::AnyWindowHandle>, Vec<String>) {
        log::info!("received from another process: {request:?}");
        let pane = self.state.active_pane_id.clone();
        let properties = match request {
            InstanceRequest::Folders(folders) => {
                for folder in folders {
                    self.add_tab(&pane, folder, true, cx);
                }
                Vec::new()
            }
            InstanceRequest::Items(items) => {
                for (folder, members) in by_folder(items) {
                    self.add_tab(&pane, folder, true, cx);
                    self.select_received_items(members, cx);
                }
                Vec::new()
            }
            InstanceRequest::Properties(items) => items,
        };
        (self.main_window, properties)
    }

    /// Selects `items` in the active tab, which just opened on their folder, and scrolls
    /// to the first once the listing is there.
    fn select_received_items(&mut self, items: Vec<String>, cx: &mut Context<Self>) {
        let pane_id = self.state.active_pane_id.clone();
        let width = self.pane_sizes.get(&pane_id).map_or(self.list_width, |size| size.width);
        let Some(pane) = self.state.find_pane_mut(&pane_id) else { return };
        let tab = pane.active_tab_mut();
        tab.selected_paths.clear();
        tab.selected_paths.extend(items.iter().cloned());
        tab.selection_anchor = items.first().cloned();
        tab.keyboard_cursor = items.first().cloned();
        tab.pending_reveal = items.first().cloned();
        if matches!(tab.load_status, crate::ui::workspace::TabLoadStatus::Ready) {
            tab.reveal_pending_item(width);
        }
        cx.notify();
    }
}

#[cfg(test)]
#[path = "instance/tests/groups.rs"]
mod tests;
