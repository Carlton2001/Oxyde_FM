use super::visible_size;
use gpui::{px, size};

#[test]
fn hidden_empty_layout_does_not_prevent_first_native_show() {
    for measured in [
        None,
        Some(size(px(0.0), px(0.0))),
        Some(size(px(424.0), px(0.0))),
        Some(size(px(f32::NAN), px(120.0))),
    ] {
        assert_eq!(visible_size(measured, px(16.0)), size(px(424.0), px(128.0)));
    }
    assert_eq!(visible_size(None, px(24.0)), size(px(636.0), px(192.0)));
    let content = size(px(350.0), px(460.0));
    assert_eq!(visible_size(Some(content), px(16.0)), content);
}
