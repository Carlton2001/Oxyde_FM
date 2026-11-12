use crate::ui::scale::set_reference;
use super::{ScrollAreaState, ScrollbarAxis};
use gpui::{point, px};

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
