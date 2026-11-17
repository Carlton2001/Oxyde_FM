use super::{accepts_click, is_activation_key};

#[test]
fn only_a_single_mouse_click_answers_a_conflict() {
    assert!(accepts_click(false, 1));
    assert!(!accepts_click(false, 2));
    assert!(!accepts_click(false, 3));
}

#[test]
fn keyboard_clicks_are_left_to_the_key_handler() {
    assert!(!accepts_click(true, 1));
}

#[test]
fn enter_and_space_activate_the_focused_choice() {
    assert!(is_activation_key("enter"));
    assert!(is_activation_key("space"));
    assert!(!is_activation_key("escape"));
}
