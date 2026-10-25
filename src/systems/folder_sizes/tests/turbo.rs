use super::*;

#[test]
fn live_turbo_transitions_preserve_totals_and_allow_reusing_the_same_lane() {
    let root = tempfile::tempdir().unwrap();
    for index in 0..64 {
        let folder = root.path().join(index.to_string());
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("file"), vec![0; index + 1]).unwrap();
    }
    let turbo = Arc::new(AtomicBool::new(false));
    let pool = make_pool(4).unwrap();
    for initial in [false, true] {
        turbo.store(initial, Ordering::Relaxed);
        let mut checkpoints = 0;
        let result = scan(
            root.path(),
            ScanCancellation {
                requested: Arc::new(AtomicBool::new(false)),
                abandoned: Arc::new(AtomicBool::new(false)),
            },
            pool.clone(),
            turbo.clone(),
            || {
                checkpoints += 1;
                turbo.fetch_xor(true, Ordering::Relaxed);
            },
        )
        .unwrap();
        assert!(checkpoints > 2);
        assert_eq!(result.size, 2080);
        assert_eq!(result.files_count, 64);
        assert_eq!(result.folders_count, 64);
    }
}

#[test]
#[ignore = "manual storage profile; creates 8192 files and compares normal/Turbo worker budgets"]
fn profile_normal_and_turbo_size_scans() {
    let root = tempfile::tempdir().unwrap();
    let mut many = Vec::new();
    for index in 0..32 {
        let path = root.path().join(index.to_string());
        for branch in 0..8 {
            let folder = path.join(branch.to_string());
            std::fs::create_dir_all(&folder).unwrap();
            for file in 0..32 {
                std::fs::write(folder.join(file.to_string()), [0; 64]).unwrap();
            }
        }
        many.push(path);
    }
    let pools = Arc::new(pools::ScanPools::new());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let slots = Arc::new(tokio::sync::Semaphore::new(2));
    eprintln!(
        "size profile: logical_cpus={:?}; ssd={}; cache=warm; profile=dev",
        std::thread::available_parallelism(),
        crate::utils::hardware::is_ssd(root.path())
    );
    for (name, paths) in [
        ("one large folder", vec![root.path().to_owned()]),
        ("32 small folders", many),
    ] {
        let mut samples = [Vec::new(), Vec::new()];
        for pass in 0..8 {
            let order = if pass % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            };
            for turbo in order {
                let started = std::time::Instant::now();
                let results = runtime.block_on(futures::future::join_all(
                    paths.iter().cloned().map(|path| {
                        let pools = pools.clone();
                        run_scan(
                            Arc::new(AtomicBool::new(false)),
                            slots.clone(),
                            move |cancel| {
                                let lane = pools.acquire().unwrap();
                                scan(
                                    &path,
                                    cancel,
                                    lane.pool(true, turbo).unwrap(),
                                    Arc::new(AtomicBool::new(turbo)),
                                    || {},
                                )
                            },
                        )
                    }),
                ));
                let elapsed = started.elapsed();
                assert_eq!(
                    results
                        .iter()
                        .map(|r| r.as_ref().unwrap().size)
                        .sum::<u64>(),
                    8192 * 64
                );
                assert_eq!(
                    results
                        .iter()
                        .map(|r| r.as_ref().unwrap().files_count)
                        .sum::<u64>(),
                    8192
                );
                if pass > 0 {
                    samples[usize::from(turbo)].push(elapsed);
                }
            }
        }
        for (index, times) in samples.iter_mut().enumerate() {
            times.sort();
            eprintln!(
                "{name}: turbo={}; median={:?}; samples={times:?}",
                index != 0,
                times[times.len() / 2]
            );
        }
    }
}
