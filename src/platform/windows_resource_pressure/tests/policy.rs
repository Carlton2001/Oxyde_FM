use super::*;
#[test]
fn measured_thresholds_refuse_each_exhausted_resource_and_accept_recovery() {
    let healthy = Sample {
        parent_commit: PARENT_COMMIT - 1,
        parent_handles: PARENT_HANDLES - 1,
        available_physical: MIN_AVAILABLE,
        available_commit: MIN_AVAILABLE,
    };
    assert!(healthy.admitted());
    for pressured in [
        Sample {
            parent_commit: PARENT_COMMIT,
            ..healthy
        },
        Sample {
            parent_handles: PARENT_HANDLES,
            ..healthy
        },
        Sample {
            available_physical: MIN_AVAILABLE - 1,
            ..healthy
        },
        Sample {
            available_commit: MIN_AVAILABLE - 1,
            ..healthy
        },
    ] {
        assert!(!pressured.admitted());
        assert!(healthy.admitted());
    }
}
#[test]
fn native_measurements_return_real_parent_and_system_counters() {
    let sample = Sample::read().unwrap();
    assert!(sample.parent_commit > 0);
    assert!(sample.parent_handles > 0);
    assert!(sample.available_physical > 0 && sample.available_commit > 0);
}

#[test]
fn native_worker_handle_saturation_stops_work_and_allows_a_subsequent_worker() {
    use super::super::{windows_preview_worker, windows_process_launch::native_probe};
    let error = windows_preview_worker::supervise(
        native_probe::command("handles"),
        Duration::from_secs(15),
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    assert!(error.to_string().contains("Worker handle pressure"));
    let reply = windows_preview_worker::supervise(
        native_probe::command("success"),
        Duration::from_secs(15)
    ).unwrap();
    assert_eq!(reply, [1]);
}
