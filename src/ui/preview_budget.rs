//! Shared logical budget for retained tooltip pixels (images, PDFs and video).
//! Weak references account for cache, entity and render clones until final release.
//! This does not measure GPU textures or decoder scratch allocations.
use gpui::RenderImage;
use std::sync::{Arc, Mutex, OnceLock, Weak};

const MAX_BYTES: usize = 256 * 1024 * 1024;
const MAX_IMAGES: usize = 256;

struct Entry {
    image: Weak<RenderImage>,
    bytes: usize,
}
struct Budget {
    images: Vec<Entry>,
    bytes: usize,
    limit: usize,
    entries: usize,
}
impl Budget {
    fn new(limit: usize, entries: usize) -> Self {
        Self {
            images: Vec::new(),
            bytes: 0,
            limit,
            entries,
        }
    }
    fn admit(&mut self, image: &Arc<RenderImage>) -> Result<(), String> {
        self.images.retain(|entry| entry.image.strong_count() != 0);
        self.bytes = self.images.iter().map(|entry| entry.bytes).sum();
        let weak = Arc::downgrade(image);
        if self.images.iter().any(|entry| entry.image.ptr_eq(&weak)) {
            return Ok(());
        }
        let bytes = (0..image.frame_count())
            .try_fold(0usize, |total, frame| {
                total.checked_add(image.as_bytes(frame)?.len())
            })
            .ok_or_else(|| "Invalid preview pixel accounting".to_string())?;
        if bytes > self.limit.saturating_sub(self.bytes) || self.images.len() >= self.entries {
            return Err("Global retained preview pixel budget exceeded".into());
        }
        self.bytes += bytes;
        self.images.push(Entry { image: weak, bytes });
        Ok(())
    }
}

pub(crate) fn admit(image: &Arc<RenderImage>) -> Result<(), String> {
    static BUDGET: OnceLock<Mutex<Budget>> = OnceLock::new();
    BUDGET
        .get_or_init(|| Mutex::new(Budget::new(MAX_BYTES, MAX_IMAGES)))
        .lock()
        .map_err(|_| "Preview pixel budget unavailable".to_string())?
        .admit(image)
}

#[cfg(test)]
#[path = "preview_budget/tests/lifetimes.rs"]
mod tests;
