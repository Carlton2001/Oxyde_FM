use crate::models::{CommandError, FolderSizeResult};
use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;
mod pools;

static SCAN_SLOTS: Lazy<Arc<tokio::sync::Semaphore>> =
    Lazy::new(|| Arc::new(tokio::sync::Semaphore::new(2)));
static SCAN_POOLS: Lazy<Arc<pools::ScanPools>> = Lazy::new(|| Arc::new(pools::ScanPools::new()));

fn make_pool(threads: usize) -> Result<Arc<rayon::ThreadPool>, String> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|index| format!("oxyde-sizes-{index}"))
        .build()
        .map(Arc::new)
        .map_err(|error| error.to_string())
}

#[derive(Clone)]
struct ScanCancellation {
    requested: Arc<AtomicBool>,
    abandoned: Arc<AtomicBool>,
}
impl ScanCancellation {
    fn cancelled(&self) -> bool {
        self.requested.load(Ordering::Relaxed) || self.abandoned.load(Ordering::Relaxed)
    }
    fn check(&self) -> Result<(), CommandError> {
        if self.cancelled() {
            Err(CommandError::Other(
                "Folder size calculation cancelled".into(),
            ))
        } else {
            Ok(())
        }
    }
}
struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

async fn run_scan(
    requested: Arc<AtomicBool>,
    slots: Arc<tokio::sync::Semaphore>,
    job: impl FnOnce(ScanCancellation) -> Result<FolderSizeResult, CommandError> + Send + 'static,
) -> Result<FolderSizeResult, CommandError> {
    let abandoned = Arc::new(AtomicBool::new(false));
    let _cancel_on_drop = CancelOnDrop(abandoned.clone());
    let cancellation = ScanCancellation {
        requested,
        abandoned,
    };
    let mut acquire = Box::pin(slots.acquire_owned());
    let permit = loop {
        cancellation.check()?;
        match tokio::time::timeout(Duration::from_millis(25), &mut acquire).await {
            Ok(result) => {
                break result.map_err(|error| CommandError::SystemError(error.to_string()))?
            }
            Err(_) => continue,
        }
    };
    cancellation.check()?;
    tokio::task::spawn_blocking(move || {
        // Keep the slot until the physical worker finishes, even if its async
        // caller was dropped. Otherwise navigation could exceed the limit.
        let _permit = permit;
        cancellation.check()?;
        job(cancellation)
    })
    .await
    .map_err(|error| CommandError::SystemError(format!("Task join error: {error}")))?
}

pub(crate) async fn calculate(
    path: PathBuf,
    cancellation: Arc<AtomicBool>,
) -> Result<FolderSizeResult, CommandError> {
    run_scan(cancellation, SCAN_SLOTS.clone(), move |cancellation| {
        cancellation.check()?;
        if !path.is_dir() {
            return Err(CommandError::PathError("Path is not a directory".into()));
        }
        // jwalk's par_bridge retains every worker in its pool until that walk
        // ends. A second walk on the same pool would wait for the entire first
        // directory, even though it already holds its own semaphore permit.
        let lane = SCAN_POOLS.acquire().map_err(CommandError::SystemError)?;
        let pool = lane.pool(crate::utils::hardware::is_ssd(&path))
            .map_err(CommandError::SystemError)?;
        scan(&path, cancellation, pool, || {})
    })
    .await
}

fn scan(
    path: &Path,
    cancellation: ScanCancellation,
    pool: Arc<rayon::ThreadPool>,
    mut checkpoint: impl FnMut(),
) -> Result<FolderSizeResult, CommandError> {
    cancellation.check()?;
    let size = Arc::new(AtomicU64::new(0));
    let folders = Arc::new(AtomicU64::new(0));
    let files = Arc::new(AtomicU64::new(0));
    let size_acc = size.clone();
    let folders_acc = folders.clone();
    let files_acc = files.clone();
    let worker_cancel = cancellation.clone();
    let walker = jwalk::WalkDir::new(path)
        .skip_hidden(false)
        .follow_links(false)
        .parallelism(jwalk::Parallelism::RayonExistingPool {
            pool,
            busy_timeout: None,
        })
        .process_read_dir(move |depth, path, _, children| {
            if worker_cancel.cancelled() || excluded(path) {
                children.clear();
                return;
            }
            children.retain(|child| match child {
                Ok(entry) => {
                    !entry.file_type().is_symlink()
                        && (!entry.file_type().is_dir() || !excluded(&entry.path()))
                }
                Err(_) => true,
            });
            if depth.is_none() {
                return;
            }
            for entry in children.iter().flatten() {
                if worker_cancel.cancelled() {
                    children.clear();
                    return;
                }
                if entry.file_type().is_file() {
                    files_acc.fetch_add(1, Ordering::Relaxed);
                    if let Ok(metadata) = entry.metadata() {
                        let _ =
                            size_acc.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                                Some(value.saturating_add(metadata.len()))
                            });
                    }
                } else if entry.file_type().is_dir() {
                    folders_acc.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
    for _ in walker {
        checkpoint();
        cancellation.check()?;
    }
    cancellation.check()?;
    Ok(FolderSizeResult {
        size: size.load(Ordering::Relaxed),
        folders_count: folders.load(Ordering::Relaxed),
        files_count: files.load(Ordering::Relaxed),
    })
}

/// Match whole components so similarly named user folders remain measurable.
pub(crate) fn excluded(path: &Path) -> bool {
    let normalized = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    let parts: Vec<_> = normalized.split('/').collect();
    parts.contains(&"system volume information")
        || parts
            .windows(3)
            .any(|parts| parts == ["microsoft", "windows", "containers"])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> FolderSizeResult {
        FolderSizeResult {
            size: 0,
            files_count: 0,
            folders_count: 0,
        }
    }

    #[test]
    fn queued_scan_can_be_cancelled_before_starting_its_worker() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let slots = Arc::new(tokio::sync::Semaphore::new(1));
            let _occupied = slots.clone().acquire_owned().await.unwrap();
            let requested = Arc::new(AtomicBool::new(false));
            let handle = tokio::spawn(run_scan(requested.clone(), slots, |_| {
                panic!("cancelled queued scan started")
            }));
            tokio::time::sleep(Duration::from_millis(30)).await;
            requested.store(true, Ordering::Relaxed);
            assert!(tokio::time::timeout(Duration::from_secs(1), handle)
                .await
                .unwrap()
                .unwrap()
                .is_err());
        });
    }

    #[test]
    fn dropping_async_caller_cancels_worker_but_keeps_slot_until_it_finishes() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let slots = Arc::new(tokio::sync::Semaphore::new(1));
            let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let (done_tx, done_rx) = tokio::sync::oneshot::channel();
            let handle = tokio::spawn(run_scan(
                Arc::new(AtomicBool::new(false)),
                slots.clone(),
                move |cancellation| {
                    ready_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    done_tx.send(cancellation.cancelled()).unwrap();
                    Ok(empty())
                },
            ));
            ready_rx.await.unwrap();
            handle.abort();
            assert!(handle.await.unwrap_err().is_cancelled());
            assert_eq!(slots.available_permits(), 0);
            release_tx.send(()).unwrap();
            assert!(done_rx.await.unwrap());
            let _permit = tokio::time::timeout(Duration::from_secs(1), slots.acquire())
                .await
                .unwrap()
                .unwrap();
        });
    }

    #[test]
    fn cancellation_during_traversal_never_returns_partial_totals() {
        let root = tempfile::tempdir().unwrap();
        for index in 0..64 {
            let folder = root.path().join(index.to_string());
            std::fs::create_dir(&folder).unwrap();
            std::fs::write(folder.join("file"), b"size").unwrap();
        }
        let requested = Arc::new(AtomicBool::new(false));
        let cancellation = ScanCancellation {
            requested: requested.clone(),
            abandoned: Arc::new(AtomicBool::new(false)),
        };
        let result = scan(root.path(), cancellation, make_pool(2).unwrap(), || {
            requested.store(true, Ordering::Relaxed)
        });
        assert!(result.unwrap_err().to_string().contains("cancelled"));
    }

    #[test]
    #[ignore = "manual filesystem profile comparing per-folder pools with bounded shared scans"]
    fn profile_many_folder_scans() {
        let root = tempfile::tempdir().unwrap();
        let paths: Vec<_> = (0..4)
            .map(|index| root.path().join(index.to_string()))
            .collect();
        for path in &paths {
            for branch in 0..32 {
                let folder = path.join(branch.to_string());
                std::fs::create_dir_all(&folder).unwrap();
                for file in 0..16 {
                    std::fs::write(folder.join(file.to_string()), b"generated fixture").unwrap();
                }
            }
        }
        let cancel = || ScanCancellation {
            requested: Arc::new(AtomicBool::new(false)),
            abandoned: Arc::new(AtomicBool::new(false)),
        };
        let start = std::time::Instant::now();
        let old: Vec<_> = paths
            .iter()
            .cloned()
            .map(|path| {
                std::thread::spawn(move || {
                    scan(&path, cancel(), make_pool(16).unwrap(), || {}).unwrap()
                })
            })
            .collect();
        for worker in old {
            assert_eq!(worker.join().unwrap().files_count, 512);
        }
        let previous = start.elapsed();
        let pool = make_pool(16).unwrap();
        let slots = Arc::new(tokio::sync::Semaphore::new(2));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let start = std::time::Instant::now();
        runtime.block_on(async {
            let results = futures::future::join_all(paths.into_iter().map(|path| {
                let pool = pool.clone();
                run_scan(
                    Arc::new(AtomicBool::new(false)),
                    slots.clone(),
                    move |cancel| scan(&path, cancel, pool, || {}),
                )
            }))
            .await;
            for result in results {
                assert_eq!(result.unwrap().files_count, 512);
            }
        });
        eprintln!("4 folders x 512 files: per-folder pools (64 Rayon threads) {previous:?}; shared pool (16 threads, 2 active scans) {:?}", start.elapsed());
    }
}
