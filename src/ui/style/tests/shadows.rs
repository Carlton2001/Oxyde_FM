use super::rem_shadow;
use gpui::px;

#[test]
fn modal_shadow_has_visible_outer_layers() {
    let shadows = rem_shadow(true);
    assert_eq!(shadows.len(), 2);
    for shadow in shadows {
        assert!(shadow.color.a > 0.0);
        assert!(shadow.blur_radius > px(0.0));
        assert!(shadow.offset.y > px(0.0));
        assert!(!shadow.inset);
    }
}
