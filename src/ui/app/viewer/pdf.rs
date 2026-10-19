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

impl Viewer {
    pub(super) fn load_pdf(&mut self, path: String, cx: &mut Context<Self>) {
        let generation = self.generation;
        let work = crate::ui::preview_jobs::load_viewer(move || {
            oxyde_lib::platform::platform().open_pdf_session(std::path::Path::new(&path))
        });
        self.finish(
            generation,
            work,
            |session| {
                let sizes = session.sizes().to_vec();
                Content::Pdf(Pdf {
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
        pdf.recent.retain(|page| *page != index);
        pdf.recent.push_back(index);
        if pdf.pages.contains_key(&index) || pdf.pending.contains_key(&index) || pdf.failed.contains(&index) {
            return;
        }
        let mut evicted = Vec::new();
        while pdf.recent.len() > MAX_PAGES {
            if let Some(page) = pdf.recent.pop_front() {
                evicted.extend(pdf.pages.remove(&page));
            }
        }
        for image in evicted {
            cx.drop_image(image, None);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        pdf.pending.insert(index, cancelled.clone());
        let session = pdf.session.clone();
        // The session serializes rendering; this job only waits on its own page.
        let work = cx.background_executor().spawn(async move {
            let mut session = session.lock().map_err(|_| "PDF session unavailable".to_string())?;
            if cancelled.load(Ordering::Acquire) {
                return Err(SKIPPED.to_string());
            }
            let image = session.page(index as u32).map_err(|error| error.to_string())?;
            // Bounded by MAX_PAGES, not by the tooltips' shared pixel budget:
            // hovering files must never make visible pages fail.
            Ok(Arc::new(RenderImage::new(vec![image::Frame::new(image)])))
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                let Content::Pdf(pdf) = &mut this.content else {
                    if let Ok(image) = result {
                        cx.drop_image(image, None);
                    }
                    return;
                };
                if this.generation != generation {
                    return;
                }
                pdf.pending.remove(&index);
                match result {
                    // A page scrolled far away meanwhile is not retained.
                    Ok(image) if pdf.recent.contains(&index) => {
                        pdf.pages.insert(index, image);
                    }
                    Ok(image) => cx.drop_image(image, None),
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
