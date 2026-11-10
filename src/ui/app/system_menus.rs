//! What Oxyde lists outside its window: the pinned folders under its taskbar button,
//! and itself in the menus of the system's folders. The platform decides where each
//! applies; this only says what, in the user's language, each time it changes.
use super::OxydeApp;
use crate::ui::i18n::{I18nContext, Message};
use gpui::Context;
use oxyde_lib::platform::TaskbarMenu;

impl OxydeApp {
    /// The pinned folders as the sidebar names them; places that are not folders have
    /// no path to start Oxyde on and are left to the platform to drop.
    fn taskbar_menu(&self, cx: &Context<Self>) -> TaskbarMenu {
        let i18n = cx.i18n();
        TaskbarMenu {
            category: i18n.text(Message::SidebarQuickAccess).to_string(),
            pinned: self.state.pinned_items.iter().filter(|item| !item.name.is_empty()).map(|item| (item.name.clone(), item.path.clone())).collect(),
            administrator: i18n.text(Message::TaskbarRunAsAdministrator).to_string(),
        }
    }

    /// Tells the system, off the UI thread. Called when the pinned folders or the
    /// default for folders change, and at start.
    pub(crate) fn refresh_system_menus(&mut self, cx: &mut Context<Self>) {
        let menu = self.taskbar_menu(cx);
        let label = cx.i18n().text(Message::ShellOpenInOxyde).to_string();
        cx.spawn(async move |_, _| {
            let _ = tokio::task::spawn_blocking(move || {
                let platform = oxyde_lib::platform::platform();
                platform.set_taskbar_menu(&menu);
                platform.refresh_folder_menus(&label);
            }).await;
        }).detach();
    }
}
