//! Processes that refused a device ejection. PnP only returns a veto type; the kernel
//! names the processes in event 225 of the System log, readable by interactive users.
use std::collections::BTreeSet;
use std::time::{Duration, Instant};
use windows::core::{w, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::{CM_Get_Device_IDW, CR_SUCCESS};
use windows::Win32::System::EventLog::*;

/// The kernel logs shortly after the veto returns.
const ATTEMPTS: usize = 10;
const ATTEMPT_DELAY: Duration = Duration::from_millis(100);
/// Tolerance on the log timestamps, around the time spent since the request.
const LOG_MARGIN: Duration = Duration::from_secs(1);
const KERNEL_PROCESS: &str = "System";

/// Names of the processes that vetoed the ejection of `node` requested at `started`.
pub(super) fn holders(node: u32, started: Instant) -> Vec<String> {
    let Some(device) = device_id(node) else { return Vec::new() };
    for attempt in 0..ATTEMPTS {
        if attempt > 0 {
            std::thread::sleep(ATTEMPT_DELAY);
        }
        let processes = vetoes(&device, started.elapsed() + LOG_MARGIN).unwrap_or_else(|error| {
            log::debug!("Ejection veto log: {error}");
            Vec::new()
        });
        if !processes.is_empty() {
            return names(processes);
        }
    }
    Vec::new()
}

fn device_id(node: u32) -> Option<String> {
    let mut buffer = [0u16; 512];
    if unsafe { CM_Get_Device_IDW(node, &mut buffer, 0) } != CR_SUCCESS {
        return None;
    }
    let end = buffer.iter().position(|unit| *unit == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..end]))
}

/// Closes an event log handle on every exit path.
struct Handle(EVT_HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        let _ = unsafe { EvtClose(self.0) };
    }
}

/// `ProcessName` of the recent veto events for `device`, in log order.
fn vetoes(device: &str, age: Duration) -> windows::core::Result<Vec<String>> {
    let query: Vec<u16> = format!(
        "*[System[Provider[@Name='Microsoft-Windows-Kernel-PnP'] and EventID=225 \
         and TimeCreated[timediff(@SystemTime) <= {}]]]",
        age.as_millis()
    )
    .encode_utf16()
    .chain(std::iter::once(0))
    .collect();
    let paths: [PCWSTR; 2] = [
        w!("Event/EventData/Data[@Name='ProcessName']"),
        w!("Event/EventData/Data[@Name='DeviceInstance']"),
    ];
    let mut processes = Vec::new();
    unsafe {
        let results = Handle(EvtQuery(
            None,
            w!("System"),
            PCWSTR(query.as_ptr()),
            EvtQueryChannelPath.0 | EvtQueryReverseDirection.0,
        )?);
        let context = Handle(EvtCreateRenderContext(Some(&paths), EvtRenderContextValues.0)?);
        let mut events = [0isize; 16];
        loop {
            let mut returned = 0;
            // Stops on ERROR_NO_MORE_ITEMS, which is an error for this call.
            if EvtNext(results.0, &mut events, 0, 0, &mut returned).is_err() {
                break;
            }
            for event in &events[..returned as usize] {
                let event = Handle(EVT_HANDLE(*event));
                if let Some((process, instance)) = rendered(&context, &event) {
                    if instance.eq_ignore_ascii_case(device) {
                        processes.push(process);
                    }
                }
            }
        }
    }
    Ok(processes)
}

unsafe fn rendered(context: &Handle, event: &Handle) -> Option<(String, String)> {
    // u64 elements keep the variants aligned; their strings point into this buffer.
    let mut buffer = vec![0u64; 512];
    let (mut used, mut count) = (0, 0);
    unsafe {
        EvtRender(
            Some(context.0),
            event.0,
            EvtRenderEventValues.0,
            (buffer.len() * size_of::<u64>()) as u32,
            Some(buffer.as_mut_ptr().cast()),
            &mut used,
            &mut count,
        )
        .ok()?;
        if count != 2 {
            return None;
        }
        let values = std::slice::from_raw_parts(buffer.as_ptr().cast::<EVT_VARIANT>(), 2);
        Some((text(&values[0])?, text(&values[1])?))
    }
}

unsafe fn text(value: &EVT_VARIANT) -> Option<String> {
    if value.Type != EvtVarTypeString.0 as u32 {
        return None;
    }
    unsafe { value.Anonymous.StringVal.to_string().ok() }
}

/// Sorted executable names. The kernel process only says something when it is alone.
fn names(processes: Vec<String>) -> Vec<String> {
    let names: BTreeSet<String> = processes.iter().map(|process| executable(process).to_string()).collect();
    if names.len() > 1 {
        return names.into_iter().filter(|name| name != KERNEL_PROCESS).collect();
    }
    names.into_iter().collect()
}

/// The log gives NT paths such as `\Device\HarddiskVolume3\Windows\explorer.exe`.
fn executable(process: &str) -> &str {
    process.rsplit('\\').next().unwrap_or(process)
}

#[cfg(test)]
#[path = "windows_eject_holders/tests/holders.rs"]
mod tests;
