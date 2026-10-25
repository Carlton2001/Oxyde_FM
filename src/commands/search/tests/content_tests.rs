    use super::*;
    use std::cell::Cell;
    use std::io::Cursor;

    struct CancelAfter<'a, R> {
        inner: R,
        bytes: &'a Cell<usize>,
        cancellation: &'a AtomicBool,
    }
    impl<R: Read> Read for CancelAfter<'_, R> {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let count = self.inner.read(buffer)?;
            self.bytes.set(self.bytes.get() + count);
            if self.bytes.get() >= 8192 {
                self.cancellation.store(true, Ordering::Relaxed);
            }
            Ok(count)
        }
    }

    #[test]
    fn text_cancellation_stops_inside_a_large_line_before_publishing_a_match() {
        let text = format!("needle{}", "x".repeat(2 * 1024 * 1024));
        let cancellation = AtomicBool::new(false);
        let bytes = Cell::new(0);
        assert!(!text_reader_contains_content(
            CancelAfter {
                inner: Cursor::new(text.as_bytes()),
                bytes: &bytes,
                cancellation: &cancellation
            },
            &ContentPattern::new("needle", true).unwrap(),
            false,
            &cancellation,
        ));
        assert!(bytes.get() <= 64 * 1024);
        assert!(text_reader_contains_content(
            Cursor::new("Résumé\n"),
            &ContentPattern::new("(?i)resume", true).unwrap(),
            true,
            &AtomicBool::new(false)
        ));
        assert!(!text_reader_contains_content(
            Cursor::new(b"invalid\xff\nneedle\n"),
            &ContentPattern::new("needle", true).unwrap(),
            false,
            &AtomicBool::new(false)
        ));
    }

    #[test]
    fn waiting_for_disk_lock_can_be_cancelled_without_releasing_the_lock() {
        let lock = Arc::new(Mutex::new(()));
        let guard = lock.lock().unwrap();
        let cancellation = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let worker_lock = lock.clone();
        let worker_cancel = cancellation.clone();
        let worker = thread::spawn(move || {
            ready_tx.send(()).unwrap();
            done_tx
                .send(content_lock(&worker_lock, &worker_cancel).is_none())
                .unwrap();
        });
        ready_rx.recv().unwrap();
        thread::sleep(std::time::Duration::from_millis(20));
        cancellation.store(true, Ordering::Relaxed);
        assert!(done_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap());
        worker.join().unwrap();
        drop(guard);
    }
