//! Separate executable: allocator counters must not include other test suites.
use oxyde_lib::commands::search::{advanced_search, AdvancedSearchOptions, QuickSearchEvent};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    fs::File,
    io::{BufRead, BufReader, Write},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};

struct MeasuredAllocator;
static ENABLED: AtomicBool = AtomicBool::new(false);
static LARGEST: AtomicUsize = AtomicUsize::new(0);
static TOTAL: AtomicUsize = AtomicUsize::new(0);
fn record(size: usize) {
    if ENABLED.load(Ordering::Relaxed) {
        LARGEST.fetch_max(size, Ordering::Relaxed);
        TOTAL.fetch_add(size, Ordering::Relaxed);
    }
}
unsafe impl GlobalAlloc for MeasuredAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        System.alloc(layout)
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        System.alloc_zeroed(layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(size);
        System.realloc(ptr, layout, size)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}
#[global_allocator]
static ALLOCATOR: MeasuredAllocator = MeasuredAllocator;

fn measure<T>(run: impl FnOnce() -> T) -> (T, usize, usize, std::time::Duration) {
    struct Stop;
    impl Drop for Stop {
        fn drop(&mut self) {
            ENABLED.store(false, Ordering::SeqCst);
        }
    }
    LARGEST.store(0, Ordering::SeqCst);
    TOTAL.store(0, Ordering::SeqCst);
    ENABLED.store(true, Ordering::SeqCst);
    let stop = Stop;
    let started = Instant::now();
    let result = run();
    drop(stop);
    (
        result,
        LARGEST.load(Ordering::SeqCst),
        TOTAL.load(Ordering::SeqCst),
        started.elapsed(),
    )
}

#[test]
fn advanced_search_allocations_remain_bounded_as_a_line_grows() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("minified.txt");
    let chunk = [b'x'; 64 * 1024];
    for (megabytes, accents, combining) in [
        (1, false, false),
        (16, false, false),
        (64, false, false),
        (16, true, false),
        (8, true, true),
    ] {
        {
            let mut file = File::create(&path).unwrap();
            let mut marks = [0; 64 * 1024];
            for item in marks.chunks_exact_mut(4) {
                item.copy_from_slice("\u{05b1}\u{05b0}".as_bytes());
            }
            for _ in 0..megabytes * 16 {
                file.write_all(if combining { &marks } else { &chunk })
                    .unwrap();
            }
        }
        if megabytes == 16 && !accents {
            // Calibrate against the old line collector on this exact fixture.
            let (_, maximum, _, elapsed) = measure(|| {
                assert_eq!(
                    BufReader::new(File::open(&path).unwrap())
                        .lines()
                        .next()
                        .unwrap()
                        .unwrap()
                        .len(),
                    megabytes * 1024 * 1024
                );
            });
            println!(
                "old collector: size={megabytes} MiB, largest={maximum} bytes, elapsed={elapsed:?}"
            );
            assert!(maximum >= megabytes * 1024 * 1024);
        }
        let (matches, maximum, total, elapsed) = measure(|| {
            let receiver = advanced_search(
                root.path().to_string_lossy().into_owned(),
                AdvancedSearchOptions {
                    content: "ABSENT_MARKER".into(),
                    ignore_accents: accents,
                    ..Default::default()
                },
                100,
                true,
                true,
                Arc::new(AtomicBool::new(false)),
            );
            let mut matches = 0;
            let mut finished = false;
            for event in receiver {
                match event {
                    QuickSearchEvent::Batch(entries) => matches += entries.len(),
                    QuickSearchEvent::Finished { .. } => finished = true,
                    QuickSearchEvent::Failed(error) => panic!("{error}"),
                    QuickSearchEvent::ArchiveSkipped { .. } | QuickSearchEvent::ContentSkipped { .. } => {
                        panic!("ordinary text cannot be an archive warning")
                    }
                }
            }
            assert!(finished);
            matches
        });
        println!("streaming search: size={megabytes} MiB, accents={accents}, combining={combining}, largest={maximum} bytes, allocated_total={total} bytes, elapsed={elapsed:?}");
        assert_eq!(matches, 0);
        assert!(maximum < 1024 * 1024, "line-sized allocation: {maximum}");
        assert!(
            total < 4 * 1024 * 1024,
            "allocation grows with input: {total}"
        );
        if combining {
            File::options()
                .append(true)
                .open(&path)
                .unwrap()
                .write_all(b"needle")
                .unwrap();
            let receiver = advanced_search(
                root.path().to_string_lossy().into_owned(),
                AdvancedSearchOptions {
                    content: r"\A.*needle\z".into(),
                    content_regex: true,
                    ignore_accents: true,
                    ..Default::default()
                },
                100,
                true,
                true,
                Arc::new(AtomicBool::new(false)),
            );
            let mut paths = Vec::new();
            for event in receiver {
                match event {
                    QuickSearchEvent::Batch(entries) => {
                        paths.extend(entries.into_iter().map(|entry| entry.path))
                    }
                    QuickSearchEvent::Finished { .. } => {}
                    _ => panic!("long normalized file search failed"),
                }
            }
            assert_eq!(paths, vec![path.to_string_lossy().into_owned()]);
        }
    }
}
