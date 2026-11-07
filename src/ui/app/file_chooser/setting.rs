//! Oxyde as the file chooser of other applications: a choice of the system's portal,
//! read each time the settings open, never stored as a preference.
use super::super::OxydeApp;
use crate::ui::i18n::Message;
use gpui::Context;

impl OxydeApp {
    /// Reads the portal's configuration off the UI thread; the row stays hidden until known.
    pub(crate) fn refresh_file_chooser_setting(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let state = tokio::task::spawn_blocking(|| oxyde_lib::platform::platform().chooses_files())
                .await.ok().flatten();
            let _ = this.update(cx, |this, cx| {
                if !this.changing_file_chooser && this.chooses_files != state {
                    this.chooses_files = state;
                    cx.notify();
                }
            });
        }).detach();
    }

    /// Makes Oxyde the portal's file chooser, or gives that back; the portal restarts.
    pub(crate) fn toggle_file_chooser_setting(&mut self, cx: &mut Context<Self>) {
        let Some(current) = self.chooses_files else { return };
        if self.changing_file_chooser {
            return;
        }
        let enabled = !current;
        self.chooses_files = Some(enabled);
        self.changing_file_chooser = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let platform = oxyde_lib::platform::platform();
                (platform.choose_files(enabled), platform.chooses_files())
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.changing_file_chooser = false;
                match result {
                    Ok((Ok(()), state)) => this.chooses_files = state,
                    Ok((Err(error), state)) => {
                        log::warn!("Cannot change the file chooser: {error}");
                        this.chooses_files = state;
                        this.settings_error = Some(Message::SettingsFileChooserError);
                    }
                    Err(error) => {
                        log::warn!("Changing the file chooser stopped: {error}");
                        this.chooses_files = Some(current);
                        this.settings_error = Some(Message::SettingsFileChooserError);
                    }
                }
                cx.notify();
            });
        }).detach();
    }
}
