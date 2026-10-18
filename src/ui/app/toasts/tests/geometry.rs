use super::notification_height;
use gpui::px;

#[test]
fn notification_stack_leaves_title_and_status_visible_at_every_scale() {
    for rem in [10.0, 16.0, 24.0, 32.0] {
        for height in [0.0, 100.0, 400.0, 768.0, 2160.0] {
            let available = notification_height(px(height), px(rem));
            assert!(available >= px(0.0) && available <= px(height));
            if height >= 5.75 * rem {
                assert_eq!(available + px(5.75 * rem), px(height));
            } else {
                assert_eq!(available, px(0.0));
            }
        }
    }
}
