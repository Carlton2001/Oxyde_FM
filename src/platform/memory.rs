//! Process-local counters; virtual region totals are not resident RAM totals.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessMemorySnapshot {
    pub private_bytes: u64,
    pub working_set_bytes: u64,
    pub private_committed_bytes: u64,
    pub mapped_committed_bytes: u64,
    pub image_committed_bytes: u64,
    pub reserved_bytes: u64,
    pub icon_cache_entries_bytes: Option<(usize, usize)>,
}

#[cfg(target_os = "windows")]
pub(super) fn read_windows() -> Result<ProcessMemorySnapshot, String> {
    use windows::Win32::System::{
        Memory::{VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_IMAGE, MEM_MAPPED, MEM_PRIVATE, MEM_RESERVE},
        ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
        Threading::GetCurrentProcess,
    };
    let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
    counters.cb = std::mem::size_of_val(&counters) as u32;
    // The pseudo-handle is borrowed; querying does not alter the working set.
    unsafe {
        GetProcessMemoryInfo(GetCurrentProcess(), (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX).cast(), counters.cb)
    }.map_err(|error| error.to_string())?;
    let mut snapshot = ProcessMemorySnapshot {
        private_bytes: counters.PrivateUsage as u64,
        working_set_bytes: counters.WorkingSetSize as u64,
        icon_cache_entries_bytes: super::windows_icon_pixels::diagnostic_snapshot(),
        ..Default::default()
    };
    let mut address = 0usize;
    loop {
        let mut region = MEMORY_BASIC_INFORMATION::default();
        let read = unsafe { VirtualQuery(Some(address as *const _), &mut region, std::mem::size_of_val(&region)) };
        if read == 0 { break; }
        let size = region.RegionSize as u64;
        if region.State == MEM_RESERVE {
            snapshot.reserved_bytes += size;
        } else if region.State == MEM_COMMIT {
            match region.Type {
                MEM_PRIVATE => snapshot.private_committed_bytes += size,
                MEM_MAPPED => snapshot.mapped_committed_bytes += size,
                MEM_IMAGE => snapshot.image_committed_bytes += size,
                _ => {}
            }
        }
        let Some(next) = (region.BaseAddress as usize).checked_add(region.RegionSize) else { break };
        if next <= address { break; }
        address = next;
    }
    Ok(snapshot)
}

#[cfg(all(test, target_os = "windows"))]
#[path = "memory/tests/directx.rs"]
mod directx_tests;
