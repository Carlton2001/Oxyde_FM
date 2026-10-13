use super::fit_media_size;
use gpui::{px, size};

#[test]
fn landscape_image_uses_tauri_height_limit() {
    let fitted = fit_media_size(1024.0, 576.0, px(774.0), px(200.0));
    assert!((fitted.width.as_f32() - 355.55557).abs() < 0.001);
    assert_eq!(fitted.height, px(200.0));
}

#[test]
fn portrait_pdf_uses_tauri_height_limit() {
    let fitted = fit_media_size(724.0, 1024.0, px(774.0), px(300.0));
    assert_eq!(fitted.height, px(300.0));
    assert!((fitted.width.as_f32() - 212.10938).abs() < 0.001);
}

#[test]
fn small_source_is_not_enlarged_and_wide_source_is_bounded() {
    assert_eq!(fit_media_size(80.0, 40.0, px(774.0), px(200.0)), size(px(80.0), px(40.0)));
    assert_eq!(fit_media_size(2000.0, 100.0, px(774.0), px(200.0)), size(px(774.0), px(38.7)));
}
