use super::*;

/// Red pages of `(width, height, rotation)` in points.
pub(super) fn write_pdf(path: &Path, pages: &[(u32, u32, u32)]) {
    let kids: Vec<String> = (0..pages.len()).map(|index| format!("{} 0 R", 3 + index * 2)).collect();
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), pages.len()),
    ];
    for (index, (width, height, rotation)) in pages.iter().enumerate() {
        let content = format!("1 0 0 rg 0 0 {width} {height} re f\n");
        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Rotate {rotation} /Resources << >> /Contents {} 0 R >>", 4 + index * 2));
        objects.push(format!("<< /Length {} >>\nstream\n{content}endstream", content.len()));
    }
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
    std::fs::write(path, pdf).unwrap();
}

fn document(pages: &[(u32, u32, u32)]) -> (tempfile::TempDir, Document) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("pages.pdf");
    write_pdf(&path, pages);
    let document = Document::open(&path).unwrap();
    (directory, document)
}

#[test]
fn pages_render_as_premultiplied_bgra_within_the_requested_side() {
    let (_directory, document) = document(&[(72, 144, 0), (144, 72, 0)]);
    let first = document.render(0, 1024).unwrap();
    assert_eq!(first.dimensions(), (512, 1024));
    // Red is B, G, R, A in the UI's byte order.
    assert_eq!(first.get_pixel(256, 512).0, [0, 0, 255, 255]);
    assert_eq!(document.render(1, 128).unwrap().dimensions(), (128, 64));
}

#[test]
fn sizes_and_pixels_follow_page_rotation() {
    let (_directory, document) = document(&[(72, 144, 90)]);
    assert_eq!(document.sizes().unwrap(), "144 72\n");
    let page = document.render(0, 128).unwrap();
    assert_eq!(page.dimensions(), (128, 64));
    assert_eq!(page.get_pixel(64, 32).0, [0, 0, 255, 255]);
}

#[test]
fn a_page_beyond_the_document_is_an_error() {
    let (_directory, document) = document(&[(72, 72, 0)]);
    assert!(document.render(1, 128).is_err());
    assert!(document.render(CLOSE_DOCUMENT, 128).is_err());
}

#[test]
fn preview_modes_render_the_first_page_at_their_side() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("modes.pdf");
    write_pdf(&path, &[(200, 100, 0), (50, 50, 0)]);
    for (mode, size) in [("pdf", (1024, 512)), ("pdf-grid", (128, 64))] {
        let Reply::Images { frames, .. } = decode(mode, &path) else { panic!("{mode} gave no image") };
        assert_eq!(frames[0].buffer().dimensions(), size);
    }
    assert!(matches!(decode("image", &path), Reply::Error { .. }));
}

#[test]
fn a_file_that_is_not_a_pdf_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fake.pdf");
    std::fs::write(&path, b"%PDF-1.4\nnot really").unwrap();
    assert!(Document::open(&path).is_err());
    assert!(Document::open(&directory.path().join("missing.pdf")).is_err());
}

#[test]
fn extreme_page_shapes_keep_at_least_one_pixel() {
    assert_eq!(fit_within(1.0, 100_000.0, 128).unwrap(), (1, 128));
    assert!(fit_within(0.0, 10.0, 128).is_err());
    assert!(fit_within(f64::NAN, 10.0, 128).is_err());
}
