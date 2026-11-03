use super::speed;
use gpui::px;

#[test]
fn speed_grows_with_distance_and_is_capped() {
    assert_eq!(speed(px(-4.0)), px(-3.0));
    assert_eq!(speed(px(40.0)), px(12.0));
    assert_eq!(speed(px(1000.0)), speed(px(160.0)));
}
