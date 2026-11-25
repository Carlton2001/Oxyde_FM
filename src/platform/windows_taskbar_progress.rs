//! Progress of the file operations on the window's taskbar button.
use super::TaskbarProgress;
use raw_window_handle::RawWindowHandle;
use std::cell::RefCell;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Shell::{
    ITaskbarList3, TaskbarList, TBPF_INDETERMINATE, TBPF_NOPROGRESS, TBPF_NORMAL, TBPF_PAUSED,
};

/// Steps of the value given to the taskbar.
const STEPS: u64 = 1000;

thread_local! {
    /// Created once on the UI thread, whose apartment GPUI initialized; `None` when the
    /// Shell offers no taskbar (Windows Server Core, some kiosks).
    static TASKBAR: RefCell<Option<Option<ITaskbarList3>>> = const { RefCell::new(None) };
}

pub(super) fn set(window: RawWindowHandle, progress: TaskbarProgress) {
    let RawWindowHandle::Win32(handle) = window else { return };
    let hwnd = HWND(handle.hwnd.get() as *mut std::ffi::c_void);
    TASKBAR.with_borrow_mut(|taskbar| {
        let taskbar = taskbar.get_or_insert_with(|| unsafe {
            let list: ITaskbarList3 = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)
                .inspect_err(|error| log::warn!("Cannot reach the taskbar: {error}"))
                .ok()?;
            list.HrInit().ok()?;
            Some(list)
        });
        let Some(taskbar) = taskbar else { return };
        let value = |ratio: f32| (f64::from(ratio.clamp(0.0, 1.0)) * STEPS as f64).round() as u64;
        let result = unsafe {
            match progress {
                TaskbarProgress::None => taskbar.SetProgressState(hwnd, TBPF_NOPROGRESS),
                TaskbarProgress::Indeterminate => taskbar.SetProgressState(hwnd, TBPF_INDETERMINATE),
                TaskbarProgress::Running(ratio) => taskbar.SetProgressState(hwnd, TBPF_NORMAL)
                    .and_then(|_| taskbar.SetProgressValue(hwnd, value(ratio), STEPS)),
                TaskbarProgress::Paused(ratio) => taskbar.SetProgressState(hwnd, TBPF_PAUSED)
                    .and_then(|_| taskbar.SetProgressValue(hwnd, value(ratio), STEPS)),
            }
        };
        if let Err(error) = result {
            log::debug!("Cannot show the progress on the taskbar: {error}");
        }
    });
}
