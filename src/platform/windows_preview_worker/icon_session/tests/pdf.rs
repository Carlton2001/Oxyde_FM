use super::*;

fn executable() -> std::path::PathBuf {
    let path = std::path::PathBuf::from(
        std::env::var_os("OXYDE_TEST_PDF_WORKER").expect("set OXYDE_TEST_PDF_WORKER"),
    );
    assert!(
        path.is_file(),
        "Build oxyde before native PDF session tests"
    );
    path
}

fn pdf(path: &Path, width: u32, height: u32) {
    let content = format!("1 0 0 rg 0 0 {width} {height} re f\n");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Resources << >> /Contents 4 0 R >>"),
        format!("<< /Length {} >>\nstream\n{content}endstream", content.len()),
    ];
    let mut bytes = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = bytes.len();
    bytes.extend_from_slice(b"xref\n0 5\n0000000000 65535 f \n");
    for offset in offsets {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(
        format!("trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    std::fs::write(path, bytes).unwrap();
}

fn pixels(bytes: &[u8]) -> image::RgbaImage {
    let crate::utils::preview_protocol::Reply::Images { mut frames, .. } =
        crate::utils::preview_protocol::decode(bytes).unwrap()
    else {
        panic!("Expected PDF pixels");
    };
    assert_eq!(frames.len(), 1);
    frames.remove(0).into_buffer()
}

#[test]
#[ignore = "requires OXYDE_TEST_PDF_WORKER pointing to the freshly built oxyde executable"]
fn pdf_preview_session_reuses_pid_across_files_and_renders_grid_directly_at_128px() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("first.pdf");
    let second = root.path().join("second.pdf");
    pdf(&first, 200, 100);
    pdf(&second, 100, 200);
    let cold = Instant::now();
    let mut session = Session::start_kind(&executable(), cold, Kind::Pdf).unwrap();
    let pid = session.worker.child.id();
    let grid = session
        .request(first.to_str().unwrap(), false, Instant::now())
        .unwrap();
    let cold = cold.elapsed();
    assert!(grid.len() < 128 * 128 * 4 + 128);
    let grid = pixels(&grid);
    assert_eq!(grid.width().max(grid.height()), 128);
    assert!(grid.width() > grid.height());
    assert_eq!(
        grid.get_pixel(grid.width() / 2, grid.height() / 2).0,
        [0, 0, 255, 255]
    );
    let warm = Instant::now();
    let hover = pixels(
        &session
            .request(second.to_str().unwrap(), true, Instant::now())
            .unwrap(),
    );
    let warm = warm.elapsed();
    assert_eq!(hover.width().max(hover.height()), 1024);
    assert!(hover.height() > hover.width());
    assert_eq!(session.worker.child.id(), pid);
    assert!(session.reusable(100));
    eprintln!("PDF previews: cold grid {cold:?}, warm hover {warm:?}, same PID {pid}");
    std::fs::remove_file(first).unwrap();
    std::fs::remove_file(second).unwrap();
}

#[test]
#[ignore = "requires OXYDE_TEST_PDF_WORKER pointing to the freshly built oxyde executable"]
fn pdf_preview_session_bounds_reuse_deadlines_and_reaps_its_process() {
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use windows::Win32::{
        Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS, WAIT_OBJECT_0},
        System::Threading::{GetCurrentProcess, WaitForSingleObject},
    };
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("a.pdf");
    pdf(&path, 100, 100);
    let mut session = Session::start_kind(&executable(), Instant::now(), Kind::Pdf).unwrap();
    assert!(!session.reusable(MAX_INPUT + 1));
    assert!(session
        .request(path.to_str().unwrap(), false, Instant::now() - DEADLINE)
        .is_err());
    session.created = Instant::now() - Duration::from_secs(61);
    assert!(!session.reusable(0));
    session.created = Instant::now();
    session.last_used = Instant::now() - IDLE;
    assert!(!session.reusable(0));
    session.last_used = Instant::now();
    let request_bytes = path.to_str().unwrap().len() + 5;
    let requests = (MAX_INPUT / request_bytes).min(32);
    assert!(requests > 0);
    for _ in 0..requests {
        session
            .request(path.to_str().unwrap(), false, Instant::now())
            .unwrap();
    }
    assert!(!session.reusable(request_bytes));
    let mut handle = HANDLE::default();
    unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            HANDLE(session.worker.child.as_raw_handle()),
            GetCurrentProcess(),
            &mut handle,
            0,
            false,
            DUPLICATE_SAME_ACCESS,
        )
    }
    .unwrap();
    let held = unsafe { OwnedHandle::from_raw_handle(handle.0) };
    drop(session);
    assert_eq!(
        unsafe { WaitForSingleObject(HANDLE(held.as_raw_handle()), 2000) },
        WAIT_OBJECT_0
    );
}

#[test]
#[ignore = "requires OXYDE_TEST_PDF_WORKER pointing to the freshly built oxyde executable"]
fn pdf_preview_session_rejects_invalid_documents_and_malformed_request_lengths() {
    use windows::Win32::{Foundation::WAIT_OBJECT_0, System::Threading::WaitForSingleObject};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("invalid.pdf");
    std::fs::write(&path, b"not a PDF").unwrap();
    let mut session = Session::start_kind(&executable(), Instant::now(), Kind::Pdf).unwrap();
    assert!(session
        .request(path.to_str().unwrap(), false, Instant::now())
        .is_err());
    drop(session);
    let mut session = Session::start_kind(&executable(), Instant::now(), Kind::Pdf).unwrap();
    let mut packet = [0u8; 5];
    packet[1..].copy_from_slice(&(MAX_INPUT as u32 + 1).to_le_bytes());
    session.input.write_all(&packet).unwrap();
    assert_eq!(
        unsafe { WaitForSingleObject(HANDLE(session.worker.child.as_raw_handle()), 2000) },
        WAIT_OBJECT_0
    );
}
