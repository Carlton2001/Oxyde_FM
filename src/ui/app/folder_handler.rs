//! Oxyde as the application other programs open folders with: a choice of the system,
//! read each time the settings open, never stored as a preference.
use super::OxydeApp;
use crate::ui::i18n::Message;
use gpui::Context;

impl OxydeApp {
    /// Reads the system's choice off the UI thread; the row stays hidden until known.
    pub(crate) fn refresh_folder_handler(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let state = tokio::task::spawn_blocking(|| oxyde_lib::platform::platform().opens_folders_by_default())
                .await.ok().flatten();
            let _ = this.update(cx, |this, cx| {
                if !this.changing_folder_handler && this.opens_folders != state {
                    this.opens_folders = state;
                    cx.notify();
                }
            });
        }).detach();
    }

    /// Makes Oxyde the default for folders, remembering the application it replaces, or
    /// gives folders back to that application.
    pub(crate) fn toggle_folder_handler(&mut self, cx: &mut Context<Self>) {
        let Some(current) = self.opens_folders else { return };
        if self.changing_folder_handler {
            return;
        }
        let enabled = !current;
        self.opens_folders = Some(enabled);
        self.changing_folder_handler = true;
        cx.notify();
        let previous = self.config.folder_handler_before.clone();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let platform = oxyde_lib::platform::platform();
                (platform.open_folders_by_default(enabled, previous.as_deref()), platform.opens_folders_by_default())
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.changing_folder_handler = false;
                match result {
                    Ok((Ok(replaced), state)) => {
                        this.opens_folders = state;
                        this.config.folder_handler_before = if enabled { replaced } else { None };
                        this.save_preferences(cx);
                        // Oxyde is in the menus of folders only while it is not their default.
                        this.refresh_system_menus(cx);
                    }
                    Ok((Err(error), state)) => {
                        log::warn!("Cannot change the default application for folders: {error}");
                        this.opens_folders = state;
                        this.settings_error = Some(Message::SettingsDefaultFileManagerError);
                    }
                    Err(error) => {
                        log::warn!("Changing the default application for folders stopped: {error}");
                        this.opens_folders = Some(current);
                        this.settings_error = Some(Message::SettingsDefaultFileManagerError);
                    }
                }
                cx.notify();
            });
        }).detach();
    }
}
