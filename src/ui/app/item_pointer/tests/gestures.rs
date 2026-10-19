use super::gesture;
use gpui::Modifiers;

#[test]
fn unselected_item_waits_for_movement_then_starts_rectangle() {
    assert_eq!(gesture(false, Modifiers::default(), 0.0, 4.0), None);
    assert_eq!(gesture(false, Modifiers::default(), 4.0, 4.0), None);
    assert_eq!(gesture(false, Modifiers::default(), 5.0, 4.0), Some(false));
}

#[test]
fn previously_selected_item_immediately_uses_gpui_file_drag() {
    assert_eq!(gesture(true, Modifiers::default(), 0.0, 4.0), Some(true));
    assert_eq!(gesture(true, Modifiers::default(), 3.0, 4.0), Some(true));
    assert_eq!(gesture(true, Modifiers::default(), 5.0, 4.0), Some(true));
}

#[test]
fn selection_modifiers_override_file_drag_even_on_selected_items() {
    for was_selected in [false, true] {
        for (control, shift) in [(true, false), (false, true), (true, true)] {
            let modifiers = Modifiers { control, shift, ..Default::default() };
            assert_eq!(gesture(was_selected, modifiers, 0.0, 4.0), None);
            assert_eq!(gesture(was_selected, modifiers, 4.0, 4.0), None);
            assert_eq!(gesture(was_selected, modifiers, 5.0, 4.0), Some(false));
        }
    }
}
