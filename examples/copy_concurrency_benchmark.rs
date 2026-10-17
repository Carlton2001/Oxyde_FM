//! Compare Turbo copy worker counts on generated file sets, per destination directory.
//! Usage: copy_concurrency_benchmark DESTINATION_DIRECTORY... ; each destination gets a
//! private subdirectory that is removed afterwards. Sources live under target/.
//! COPY_BENCH_SET=large|medium|small limits the timed copies to one set.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use oxyde_lib::systems::file_ops::{with_copy_workers_for_benchmark, FileOpType, FileOperation, FileOperationManager};
    use std::{fs, io::Write, path::PathBuf, sync::{atomic::Ordering, Arc, Mutex}, time::Instant};

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();
    let destinations: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if destinations.is_empty() {
        return Err("Usage: copy_concurrency_benchmark DESTINATION_DIRECTORY...".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("copy-concurrency-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root)?;
    let mut state = 0x9e37_79b9u32;
    let mut noise = |length: usize| -> Vec<u8> {
        (0..length).map(|_| { state ^= state << 13; state ^= state >> 17; state ^= state << 5; state as u8 }).collect()
    };
    // Sets: (name, files, bytes per file). Random content defeats any compression.
    let sets = [("large", 8usize, 256usize << 20), ("medium", 64, 8 << 20), ("small", 2000, 64 << 10)];
    for (name, count, size) in sets {
        let directory = root.join(name);
        fs::create_dir(&directory)?;
        let block = noise(size.min(8 << 20));
        for index in 0..count {
            let mut file = fs::File::create_new(directory.join(format!("{index:05}.bin")))?;
            let mut written = 0;
            while written < size {
                let part = (size - written).min(block.len());
                file.write_all(&block[..part])?;
                written += part;
            }
        }
    }
    println!("destination,set,workers,repetition,seconds,mib_per_s");
    for destination in &destinations {
        let base = fs::canonicalize(destination)?.join(format!("oxyde-copy-bench-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&base)?;
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let only = std::env::var("COPY_BENCH_SET").ok();
            for (name, count, size) in sets {
                if only.as_deref().is_some_and(|only| only != name) {
                    continue;
                }
                for repetition in 0..2 {
                    // Rotate the order so cache warm-up does not favour one count.
                    // 0 runs the production policy.
                    let mut counts = vec![0usize, 1, 2, 4];
                    counts.rotate_left(repetition % 4);
                    for workers in counts {
                        let target = base.join(format!("{name}-{workers}-{repetition}"));
                        fs::create_dir(&target)?;
                        let operation = FileOperation::new(FileOpType::Copy, vec![root.join(name)], Some(target.clone()), None);
                        operation.turbo_flag.store(true, Ordering::Relaxed);
                        let operation = Arc::new(Mutex::new(operation));
                        let started = Instant::now();
                        with_copy_workers_for_benchmark(workers, || {
                            FileOperationManager::perform_copy(&operation, vec![root.join(name)], Some(target.clone()), false, None)
                        })?;
                        let seconds = started.elapsed().as_secs_f64();
                        let mib = (count * size) as f64 / (1 << 20) as f64;
                        println!("{},{name},{workers},{repetition},{seconds:.3},{:.1}", destination.display(), mib / seconds);
                        fs::remove_dir_all(&target)?;
                    }
                }
            }
            Ok(())
        })();
        fs::remove_dir_all(&base)?;
        result?;
    }
    fs::remove_dir_all(&root)?;
    Ok(())
}
