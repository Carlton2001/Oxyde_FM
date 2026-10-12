use once_cell::sync::OnceCell;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Two independently scheduled walks, with a combined budget of 16 SSD / 2 HDD workers.
pub(super) struct ScanPools {
    busy: [AtomicBool; 2],
    ssd: [OnceCell<Result<Arc<rayon::ThreadPool>, String>>; 2],
    hdd: [OnceCell<Result<Arc<rayon::ThreadPool>, String>>; 2],
}

impl ScanPools {
    pub(super) fn new() -> Self {
        Self {
            busy: std::array::from_fn(|_| AtomicBool::new(false)),
            ssd: std::array::from_fn(|_| OnceCell::new()),
            hdd: std::array::from_fn(|_| OnceCell::new()),
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
    pub(super) fn pool(&self, ssd: bool) -> Result<Arc<rayon::ThreadPool>, String> {
        let (cell, workers) = if ssd {
            (&self.pools.ssd[self.index], 8)
        } else {
            (&self.pools.hdd[self.index], 1)
        };
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
