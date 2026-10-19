use super::drag_copy_override;
use gpui::Modifiers;

#[test]
fn copy_and_move_can_be_changed_and_released_during_the_same_drag() {
    let sequence = [
        (Modifiers::default(), None),
        (Modifiers { control: true, ..Default::default() }, Some(true)),
        (Modifiers { shift: true, ..Default::default() }, Some(false)),
        (Modifiers::default(), None),
    ];
    // Releasing an override returns to the target's same-volume/cross-volume policy.
    for default_copy in [false, true] {
        for (modifiers, expected_override) in sequence {
            let copy_override = drag_copy_override(modifiers);
            assert_eq!(copy_override, expected_override);
            assert_eq!(copy_override.unwrap_or(default_copy), expected_override.unwrap_or(default_copy));
        }
    }
}

#[test]
fn releasing_control_while_shift_is_held_returns_to_move() {
    let both = Modifiers { control: true, shift: true, ..Default::default() };
    assert_eq!(drag_copy_override(both), Some(true));
    assert_eq!(drag_copy_override(Modifiers { control: false, ..both }), Some(false));
}
