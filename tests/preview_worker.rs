#![cfg(target_os = "windows")]
mod utils {
    pub use oxyde_lib::utils::preview_protocol;
}
#[allow(dead_code)]
#[path = "../src/platform/windows_preview_worker.rs"]
mod transport;
use oxyde_lib::utils::preview_protocol::{self, Reply};
use std::path::Path;
use std::time::{Duration, Instant};

fn worker(mode: &str, source: &Path) -> Reply {
    let bytes = transport::run_in(
        Path::new(env!("CARGO_BIN_EXE_oxyde")),
        mode,
        source,
        Duration::from_secs(15),
    )
    .unwrap();
    preview_protocol::decode(&bytes).unwrap()
}

#[test]
fn actual_headless_worker_preserves_raster_svg_and_animation_without_preferences_writes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("école.png");
    image::RgbaImage::from_pixel(2, 1, image::Rgba([11, 22, 33, 255]))
        .save(&path)
        .unwrap();
    let Reply::Images { frames, limited } = worker("image", &path) else {
        panic!()
    };
    assert!(!limited);
    assert_eq!(
        frames[0].buffer().as_raw(),
        &[33, 22, 11, 255, 33, 22, 11, 255]
    );
    let path = directory.path().join("école.svg");
    std::fs::write(&path, r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><rect width="2" height="1" fill="red"/></svg>"#).unwrap();
    let Reply::Images { frames, .. } = worker("image", &path) else {
        panic!()
    };
    assert!(frames[0].buffer().width() <= 1024);
    assert_eq!(&frames[0].buffer().as_raw()[..4], &[0, 0, 255, 255]);
    let path = directory.path().join("école.gif");
    let file = std::fs::File::create(&path).unwrap();
    let frames = (0..2).map(|index| {
        image::Frame::from_parts(
            image::RgbaImage::from_pixel(2, 1, image::Rgba([index, 0, 0, 255])),
            0,
            0,
            image::Delay::from_numer_denom_ms(30 + u32::from(index), 1),
        )
    });
    image::codecs::gif::GifEncoder::new(file)
        .encode_frames(frames)
        .unwrap();
    let Reply::Images { frames, limited } = worker("image", &path) else {
        panic!()
    };
    assert!(!limited);
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].delay().numer_denom_ms(), (30, 1));
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 3);
}

#[test]
fn actual_worker_returns_typed_error_for_missing_and_oversized_sources() {
    let directory = tempfile::tempdir().unwrap();
    assert!(matches!(
        worker("image", &directory.path().join("absent.png")),
        Reply::Error { limit: false, .. }
    ));
    let path = directory.path().join("large.png");
    std::fs::File::create(&path)
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    assert!(matches!(
        worker("image", &path),
        Reply::Error { limit: true, .. }
    ));
    assert!(matches!(
        worker("unknown", &path),
        Reply::Error { limit: false, .. }
    ));
}

#[test]
fn actual_worker_renders_pdf_and_reads_office_thumbnail_and_text() {
    use std::io::Write;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("école.pdf");
    let content = "1 0 0 rg 0 0 72 144 re f\n";
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 144] /Resources << >> /Contents 4 0 R >>"
            .to_owned(),
        format!(
            "<< /Length {} >>\nstream\n{content}endstream",
            content.len()
        ),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(b"xref\n0 5\n0000000000 65535 f \n");
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!("trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    std::fs::write(&path, pdf).unwrap();
    let reply = worker("pdf", &path);
    if let Reply::Error { detail, .. } = &reply {
        panic!("PDF worker: {detail}");
    }
    let Reply::Images { frames, .. } = reply else {
        panic!("Unexpected PDF worker reply")
    };
    assert_eq!(frames[0].buffer().dimensions(), (256, 512));
    let pixel = frames[0].buffer().get_pixel(128, 256).0;
    // Native PDF/GPU rasterization at fractional DPI can differ by a few units.
    // Exact channel preservation is independently checked by the PNG fixture.
    assert!(
        pixel[0] <= 2 && pixel[1] <= 2 && pixel[2] >= 250 && pixel[3] >= 250,
        "{pixel:?}"
    );
    std::fs::remove_file(path).unwrap();
    let path = directory.path().join("école.docx");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    zip.start_file(
        "word/document.xml",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all("<w:p><w:t>école</w:t></w:p>".as_bytes())
        .unwrap();
    zip.start_file(
        "docProps/thumbnail.jpeg",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        2,
        1,
        image::Rgba([11, 22, 33, 255]),
    ))
    .write_to(&mut bytes, image::ImageFormat::Jpeg)
    .unwrap();
    zip.write_all(&bytes.into_inner()).unwrap();
    zip.finish().unwrap();
    let Reply::Images { frames, .. } = worker("office", &path) else {
        panic!()
    };
    assert!(frames[0].buffer().width() <= 256);
    let Reply::Text(text) = worker("office-text", &path) else {
        panic!()
    };
    assert!(text.contains("école"));
    std::fs::remove_file(path).unwrap();
}

fn powershell(script: &str) -> std::process::Command {
    let path = oxyde_lib::platform::platform()
        .system_executable(oxyde_lib::platform::SystemExecutable::PowerShell)
        .unwrap();
    let mut command = std::process::Command::new(path);
    command.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    command
}

#[test]
fn supervisor_stops_a_stalled_worker_instead_of_waiting_for_stdout_eof() {
    let started = Instant::now();
    let result = transport::supervise(
        powershell(
            "$g=[Console]::OpenStandardInput().ReadByte(); [Threading.Thread]::Sleep(10000)",
        ),
        Duration::from_secs(2),
    );
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn supervisor_bounds_actual_output_and_reaps_a_failed_process() {
    let output = "$g=[Console]::OpenStandardInput().ReadByte(); $o=[Console]::OpenStandardOutput(); $b=New-Object byte[] 65536; for($i=0;$i -lt 600;$i++){$o.Write($b,0,$b.Length)}";
    let error = transport::supervise(powershell(output), Duration::from_secs(10)).unwrap_err();
    assert!(error.to_string().contains("output exceeds"), "{error}");
    let error = transport::supervise(
        powershell("$g=[Console]::OpenStandardInput().ReadByte(); exit 9"),
        Duration::from_secs(10),
    )
    .unwrap_err();
    assert!(error.to_string().contains("failed"), "{error}");
}
