use super::*;
use std::sync::mpsc;

#[derive(Clone, Default)]
struct Blocker(Arc<(Mutex<bool>, Condvar)>);
impl Blocker {
    fn wait(&self) {
        let (lock, changed) = &*self.0;
        let state = lock.lock().unwrap();
        drop(changed.wait_while(state, |released| !*released).unwrap());
    }
    fn release(&self) {
        let (lock, changed) = &*self.0;
        *lock.lock().unwrap() = true;
        changed.notify_all();
    }
}
struct Release(Blocker);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
    }
}
struct Stop(Writer);
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.shutdown(Duration::from_secs(2));
    }
}
fn image() -> RgbaImage {
    RgbaImage::from_pixel(1, 1, image::Rgba([12, 34, 56, 128]))
}
fn wait_for(writer: &Writer, ready: impl Fn(&State) -> bool) {
    let state = writer.0.state.lock().unwrap();
    let (state, _) = writer
        .0
        .changed
        .wait_timeout_while(state, Duration::from_secs(2), |state| !ready(state))
        .unwrap();
    assert!(ready(&state), "Writer did not reach the expected state");
}

#[test]
fn enqueue_never_waits_for_the_active_publication_and_bounds_pending_pixels() {
    for (entries, bytes, accepted) in [(2, 100, 2), (10, 4, 1)] {
        let blocker = Blocker::default();
        let gate = blocker.clone();
        let (sent, received) = mpsc::channel();
        let writer = Writer::start(entries, bytes, move |job| {
            sent.send(job.key).unwrap();
            gate.wait();
        })
        .unwrap();
        let _stop = Stop(writer.clone());
        let _release = Release(blocker.clone());
        let revision = writer.revision().unwrap();
        assert!(writer.enqueue(revision, Path::new("cache"), "active", &image()));
        assert_eq!(
            received.recv_timeout(Duration::from_secs(2)).unwrap(),
            "active"
        );
        for index in 0..accepted {
            assert!(writer.enqueue(revision, Path::new("cache"), &index.to_string(), &image()));
        }
        assert!(!writer.enqueue(revision, Path::new("cache"), "overflow", &image()));
        let state = writer.0.state.lock().unwrap();
        assert_eq!(state.pending.len(), accepted);
        assert_eq!(state.bytes, accepted * 4);
        drop(state);
        blocker.release();
        wait_for(&writer, |state| !state.active && state.pending.is_empty());
    }
}

#[test]
fn invalid_dimensions_and_images_over_the_byte_budget_are_not_queued() {
    let writer = Writer::start(4, 4, |_| panic!("Rejected image must not be published")).unwrap();
    let _stop = Stop(writer.clone());
    let revision = writer.revision().unwrap();
    for (width, height) in [(0, 1), (1, 0), (257, 1), (1, 257), (2, 1)] {
        assert!(!writer.enqueue(
            revision,
            Path::new("cache"),
            "invalid",
            &RgbaImage::new(width, height)
        ));
    }
    assert_eq!(writer.0.state.lock().unwrap().bytes, 0);
}

#[test]
fn purge_waits_for_active_write_drops_pending_work_and_rejects_old_revisions() {
    let blocker = Blocker::default();
    let gate = blocker.clone();
    let (sent, received) = mpsc::channel();
    let writer = Writer::start(4, 100, move |job| {
        sent.send(job.key).unwrap();
        gate.wait();
    })
    .unwrap();
    let _stop = Stop(writer.clone());
    let _release = Release(blocker.clone());
    let revision = writer.revision().unwrap();
    assert!(writer.enqueue(revision, Path::new("cache"), "active", &image()));
    assert_eq!(
        received.recv_timeout(Duration::from_secs(2)).unwrap(),
        "active"
    );
    assert!(writer.enqueue(revision, Path::new("cache"), "obsolete", &image()));
    let pausing = writer.clone();
    let (paused, pause_complete) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        paused.send(pausing.pause()).unwrap();
    });
    wait_for(&writer, |state| state.pauses == 1);
    assert!(matches!(
        pause_complete.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert_eq!(writer.revision(), None);
    assert!(!writer.enqueue(revision, Path::new("cache"), "late", &image()));
    blocker.release();
    let pause = pause_complete.recv_timeout(Duration::from_secs(2)).unwrap();
    thread.join().unwrap();
    {
        let state = writer.0.state.lock().unwrap();
        assert!(state.pending.is_empty());
        assert_eq!(state.bytes, 0);
    }
    assert!(matches!(
        received.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    drop(pause);
    assert!(!writer.enqueue(revision, Path::new("cache"), "old-flight", &image()));
    assert!(writer.enqueue(
        writer.revision().unwrap(),
        Path::new("cache"),
        "fresh",
        &image()
    ));
    assert_eq!(
        received.recv_timeout(Duration::from_secs(2)).unwrap(),
        "fresh"
    );
}

#[test]
fn nested_pause_keeps_publication_disabled_until_all_cleanup_guards_drop() {
    let writer = Writer::start(4, 100, |_| {}).unwrap();
    let _stop = Stop(writer.clone());
    let first = writer.pause();
    let second = writer.pause();
    drop(first);
    assert_eq!(writer.revision(), None);
    drop(second);
    assert!(writer.revision().is_some());
}

#[test]
fn shutdown_is_bounded_drops_pending_work_and_does_not_restart() {
    let blocker = Blocker::default();
    let gate = blocker.clone();
    let (sent, received) = mpsc::channel();
    let writer = Writer::start(4, 100, move |job| {
        sent.send(job.key).unwrap();
        gate.wait();
    })
    .unwrap();
    let _stop = Stop(writer.clone());
    let _release = Release(blocker.clone());
    let revision = writer.revision().unwrap();
    assert!(writer.enqueue(revision, Path::new("cache"), "active", &image()));
    received.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(writer.enqueue(revision, Path::new("cache"), "pending", &image()));
    assert!(!writer.shutdown(Duration::from_millis(10)));
    assert!(!writer.enqueue(revision, Path::new("cache"), "late", &image()));
    assert_eq!(writer.revision(), None);
    blocker.release();
    assert!(writer.shutdown(Duration::from_secs(2)));
    assert!(matches!(
        received.try_recv(),
        Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected)
    ));
}

#[test]
fn publication_panic_releases_purge_and_shutdown_waiters() {
    let writer = Writer::start(4, 100, |_| panic!("injected writer panic")).unwrap();
    let _stop = Stop(writer.clone());
    assert!(writer.enqueue(
        writer.revision().unwrap(),
        Path::new("cache"),
        "panic",
        &image()
    ));
    wait_for(&writer, |state| state.done);
    drop(writer.pause());
    assert!(writer.shutdown(Duration::from_secs(2)));
}

#[test]
fn deferred_publication_preserves_pixels_and_cleanup_cannot_recreate_old_files() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("icons");
    let (sent, completed) = mpsc::channel();
    let writer = Writer::start(4, 100, move |job| {
        sent.send(super::super::windows_icon_cache::write_cached_png(
            &job.directory,
            &job.key,
            &job.image,
        ))
        .unwrap();
    })
    .unwrap();
    let _stop = Stop(writer.clone());
    let revision = writer.revision().unwrap();
    assert!(writer.enqueue(revision, &directory, "entry", &image()));
    completed
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    assert_eq!(
        super::super::windows_icon_cache::read_cached_png(&directory.join("entry.png")).unwrap(),
        image()
    );
    let pause = writer.pause();
    std::fs::remove_dir_all(&directory).unwrap();
    assert!(!writer.enqueue(revision, &directory, "late", &image()));
    drop(pause);
    assert!(!writer.enqueue(revision, &directory, "late", &image()));
    assert!(!directory.exists());
}
