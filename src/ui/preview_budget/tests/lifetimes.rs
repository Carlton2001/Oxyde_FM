use super::*;
fn image() -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(2, 1),
    )]))
}
#[test]
fn cache_and_render_clones_count_once_until_the_final_reference_expires() {
    let mut budget = Budget::new(16, 4);
    let first = image();
    let rendered = first.clone();
    budget.admit(&first).unwrap();
    budget.admit(&rendered).unwrap();
    assert_eq!(budget.bytes, 8);
    let second = image();
    budget.admit(&second).unwrap();
    drop(first);
    let third = image();
    assert!(budget.admit(&third).is_err());
    drop(rendered);
    budget.admit(&third).unwrap();
    assert_eq!(budget.bytes, 16);
    drop((second, third));
    budget.admit(&image()).unwrap();
    assert_eq!(budget.bytes, 8);
}
#[test]
fn concurrent_image_families_share_the_same_admission_and_release_dead_entries() {
    let budget = Arc::new(Mutex::new(Budget::new(32, 4)));
    let held = std::sync::Barrier::new(8);
    let admitted = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                let image = image();
                if budget.lock().unwrap().admit(&image).is_ok() {
                    admitted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
                held.wait();
            });
        }
    });
    assert_eq!(admitted.load(std::sync::atomic::Ordering::SeqCst), 4);
    assert!(budget.lock().unwrap().admit(&image()).is_ok());
}
