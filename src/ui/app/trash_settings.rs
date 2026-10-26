use super::*;
#[cfg(target_os = "windows")]
use oxyde_lib::commands::ops::TrashSystemConfig;
use oxyde_lib::models::DriveInfo;

mod actions;
mod confirmation;
mod deletion_policy;
#[cfg(target_os = "windows")]
mod render;
#[cfg(target_os = "windows")]
mod settings;
#[cfg(not(target_os = "windows"))]
mod linux;

pub(crate) use confirmation::render_delete_confirmation;
#[cfg(target_os = "windows")]
pub(crate) use render::render;
#[cfg(not(target_os = "windows"))]
pub(crate) use linux::{render, TrashSettings};
pub(crate) use deletion_policy::{DeletePolicy, read_drive_policy};

#[cfg(target_os = "windows")]
pub(crate) struct TrashSettings {
    drives: Vec<DriveInfo>,
    configs: Vec<TrashSystemConfig>,
    initial: Vec<TrashSystemConfig>,
    selected: usize,
    percent: bool,
    input: Entity<TextInput>,
    value: String,
    value_dirty: bool,
    confirm: bool,
    usage: (u64, u64),
    loading: bool,
    pub(super) busy: bool,
    error: Option<String>,
    pub(super) focus: FocusHandle,
    save_focus: FocusHandle,
}

#[cfg(target_os = "windows")]
impl TrashSettings {
    pub(super) fn request_id(&self) -> EntityId { self.input.entity_id() }
}

#[cfg(target_os = "windows")]
fn capacity_mb(value: &str, percent: bool, total_bytes: u64) -> Option<u64> {
    let number: u64 = value.parse().ok()?;
    if percent {
        if number > 100 {
            return None;
        }
        if total_bytes == 0 {
            return None;
        }
        let capacity = ((total_bytes as u128 * number as u128) / (100 * 1024 * 1024)) as u64;
        (capacity <= u32::MAX as u64).then_some(capacity)
    } else {
        (number <= u32::MAX as u64).then_some(number)
    }
}

#[cfg(target_os = "windows")]
fn capacity_percent(max_size_mb: u64, total_bytes: u64) -> u64 {
    if total_bytes == 0 {
        return 0;
    }
    let total = total_bytes as u128;
    let scaled = max_size_mb as u128 * 1024 * 1024 * 100;
    ((scaled + total / 2) / total) as u64
}


pub(crate) struct PendingDelete {
    pub paths: Vec<String>,
    pub permanent: bool,
    pub permanent_prompt: bool,
    pub focus: FocusHandle,
    pub confirm_focus: FocusHandle,
}

#[cfg(all(test, target_os = "windows"))]
#[path = "trash_settings/tests/capacity.rs"]
mod tests;
