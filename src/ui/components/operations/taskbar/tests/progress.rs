use super::taskbar_progress;
use oxyde_lib::platform::TaskbarProgress;
use oxyde_lib::systems::file_ops::{FileOpType, OpStatus, OperationProgress};

fn operation(status: OpStatus, processed: u64, total: u64) -> OperationProgress {
    OperationProgress {
        id: String::new(),
        op_type: FileOpType::Copy,
        status,
        total_bytes: total,
        processed_bytes: processed,
        total_files: 0,
        processed_files: 0,
    }
}

#[test]
fn the_taskbar_shows_the_average_share_and_the_paused_or_measuring_states() {
    assert_eq!(taskbar_progress(&[]), TaskbarProgress::None);
    assert_eq!(taskbar_progress(&[operation(OpStatus::Calculating, 0, 0)]), TaskbarProgress::Indeterminate);
    assert_eq!(
        taskbar_progress(&[operation(OpStatus::Running, 1, 4), operation(OpStatus::Queued, 0, 0)]),
        TaskbarProgress::Running(0.125)
    );
    assert_eq!(
        taskbar_progress(&[operation(OpStatus::Paused, 1, 2), operation(OpStatus::Paused, 1, 1)]),
        TaskbarProgress::Paused(0.75)
    );
    // One paused beside a running one: the transfer still runs.
    assert_eq!(
        taskbar_progress(&[operation(OpStatus::Paused, 0, 2), operation(OpStatus::Running, 2, 2)]),
        TaskbarProgress::Running(0.5)
    );
}
