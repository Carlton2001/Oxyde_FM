use super::*;
use std::io;

#[test]
fn phase_times_exclude_pause_and_flush_stays_inside_native() {
    let mut metrics = Measurements::default();
    metrics.record(Phase::Flush, Duration::from_secs(3), Duration::ZERO);
    metrics.record(Phase::Pause, Duration::from_secs(2), Duration::ZERO);
    metrics.record(Phase::Native, Duration::from_secs(10), Duration::ZERO);
    assert_eq!(metrics.native, Duration::from_secs(8));
    assert_eq!(metrics.flush, Duration::from_secs(3));
    assert_eq!(metrics.flush_calls, 1);
    metrics.record(Phase::Hash, Duration::from_secs(4), Duration::from_secs(2));
    assert_eq!(metrics.hash, Duration::from_secs(4));
}

#[test]
fn nested_file_scopes_restore_parent_and_worker_context() {
    let _worker = WorkerScope::enter("test-operation".into(), 2);
    let outer = FileTiming::start(Path::new("a"), Path::new("b"));
    FILES.with(|files| files.borrow_mut().last_mut().unwrap().pause = Duration::from_secs(2));
    {
        let _inner = FileTiming::start(Path::new("c"), Path::new("d"));
        FILES.with(|files| assert_eq!(files.borrow().last().unwrap().pause, Duration::ZERO));
    }
    FILES.with(|files| assert_eq!(files.borrow().last().unwrap().pause, Duration::from_secs(2)));
    drop(outer);
    FILES.with(|files| assert!(files.borrow().is_empty()));
    drop(_worker);
    WORKER.with(|worker| assert!(worker.borrow().is_none()));
}

#[test]
fn remaining_time_excludes_all_top_level_phases_without_double_counting_flush() {
    let mut metrics = Measurements::default();
    for phase in [
        Phase::Parents,
        Phase::StagingCreate,
        Phase::WriterRelease,
        Phase::OutputOpen,
        Phase::Checks,
        Phase::Cleanup,
        Phase::Hash,
        Phase::Publish,
    ] {
        metrics.record(phase, Duration::from_secs(1), Duration::ZERO);
    }
    metrics.record(Phase::Native, Duration::from_secs(5), Duration::ZERO);
    metrics.record(Phase::Flush, Duration::from_secs(3), Duration::ZERO);
    metrics.record(Phase::Pause, Duration::from_secs(2), Duration::ZERO);
    assert_eq!(
        metrics.remaining(Duration::from_secs(19)),
        Duration::from_secs(4)
    );
    assert_eq!(metrics.remaining(Duration::from_secs(1)), Duration::ZERO);
}

#[test]
fn failed_phase_still_records_elapsed_time_and_restores_scope() {
    let file = FileTiming::start(Path::new("a"), Path::new("b"));
    let fail = || -> io::Result<()> {
        let _timer = Timer::start(Phase::OutputOpen);
        Err(io::Error::other("injected open failure"))
    };
    assert!(fail().is_err());
    FILES.with(|files| assert!(files.borrow().last().unwrap().output_open > Duration::ZERO));
    drop(file);
    FILES.with(|files| assert!(files.borrow().is_empty()));
}
