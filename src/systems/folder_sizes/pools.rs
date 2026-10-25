use once_cell::sync::OnceCell;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Independent walks: at most 4 normal / 16 Turbo SSD workers, or 2 HDD workers.
pub(super) struct ScanPools {
    busy: [AtomicBool; 2],
    pools: [[OnceCell<Result<Arc<rayon::ThreadPool>, String>>; 3]; 2],
    ssd_workers: usize,
}

impl ScanPools {
    pub(super) fn new() -> Self {
        Self::with_parallelism(std::thread::available_parallelism().map_or(2, usize::from))
    }

    fn with_parallelism(parallelism: usize) -> Self {
        Self {
            busy: std::array::from_fn(|_| AtomicBool::new(false)),
            pools: std::array::from_fn(|_| std::array::from_fn(|_| OnceCell::new())),
            ssd_workers: parallelism.div_ceil(2).clamp(1, 8),
        }
    }

    pub(super) fn acquire(self: &Arc<Self>) -> Result<ScanPoolLease, String> {
        let index = self
            .busy
            .iter()
            .position(|busy| {
                busy.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
            })
            .ok_or("Folder size scan has no available worker lane")?;
        Ok(ScanPoolLease {
            pools: self.clone(),
            index,
        })
    }
}

pub(super) struct ScanPoolLease {
    pools: Arc<ScanPools>,
    index: usize,
}

impl ScanPoolLease {
    pub(super) fn pool(&self, ssd: bool, turbo: bool) -> Result<Arc<rayon::ThreadPool>, String> {
        let (kind, workers) = if !ssd {
            (0, 1)
        } else if turbo {
            (2, self.pools.ssd_workers)
        } else {
            (1, self.pools.ssd_workers.min(2))
        };
        let cell = &self.pools.pools[self.index][kind];
        cell.get_or_init(|| super::make_pool(workers)).clone()
    }
}

impl Drop for ScanPoolLease {
    fn drop(&mut self) {
        self.pools.busy[self.index].store(false, Ordering::Release);
    }
}

#[cfg(test)]
#[path = "tests/pools.rs"]
mod tests;
