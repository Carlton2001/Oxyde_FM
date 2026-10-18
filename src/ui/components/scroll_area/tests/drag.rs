use super::drag_offset;
use crate::ui::scale::set_reference;
use super::{ScrollAreaState, ScrollbarAxis};
use gpui::{point, px};

#[test]
fn grip_drag_reaches_both_ends_at_each_scale() {
    for reference in [10, 16, 24, 32] {
        set_reference(reference);
        let factor = reference as f32 / 16.0;
        let viewport = 200.0 * factor;
        let content = 800.0 * factor;
        let grab = 10.0 * factor;
        assert_eq!(drag_offset(viewport, content, -100.0, grab), 0.0);
        assert_eq!(drag_offset(viewport, content, grab, grab), 0.0);
        assert_eq!(drag_offset(viewport, content, 160.0 * factor, grab), -600.0 * factor);
        assert_eq!(drag_offset(viewport, content, 1000.0 * factor, grab), -600.0 * factor);
    }
    set_reference(16);
}

#[test]
fn dragging_uses_the_owner_origin_and_preserves_the_other_axis() {
    set_reference(16);
    let mut state = ScrollAreaState::default();
    state.scroll.set_offset(point(px(-42.0), px(-60.0)));
    state.origins.insert("rail".into(), point(300.0, 500.0));
    state.grab = 10.0;
    state.drag = Some((ScrollbarAxis::Vertical, "rail".into(), 200.0, 800.0));
    state.move_drag(point(px(900.0), px(660.0)));
    assert_eq!(state.scroll.offset(), point(px(-42.0), px(-600.0)));
    state.drag = Some((ScrollbarAxis::Horizontal, "rail".into(), 200.0, 800.0));
    state.move_drag(point(px(310.0), px(900.0)));
    assert_eq!(state.scroll.offset(), point(px(0.0), px(-600.0)));
}
