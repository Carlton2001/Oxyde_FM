use super::aims_right;
use gpui::{point, px};

#[test]
fn only_moves_mostly_rightward_head_for_the_submenu() {
    let from = point(px(100.0), px(100.0));
    assert!(aims_right(from, point(px(110.0), px(104.0))));
    assert!(aims_right(from, point(px(110.0), px(90.0))));
    assert!(!aims_right(from, point(px(104.0), px(110.0))));
    assert!(!aims_right(from, point(px(90.0), px(100.0))));
    assert!(!aims_right(from, from));
}
