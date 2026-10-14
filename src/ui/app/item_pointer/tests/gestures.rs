use super::gesture;

#[test]
fn unselected_item_waits_for_movement_then_starts_rectangle() {
    assert_eq!(gesture(false, 0.0, 4.0), None);
    assert_eq!(gesture(false, 4.0, 4.0), None);
    assert_eq!(gesture(false, 5.0, 4.0), Some(false));
}

#[test]
fn previously_selected_item_immediately_uses_gpui_file_drag() {
    assert_eq!(gesture(true, 0.0, 4.0), Some(true));
    assert_eq!(gesture(true, 3.0, 4.0), Some(true));
    assert_eq!(gesture(true, 5.0, 4.0), Some(true));
}
