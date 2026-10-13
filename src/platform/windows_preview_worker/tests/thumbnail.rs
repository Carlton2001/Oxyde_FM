use super::run_image_mode_in;
use crate::utils::preview_protocol::{decode, Reply};
use std::time::Duration;

#[test]
fn isolated_thumbnail_worker_outputs_128_pixels_directly() {
    let executable = std::env::current_exe().unwrap().parent().unwrap()
        .parent().unwrap().join("oxyde-image-worker.exe");
    assert!(executable.is_file(), "Build oxyde-image-worker before this test");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.png");
    image::RgbaImage::from_pixel(1024, 576, image::Rgba([200, 30, 10, 128]))
        .save(&source).unwrap();
    let reply = run_image_mode_in(&executable, &source, Duration::from_secs(15), true).unwrap();
    let Reply::Images { frames, .. } = decode(&reply).unwrap() else { panic!("Expected thumbnail pixels"); };
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].buffer().dimensions(), (128, 72));
    assert_eq!(frames[0].buffer().get_pixel(0, 0).0, [10, 30, 200, 128]);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}
