use super::super::super::PdfSession;
use super::super::tests::write_pdf;
use super::*;

/// The application binary, which serves as its own worker.
fn oxyde() -> PathBuf {
    let executable = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().join("oxyde");
    assert!(executable.is_file(), "Build oxyde before worker tests: cargo build --offline --bins");
    executable
}

#[test]
fn session_keeps_the_document_open_across_page_requests() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("two.pdf");
    write_pdf(&path, &[(72, 144, 0), (144, 72, 0)]);

    let mut session = Session::open_in(&oxyde(), &path).unwrap();
    assert_eq!(session.sizes(), [(72.0, 144.0), (144.0, 72.0)]);
    // The document stays open even once its file is gone.
    std::fs::remove_file(&path).unwrap();
    let last = session.page(1).unwrap();
    assert!(last.width() > last.height());
    assert!(session.page(2).is_err());
    assert!(session.page(0).is_ok(), "A page error must not close the session");
    session.close().unwrap();
    assert!(session.page(0).is_err());
}

#[test]
fn thumbnails_come_back_at_their_side() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("wide.pdf");
    write_pdf(&path, &[(200, 100, 0)]);
    assert_eq!(thumbnail_in(&oxyde(), &path, true).unwrap().dimensions(), (128, 64));
    assert_eq!(thumbnail_in(&oxyde(), &path, false).unwrap().dimensions(), (1024, 512));
}

#[test]
fn a_damaged_document_is_an_error_from_the_worker() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("damaged.pdf");
    std::fs::write(&path, b"%PDF-1.7\n1 0 obj << /Type /Catalog").unwrap();
    assert!(thumbnail_in(&oxyde(), &path, true).is_err());
    assert!(Session::open_in(&oxyde(), &path).is_err());
}
