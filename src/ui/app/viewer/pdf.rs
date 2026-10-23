//! Complete PDFs: every page box is laid out from the known page sizes, and an
//! open worker session renders only the pages near the viewport. Requests for
//! pages that leave the view before rendering are skipped, so jumping to the
//! end of a long document does not wait for the pages in between.
use super::{content::Content, Viewer};
use gpui::*;
use oxyde_lib::platform::PdfSession;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

/// Decoded pages kept around the visible range (about 16 MiB each at most).
const MAX_PAGES: usize = 6;
const SKIPPED: &str = "PDF page left the view";

pub(super) struct Pdf {
    path: String,
    session: Arc<Mutex<Box<dyn PdfSession>>>,
    pub(super) sizes: Vec<(f32, f32)>,
    pub(super) pages: HashMap<usize, Arc<RenderImage>>,
    /// Requested pages and their cancellation flags.
    pending: HashMap<usize, Arc<AtomicBool>>,
    pub(super) failed: HashSet<usize>,
    recent: VecDeque<usize>,
    visible: std::ops::Range<usize>,
    pub(super) list: ListState,
}

impl Pdf {
    pub(super) fn retire(self, jobs: &mut super::deletion::Jobs, cx: &mut App) {
        let completion = jobs.register(&self.path);
        let session = self.session.clone();
        // List state stays on the UI thread. Keep a session reference until
        // its native worker can be closed in the background.
        drop(self);
        cx.background_executor().spawn(async move {
            match session.lock() {
                Ok(mut session) => {
                    if let Err(error) = session.close() {
                        log::warn!("Viewer PDF close failed: {error}");
                    }
                }
                Err(error) => log::warn!("Viewer PDF session unavailable during close: {error}"),
            }
            drop(session);
            drop(completion);
        }).detach();
    }

    fn cancel_page(&mut self, index: usize) {
        if let Some(cancelled) = self.pending.remove(&index) {
            cancelled.store(true, Ordering::Release);
        }
    }

    fn finish_request(&mut self, index: usize, request: &Arc<AtomicBool>) -> bool {
        if !self.pending.get(&index).is_some_and(|pending| Arc::ptr_eq(pending, request)) {
            return false;
        }
        self.pending.remove(&index);
        true
    }

    fn touch_page(&mut self, index: usize) -> Vec<Arc<RenderImage>> {
        self.recent.retain(|page| *page != index);
        self.recent.push_back(index);
        let mut evicted = Vec::new();
        while self.recent.len() > MAX_PAGES {
            if let Some(page) = self.recent.pop_front() {
                self.cancel_page(page);
                evicted.extend(self.pages.remove(&page));
            }
        }
        evicted
    }

    /// Snapshot taken by the Viewer's render, before the list lays out: GPUI
    /// keeps the list state borrowed while it calls the item renderer.
    pub(super) fn update_visible(&mut self) {
        let top = self.list.logical_scroll_top().item_ix;
        let viewport = self.list.viewport_bounds().size;
        let (width, height) = self.sizes.get(top).copied().unwrap_or((1.0, 1.0));
        let page = (viewport.width.as_f32() * height / width.max(1.0)).max(1.0);
        let count = (viewport.height.as_f32() / page).ceil() as usize + 1;
        self.visible = top.saturating_sub(1)..top + count + 1;
        let visible = self.visible.clone();
        self.pending.retain(|index, cancelled| {
            let keep = visible.contains(index);
            if !keep {
                cancelled.store(true, Ordering::Release);
            }
            keep
        });
    }

    pub(super) fn images(&self) -> impl Iterator<Item = Arc<RenderImage>> + '_ {
        self.pages.values().cloned()
    }
}

impl Drop for Pdf {
    fn drop(&mut self) {
        // A blocking request already running can finish; queued requests must
        // not keep rendering an obsolete document after its Viewer closes.
        for cancelled in self.pending.values() { cancelled.store(true, Ordering::Release); }
    }
}

impl Viewer {
    pub(super) fn load_pdf(&mut self, path: String, cx: &mut Context<Self>) {
        let generation = self.generation;
        let source_path = path.clone();
        let work = self.load_work(move || {
            oxyde_lib::platform::platform().open_pdf_session(std::path::Path::new(&path))
        });
        self.finish(
            generation,
            work,
            move |session| {
                let sizes = session.sizes().to_vec();
                Content::Pdf(Pdf {
                    path: source_path,
                    session: Arc::new(Mutex::new(session)),
                    // Page boxes are measured up front; only nearby pages are decoded.
                    list: ListState::new(sizes.len(), ListAlignment::Top, px(800.0)).measure_all(),
                    sizes,
                    pages: HashMap::new(),
                    pending: HashMap::new(),
                    failed: HashSet::new(),
                    recent: VecDeque::new(),
                    visible: 0..2,
                })
            },
            cx,
        );
    }

    /// Called while laying out a page box; renders it when it is near the
    /// viewport and evicts distant pages.
    pub(super) fn request_pdf_page(&mut self, index: usize, cx: &mut Context<Self>) {
        let generation = self.generation;
        let Content::Pdf(pdf) = &mut self.content else { return };
        if !pdf.visible.contains(&index) {
            return;
        }
        let evicted = pdf.touch_page(index);
        super::content::retire_images(evicted, cx);
        if pdf.pages.contains_key(&index) || pdf.pending.contains_key(&index) || pdf.failed.contains(&index) {
            return;
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        pdf.pending.insert(index, cancelled.clone());
        let request = cancelled.clone();
        let session = pdf.session.clone();
        let completion = self.jobs.register(&pdf.path);
        // The session serializes rendering; this job only waits on its own page.
        let work = cx.background_executor().spawn(async move {
            let result = (|| {
                let mut session = session.lock().map_err(|_| "PDF session unavailable".to_string())?;
                if cancelled.load(Ordering::Acquire) {
                    return Err(SKIPPED.to_string());
                }
                let image = session.page(index as u32).map_err(|error| error.to_string())?;
                // Bounded by MAX_PAGES, not by the tooltips' shared pixel budget:
                // hovering files must never make visible pages fail.
                let image = Arc::new(RenderImage::new(vec![image::Frame::new(image)]));
                crate::ui::memory_diagnostics::track(&image, crate::ui::memory_diagnostics::State::Decoded);
                Ok(image)
            })();
            // Close the final session/worker before releasing the deletion fence.
            drop(session);
            drop(completion);
            result
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                let Content::Pdf(pdf) = &mut this.content else { return };
                // A page may leave the viewport and be requested again while
                // its previous decode finishes. That old result owns no new request.
                if !pdf.finish_request(index, &request) { return; }
                match result {
                    // A page scrolled far away meanwhile is not retained.
                    Ok(image) if pdf.recent.contains(&index) => {
                        crate::ui::memory_diagnostics::track(&image, crate::ui::memory_diagnostics::State::Shown);
                        pdf.pages.insert(index, image);
                    }
                    // Unpublished pixels never entered the atlas and drop here.
                    Ok(_) => {},
                    Err(error) if error == SKIPPED => {}
                    Err(error) => {
                        log::warn!("Viewer could not render PDF page {index}: {error}");
                        pdf.failed.insert(index);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
#[path = "tests/pdf_lifetimes.rs"]
mod lifetime_tests;
