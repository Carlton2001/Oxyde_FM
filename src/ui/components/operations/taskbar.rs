//! The running operations' progress on the main windows' taskbar entries, where the
//! system shows one; it follows the operations even when the window is minimized.
use crate::ui::app::OxydeApp;
use gpui::*;
use oxyde_lib::platform::TaskbarProgress;
use oxyde_lib::systems::file_ops::{FileOperationManager, OpStatus, OperationProgress};
use std::cell::Cell;

/// The share shown for the active operations, an average of theirs.
fn taskbar_progress(operations: &[OperationProgress]) -> TaskbarProgress {
    if operations.is_empty() {
        return TaskbarProgress::None;
    }
    let share = operations.iter().map(super::compact::completed_share).sum::<f64>() / operations.len() as f64;
    // Thousandths: the taskbar shows no finer, and an unchanged value is not sent again.
    let share = ((share.clamp(0.0, 1.0) * 1000.0).round() / 1000.0) as f32;
    let measured = |op: &OperationProgress| op.total_bytes > 0 || op.total_files > 0;
    if operations.iter().all(|op| matches!(op.status, OpStatus::Paused)) {
        TaskbarProgress::Paused(share)
    } else if operations.iter().any(|op| matches!(op.status, OpStatus::Running) && measured(op)) {
        TaskbarProgress::Running(share)
    } else {
        TaskbarProgress::Indeterminate
    }
}

/// Sends the current progress to every main window, when it or the windows changed.
pub(crate) fn sync_taskbar_progress(operations: &FileOperationManager, cx: &mut App) {
    thread_local! {
        static SENT: Cell<Option<(TaskbarProgress, usize)>> = const { Cell::new(None) };
    }
    let progress = taskbar_progress(&operations.list_active_progress());
    let windows: Vec<AnyWindowHandle> = cx.windows().into_iter()
        .filter(|handle| handle.downcast::<OxydeApp>().is_some())
        .collect();
    if SENT.get() == Some((progress, windows.len())) {
        return;
    }
    SENT.set(Some((progress, windows.len())));
    for handle in windows {
        // The window alone is borrowed: this may run while the app itself is updated.
        let _ = cx.update_window(handle, |_, window, _| {
            if let Ok(native) = raw_window_handle::HasWindowHandle::window_handle(window) {
                oxyde_lib::platform::platform().set_taskbar_progress(native.as_raw(), progress);
            }
        });
    }
}

#[cfg(test)]
#[path = "taskbar/tests/progress.rs"]
mod tests;
