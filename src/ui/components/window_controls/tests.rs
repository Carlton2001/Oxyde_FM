use super::{Side, buttons};
use gpui::{WindowButton, WindowButtonLayout};

const MIN: Option<WindowButton> = Some(WindowButton::Minimize);
const MAX: Option<WindowButton> = Some(WindowButton::Maximize);
const CLOSE: Option<WindowButton> = Some(WindowButton::Close);

#[test]
fn without_a_preference_the_usual_three_show_on_the_right() {
    assert_eq!(buttons(None, Side::Right, true), [WindowButton::Minimize, WindowButton::Maximize, WindowButton::Close]);
    assert!(buttons(None, Side::Left, true).is_empty());
}

#[test]
fn each_side_keeps_the_desktop_choice() {
    let layout = WindowButtonLayout { left: [CLOSE, None, None], right: [MIN, None, None] };
    assert_eq!(buttons(Some(layout), Side::Left, true), [WindowButton::Close]);
    assert_eq!(buttons(Some(layout), Side::Right, true), [WindowButton::Minimize]);
}

#[test]
fn fixed_windows_only_keep_close() {
    let layout = WindowButtonLayout { left: [None; 3], right: [MIN, MAX, CLOSE] };
    assert_eq!(buttons(Some(layout), Side::Right, false), [WindowButton::Close]);
}
