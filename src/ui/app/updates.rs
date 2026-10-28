//! Update checks and installation from GitHub releases.
use super::*;
use oxyde_lib::systems::updates::{self, Update};

/// Delay before the automatic check, so startup work keeps the network and disk.
const AUTOMATIC_CHECK_DELAY: std::time::Duration = std::time::Duration::from_secs(30);
const RELEASES_URL: &str = "https://github.com/Carlton2001/Oxyde_FM/releases";

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum UpdateStatus {
    Idle,
    Checking,
    Current,
    Available(Update),
    Downloading,
    Failed,
}

impl OxydeApp {
    pub(super) fn set_main_window(&mut self, window: AnyWindowHandle) {
        self.main_window = Some(window);
    }

    /// Check once per session, shortly after startup.
    pub(crate) fn schedule_update_check(&mut self, cx: &mut Context<Self>) {
        if !oxyde_lib::platform::platform().updates_itself() {
            return;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(AUTOMATIC_CHECK_DELAY).await;
            let _ = this.update(cx, |this, cx| this.check_for_updates(false, cx));
        })
        .detach();
    }

    pub(crate) fn check_for_updates(&mut self, manual: bool, cx: &mut Context<Self>) {
        if matches!(self.update_status, UpdateStatus::Checking | UpdateStatus::Downloading) {
            return;
        }
        // A silent automatic check never replaces a result the user already sees.
        if !manual && self.update_status != UpdateStatus::Idle {
            return;
        }
        self.update_status = UpdateStatus::Checking;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(updates::check).await
                .unwrap_or_else(|error| Err(error.to_string()));
            let _ = this.update(cx, |this, cx| this.finish_update_check(result, manual, cx));
        })
        .detach();
    }

    fn finish_update_check(&mut self, result: Result<Option<Update>, String>, manual: bool, cx: &mut Context<Self>) {
        self.update_status = match result {
            Ok(Some(update)) => {
                // Announce each version once; portable copies are updated by hand.
                if !manual && !oxyde_lib::app_paths::is_portable()
                    && self.config.last_notified_update.as_deref() != Some(update.version.as_str())
                {
                    self.config.last_notified_update = Some(update.version.clone());
                    self.save_preferences(cx);
                    self.toast(ToastKind::Success, Message::UpdateAvailableToast, update.version.clone(), 0, cx);
                }
                UpdateStatus::Available(update)
            }
            Ok(None) => if manual { UpdateStatus::Current } else { UpdateStatus::Idle },
            Err(error) => {
                log::warn!("Update check failed: {error}");
                if manual { UpdateStatus::Failed } else { UpdateStatus::Idle }
            }
        };
        cx.notify();
    }

    /// Install the available update, or open the releases page for a portable copy.
    pub(crate) fn install_update(&mut self, cx: &mut Context<Self>) {
        let UpdateStatus::Available(update) = self.update_status.clone() else { return; };
        if oxyde_lib::app_paths::is_portable() {
            self.open_external_item(RELEASES_URL.into(), cx);
            return;
        }
        self.update_status = UpdateStatus::Downloading;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let setup = updates::download(&update)?;
                updates::launch(&setup)
            })
            .await
            .unwrap_or_else(|error| Err(error.to_string()));
            let _ = this.update(cx, |this, cx| match result {
                Ok(()) => this.close_for_update(cx),
                Err(error) => {
                    log::warn!("Update installation failed: {error}");
                    this.update_status = UpdateStatus::Failed;
                    this.toast(ToastKind::Error, Message::UpdateFailedToast, error, 0, cx);
                }
            });
        })
        .detach();
    }

    /// Setup waits for nothing: close normally so preferences and session are saved.
    fn close_for_update(&mut self, cx: &mut Context<Self>) {
        let (Some(window), Some(handle)) = (self.main_window, self.handle.clone()) else { return; };
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                crate::ui::components::title_bar::close_main_window(&handle, "update installation", window, cx);
            });
        });
    }
}
