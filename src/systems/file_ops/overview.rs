//! What the operations panel shows of an operation without walking its sources
//! while rendering: their shared folder, the first ones by name, and how long the
//! work ran without its pauses.
use super::FileOperation;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[cfg(test)]
#[path = "tests/overview.rs"]
mod tests;

/// Sources a card lists before offering the full list.
pub(super) const PREVIEW_LEN: usize = 5;

/// The folder holding every source, or `None` when they come from several.
pub(super) fn common_parent(sources: &[PathBuf]) -> Option<PathBuf> {
    let parent = sources.first()?.parent()?;
    sources
        .iter()
        .all(|source| source.parent() == Some(parent))
        .then(|| parent.to_path_buf())
}

/// Natural, case-insensitive name order, as the file list sorts by name.
pub fn compare_names(left: &Path, right: &Path) -> Ordering {
    let name = |path: &Path| {
        path.file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned()
    };
    let (left_name, right_name) = (name(left), name(right));
    crate::utils::compare_natural(&left_name.to_lowercase(), &right_name.to_lowercase())
        .then_with(|| crate::utils::compare_natural(&left_name, &right_name))
        .then_with(|| left.cmp(right))
}

/// The first `count` sources in name order, in one pass over them.
pub(super) fn first_by_name(sources: &[PathBuf], count: usize) -> Vec<PathBuf> {
    let mut first: Vec<&PathBuf> = Vec::with_capacity(count + 1);
    for source in sources {
        let at = first.partition_point(|kept| compare_names(kept, source).is_le());
        if at < count {
            first.insert(at, source);
            first.truncate(count);
        }
    }
    first.into_iter().cloned().collect()
}

impl FileOperation {
    /// Starts or ends a pause of the running work. A pause before the work
    /// started costs nothing, as the clock starts with it.
    pub(super) fn pause_clock(&mut self, paused: bool) {
        let now = Instant::now();
        match (paused, self.paused_since) {
            (true, None) if self.started_at.is_some() => self.paused_since = Some(now),
            (false, Some(since)) => {
                self.paused_total += now.duration_since(since);
                self.paused_since = None;
                // The next speed sample measures work only.
                self.speed_sample_at = Some(now);
                self.speed_sample_bytes = self.processed_bytes;
            }
            _ => {}
        }
    }

    /// Time spent working since the start, pauses excluded.
    pub(super) fn active_duration(&self) -> Option<Duration> {
        let started = self.started_at?;
        let ongoing = self.paused_since.map_or(Duration::ZERO, |since| since.elapsed());
        Some(started.elapsed().saturating_sub(self.paused_total + ongoing))
    }
}
