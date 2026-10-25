//! Exercise the public search and its real Windows LPAC worker, not cfg(test) fallbacks.
#![cfg(target_os = "windows")]
use oxyde_lib::{
    commands::search::{advanced_search, AdvancedSearchOptions, QuickSearchEvent},
    platform::worker_test_support,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    fs::File,
    io::Write,
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

struct Meter;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            PEAK.fetch_max(
                LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size(),
                Ordering::Relaxed,
            );
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(pointer, layout) };
    }
}
#[global_allocator]
static ALLOCATOR: Meter = Meter;

// Production deliberately resolves its worker beside the calling executable.
// Stage a private caller/worker pair instead of adding a production path override.
fn isolated(test: &str) -> bool {
    const INNER: &str = "OXYDE_OFFICE_SEARCH_TEST_INNER";
    if std::env::var(INNER).as_deref() == Ok(test) {
        return true;
    }
    use std::os::windows::process::CommandExt;
    let directory = tempfile::tempdir().unwrap();
    let caller = directory.path().join("office-search-tests.exe");
    let copy = |source: &Path, destination: &Path| {
        if std::fs::hard_link(source, destination).is_err() {
            std::fs::copy(source, destination).unwrap();
        }
    };
    copy(&std::env::current_exe().unwrap(), &caller);
    copy(
        Path::new(env!("CARGO_BIN_EXE_oxyde-image-worker")),
        &directory.path().join("oxyde-image-worker.exe"),
    );
    let mut command = std::process::Command::new(caller);
    command
        .args(["--exact", test, "--nocapture"])
        .env(INNER, test)
        .creation_flags(0x08000000)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let output = worker_test_support::spawn(command)
        .unwrap()
        .wait_with_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("running 1 test"));
    assert!(output.status.success(), "{stdout}\n{stderr}");
    if !stderr.is_empty() {
        eprintln!("{stderr}");
    }
    false
}

fn package(path: &Path, entries: &[(&str, &str)]) {
    let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
    for (name, text) in entries {
        writer
            .start_file(
                *name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
        writer.write_all(text.as_bytes()).unwrap();
    }
    writer.finish().unwrap();
}

fn find(root: &Path, options: AdvancedSearchOptions) -> (Vec<String>, Vec<(String, String)>) {
    let events = advanced_search(
        root.to_string_lossy().into_owned(),
        options,
        100,
        true,
        true,
        Arc::new(AtomicBool::new(false)),
    );
    let (mut matches, mut warnings) = (Vec::new(), Vec::new());
    loop {
        match events.recv_timeout(Duration::from_secs(30)).unwrap() {
            QuickSearchEvent::Batch(batch) => {
                matches.extend(batch.into_iter().map(|file| file.name))
            }
            QuickSearchEvent::ContentSkipped { path, detail } => {
                assert!(!detail.is_empty());
                warnings.push((path, detail));
            }
            QuickSearchEvent::Finished { limited } => {
                assert!(!limited);
                matches.sort();
                return (matches, warnings);
            }
            QuickSearchEvent::Failed(error) => panic!("Search failed: {error}"),
            QuickSearchEvent::ArchiveSkipped { detail, .. } => {
                panic!("Unexpected archive warning: {detail}")
            }
        }
    }
}
fn options(content: &str) -> AdvancedSearchOptions {
    AdvancedSearchOptions {
        content: content.into(),
        ..Default::default()
    }
}

#[test]
fn public_office_search_finds_later_slides_inline_cells_and_decoded_entities() {
    if !isolated("public_office_search_finds_later_slides_inline_cells_and_decoded_entities") {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    package(
        &root.path().join("slides.pptx"),
        &[
            ("ppt/slides/slide1.xml", "<a:p><a:t>first</a:t></a:p>"),
            ("ppt/slides/slide4.xml", "<a:p><a:t>needle</a:t></a:p>"),
        ],
    );
    package(&root.path().join("inline.xlsx"), &[("xl/worksheets/sheet1.xml", "<worksheet><row><c r=\"ZZ999\" t=\"inlineStr\"><is><t>needle</t></is></c></row></worksheet>")]);
    let (names, warnings) = find(root.path(), options("needle"));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(names, ["inline.xlsx", "slides.pptx"]);
    package(
        &root.path().join("entity.docx"),
        &[(
            "word/document.xml",
            "<w:p><w:r><w:t>Résumé A&amp;</w:t></w:r><w:r><w:t>B</w:t></w:r></w:p>",
        )],
    );
    let (names, warnings) = find(
        root.path(),
        AdvancedSearchOptions {
            content: "^resume A&B$".into(),
            content_regex: true,
            ignore_accents: true,
            ..Default::default()
        },
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(names, ["entity.docx"]);
    assert!(find(root.path(), options("A&amp;B")).0.is_empty());
}

#[test]
fn office_search_parent_memory_stays_bounded_and_limits_are_reported() {
    if !isolated("office_search_parent_memory_stays_bounded_and_limits_are_reported") {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("compressed.docx");
    let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
    writer
        .start_file(
            "word/document.xml",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
    writer.write_all(b"<w:p><w:t>").unwrap();
    for _ in 0..512 {
        writer.write_all(&[b'x'; 64 * 1024]).unwrap();
    }
    writer.write_all(b"needle</w:t></w:p>").unwrap();
    writer.finish().unwrap();
    let baseline = LIVE.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    let (matches, warnings) = find(root.path(), options("needle"));
    let extra = PEAK.load(Ordering::SeqCst).saturating_sub(baseline);
    assert!(matches.is_empty());
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].0, path.to_string_lossy());
    assert!(
        warnings[0]
            .1
            .contains("Office content search limit reached"),
        "{warnings:?}"
    );
    assert!(
        extra < 16 * 1024 * 1024,
        "Office parsing leaked into the parent: {extra} extra bytes"
    );
    eprintln!("Office parent extra peak live Rust allocations: {extra} bytes");
}

#[test]
fn office_search_refuses_missing_workers_and_observes_cancellation_and_deadline() {
    if !isolated("office_search_refuses_missing_workers_and_observes_cancellation_and_deadline") {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source.docx");
    package(
        &path,
        &[(
            "word/document.xml",
            "<w:p><w:t>positive control</w:t></w:p>",
        )],
    );
    let executable = Path::new(env!("CARGO_BIN_EXE_oxyde-image-worker"));
    let run = worker_test_support::preview::run_office_search_in;
    assert_eq!(
        run(executable, &path, Duration::from_secs(15), &|| Ok(())).unwrap(),
        "positive control\n"
    );
    assert!(run(
        &root.path().join("missing.exe"),
        &path,
        Duration::from_secs(15),
        &|| Ok(())
    )
    .is_err());
    assert_eq!(
        run(executable, &path, Duration::from_secs(15), &|| Err(
            std::io::ErrorKind::Interrupted.into()
        ))
        .unwrap_err()
        .kind(),
        std::io::ErrorKind::Interrupted
    );
    assert_eq!(
        run(executable, &path, Duration::ZERO, &|| Ok(()))
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::TimedOut
    );
    assert_eq!(
        find(root.path(), options("positive control")).0,
        ["source.docx"]
    );
}
