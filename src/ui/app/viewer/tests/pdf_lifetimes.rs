use super::*;

struct UnusedSession;
impl PdfSession for UnusedSession {
    fn sizes(&self) -> &[(f32, f32)] {
        &[]
    }
    fn page(&mut self, _: u32) -> Result<image::RgbaImage, oxyde_lib::models::CommandError> {
        panic!("lifetime policy must not decode a page")
    }
}

fn document() -> Pdf {
    Pdf {
        path: "test.pdf".into(),
        session: Arc::new(Mutex::new(Box::new(UnusedSession))),
        sizes: vec![(100.0, 100.0); 20],
        pages: HashMap::new(),
        pending: HashMap::new(),
        failed: HashSet::new(),
        recent: VecDeque::new(),
        visible: 0..20,
        list: ListState::new(20, ListAlignment::Top, px(0.0)),
    }
}

#[::core::prelude::v1::test]
fn closing_a_pdf_cancels_all_queued_pages_and_releases_its_session() {
    let mut pdf = document();
    let session = Arc::downgrade(&pdf.session);
    let pending: Vec<_> = (0..MAX_PAGES)
        .map(|_| Arc::new(AtomicBool::new(false)))
        .collect();
    pdf.pending.extend(pending.iter().cloned().enumerate());
    drop(pdf);
    assert!(pending
        .iter()
        .all(|cancelled| cancelled.load(Ordering::Acquire)));
    assert!(session.upgrade().is_none());
}

#[::core::prelude::v1::test]
fn evicting_pages_bounds_pixels_and_cancels_requests_before_they_decode() {
    let mut pdf = document();
    let mut flags = Vec::new();
    let mut retired = Vec::new();
    for index in 0..20 {
        retired.extend(pdf.touch_page(index));
        let cancelled = Arc::new(AtomicBool::new(false));
        pdf.pending.insert(index, cancelled.clone());
        flags.push(cancelled);
        pdf.pages.insert(
            index,
            Arc::new(RenderImage::new(vec![image::Frame::new(
                image::RgbaImage::new(4, 3),
            )])),
        );
        assert!(pdf.pages.len() <= MAX_PAGES);
        assert!(pdf.pending.len() <= MAX_PAGES);
    }
    assert_eq!(retired.len(), 20 - MAX_PAGES);
    assert!(flags[..20 - MAX_PAGES]
        .iter()
        .all(|cancelled| cancelled.load(Ordering::Acquire)));
    assert!(flags[20 - MAX_PAGES..]
        .iter()
        .all(|cancelled| !cancelled.load(Ordering::Acquire)));
    let probe = Arc::downgrade(&retired[0]);
    drop(retired);
    assert!(probe.upgrade().is_none());
}

#[::core::prelude::v1::test]
fn an_obsolete_page_result_cannot_remove_its_replacement_request() {
    let mut pdf = document();
    let old = Arc::new(AtomicBool::new(false));
    pdf.pending.insert(3, old.clone());
    pdf.cancel_page(3);
    let current = Arc::new(AtomicBool::new(false));
    pdf.pending.insert(3, current.clone());
    assert!(old.load(Ordering::Acquire));
    assert!(!pdf.finish_request(3, &old));
    assert!(Arc::ptr_eq(pdf.pending.get(&3).unwrap(), &current));
    assert!(!current.load(Ordering::Acquire));
    assert!(pdf.finish_request(3, &current));
    assert!(!pdf.pending.contains_key(&3));
}

#[cfg(target_os = "windows")]
#[path = "native_pdf_memory.rs"]
mod native;

#[cfg(target_os = "windows")]
#[path = "native_pdf_deletion.rs"]
mod deletion;
