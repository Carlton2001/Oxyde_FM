use super::*;
use oxyde_lib::commands::ops::TrashSystemConfig;
use oxyde_lib::models::DriveInfo;

mod actions;
mod confirmation;
mod deletion_policy;
mod render;
mod settings;

pub(crate) use confirmation::render_delete_confirmation;
pub(crate) use render::render;
pub(crate) use deletion_policy::{DeletePolicy, read_drive_policy};

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
    busy: bool,
    error: Option<String>,
    focus: FocusHandle,
    save_focus: FocusHandle,
}

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


pub(crate) struct PendingDelete {
    pub paths: Vec<String>,
    pub permanent: bool,
    pub permanent_prompt: bool,
    pub focus: FocusHandle,
    pub confirm_focus: FocusHandle,
}

#[cfg(test)]
#[path = "trash_settings/tests/capacity.rs"]
mod tests;
