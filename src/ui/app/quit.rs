//! Closing Oxyde while operations run or are paused: ending the process would cut
//! them short, so the user confirms, and the operations are then cancelled and
//! allowed to clean their partial output before the window closes.
use super::*;

/// How long cancelled operations get to clean up before Oxyde closes anyway.
const CANCEL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
const CANCEL_POLL: std::time::Duration = std::time::Duration::from_millis(100);
/// How long cancelling may take before the prompt says it waits: a quicker end
/// would only flash the message, too briefly to read.
const WAIT_NOTICE: std::time::Duration = std::time::Duration::from_millis(800);

pub(crate) struct QuitConfirmation {
    /// Operations not finished when it opened, as its prompt counts them.
    pub count: usize,
    /// Set once confirmed: the operations are being cancelled.
    pub cancelling: bool,
    /// Set once cancelling lasts past `WAIT_NOTICE`: the prompt says it waits.
    pub waiting: bool,
    pub focus: FocusHandle,
    pub confirm_focus: FocusHandle,
}

impl OxydeApp {
    /// Whether the main window may close now; otherwise asks first and returns false.
    pub(crate) fn may_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let active = self.file_ops.operation_counts().1;
        if active == 0 {
            return true;
        }
        if self.quit_confirmation.is_none() {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            self.quit_confirmation = Some(QuitConfirmation {
                count: active,
                cancelling: false,
                waiting: false,
                focus,
                confirm_focus: cx.focus_handle(),
            });
            cx.notify();
        }
        false
    }

    /// Keeps Oxyde open, or cancels the operations and closes once they have ended.
    pub(crate) fn resolve_quit_confirmation(&mut self, quit: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(confirmation) = self.quit_confirmation.as_mut() else { return };
        if confirmation.cancelling {
            return;
        }
        if !quit {
            self.quit_confirmation = None;
            window.focus(&self.focus_handle, cx);
            cx.notify();
            return;
        }
        confirmation.cancelling = true;
        cx.notify();
        let manager = self.file_ops.clone();
        for operation in manager.list_active_progress() {
            manager.cancel_operation(&operation.id);
        }
        cx.spawn(async move |this, cx| {
            let started = std::time::Instant::now();
            let mut noticed = false;
            while manager.operation_counts().1 > 0 && started.elapsed() < CANCEL_TIMEOUT {
                cx.background_executor().timer(CANCEL_POLL).await;
                if !noticed && started.elapsed() >= WAIT_NOTICE {
                    noticed = true;
                    let _ = this.update(cx, |this, cx| {
                        if let Some(confirmation) = this.quit_confirmation.as_mut() {
                            confirmation.waiting = true;
                            cx.notify();
                        }
                    });
                }
            }
            if manager.operation_counts().1 > 0 {
                log::warn!("Closing with operations still ending after {CANCEL_TIMEOUT:?}");
            }
            let _ = this.update(cx, |this, cx| {
                this.quit_confirmation = None;
                let (Some(window), Some(handle)) = (this.main_window, this.handle.clone()) else { return };
                cx.defer(move |cx| {
                    let _ = window.update(cx, |_, window, cx| {
                        crate::ui::components::title_bar::close_main_window_now(&handle, "quit after cancelling operations", window, cx);
                    });
                });
            });
        })
        .detach();
    }
}
