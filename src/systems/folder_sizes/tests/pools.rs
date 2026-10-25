use super::ScanPools;
use crate::systems::folder_sizes::{scan, ScanCancellation};
use std::sync::{atomic::AtomicBool, Arc, Condvar, Mutex};
use std::time::Duration;

#[test]
fn a_small_folder_finishes_while_every_worker_of_another_scan_is_busy() {
    for ssd in [false, true] {
        let pools = Arc::new(ScanPools::new());
        let first = pools.acquire().unwrap();
        let pool = first.pool(ssd, true).unwrap();
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        for _ in 0..pool.current_num_threads() {
            let gate = gate.clone();
            let ready = ready_tx.clone();
            pool.spawn(move || {
                let (lock, wake) = &*gate;
                let mut released = lock.lock().unwrap();
                ready.send(()).unwrap();
                while !*released {
                    released = wake.wait(released).unwrap();
                }
            });
        }
        for _ in 0..pool.current_num_threads() {
            ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("small.txt"), b"independent").unwrap();
        let second_pools = pools.clone();
        let path = root.path().to_owned();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let second = std::thread::spawn(move || {
            let lane = second_pools.acquire().unwrap();
            let result = scan(
                &path,
                ScanCancellation {
                    requested: Arc::new(AtomicBool::new(false)),
                    abandoned: Arc::new(AtomicBool::new(false)),
                },
                lane.pool(ssd, true).unwrap(),
                Arc::new(AtomicBool::new(true)),
                || {},
            );
            let _ = done_tx.send(result);
        });
        let result = done_rx.recv_timeout(Duration::from_secs(2));
        // Always release the first lane before asserting, including on failure.
        let (lock, wake) = &*gate;
        *lock.lock().unwrap() = true;
        wake.notify_all();
        second.join().unwrap();
        let size = result
            .expect("small folder waited for the saturated first scan")
            .unwrap();
        assert_eq!(size.files_count, 1);
        assert_eq!(size.size, 11);
        assert!(pools.acquire().is_ok());
        drop(first);
    }
}

#[test]
fn worker_lanes_are_reused_after_release_and_keep_the_combined_thread_budget() {
    let pools = Arc::new(ScanPools::with_parallelism(32));
    let first = pools.acquire().unwrap();
    let second = pools.acquire().unwrap();
    assert!(pools.acquire().is_err());
    assert_eq!(
        first.pool(true, true).unwrap().current_num_threads()
            + second.pool(true, true).unwrap().current_num_threads(),
        16
    );
    assert_eq!(
        first.pool(false, true).unwrap().current_num_threads()
            + second.pool(false, false).unwrap().current_num_threads(),
        2
    );
    let previous = first.pool(true, true).unwrap();
    drop(first);
    let replacement = pools.acquire().unwrap();
    assert!(Arc::ptr_eq(
        &previous,
        &replacement.pool(true, true).unwrap()
    ));
}

#[test]
fn subsequent_scans_change_budget_and_reuse_cached_pools_within_hardware_cap() {
    for parallelism in [1, 2, 4, 8, 16, 64] {
        let pools = Arc::new(ScanPools::with_parallelism(parallelism));
        let lane = pools.acquire().unwrap();
        let normal = lane.pool(true, false).unwrap();
        let turbo = lane.pool(true, true).unwrap();
        assert!(normal.current_num_threads() <= 2);
        assert!(turbo.current_num_threads() <= 8);
        assert!(turbo.current_num_threads() >= normal.current_num_threads());
        assert!(turbo.current_num_threads() <= parallelism);
        assert_eq!(lane.pool(false, false).unwrap().current_num_threads(), 1);
        drop(lane);
        let next = pools.acquire().unwrap();
        assert!(Arc::ptr_eq(&turbo, &next.pool(true, true).unwrap()));
        assert!(Arc::ptr_eq(&normal, &next.pool(true, false).unwrap()));
    }
}
