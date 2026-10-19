use super::rectangle_selection;
use gpui::Modifiers;
use std::collections::HashSet;

fn paths(names: &[&str]) -> HashSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn plain_rectangle_replaces_the_previous_group() {
    assert_eq!(
        rectangle_selection(&paths(&["a", "b"]), paths(&["c", "d"]), Modifiers::default()),
        paths(&["c", "d"]),
    );
}

#[test]
fn shift_rectangle_adds_a_separate_group_and_can_shrink_again() {
    let initial = paths(&["a", "b"]);
    let modifiers = Modifiers { shift: true, ..Default::default() };
    let expanded = rectangle_selection(&initial, paths(&["d", "e", "f"]), modifiers);
    assert_eq!(expanded, paths(&["a", "b", "d", "e", "f"]));
    // Recompute from before the press, not the Shift-click interval or the last frame.
    let contracted = rectangle_selection(&initial, paths(&["d"]), modifiers);
    assert_eq!(contracted, paths(&["a", "b", "d"]));
    assert_eq!(rectangle_selection(&initial, paths(&[]), modifiers), initial);
}

#[test]
fn control_rectangle_toggles_overlap_relative_to_the_initial_selection() {
    let initial = paths(&["a", "b"]);
    let modifiers = Modifiers { control: true, ..Default::default() };
    assert_eq!(rectangle_selection(&initial, paths(&["b", "c"]), modifiers), paths(&["a", "c"]));
    // Passing over the same items repeatedly must not toggle them back each frame.
    assert_eq!(rectangle_selection(&initial, paths(&["b", "c"]), modifiers), paths(&["a", "c"]));
    assert_eq!(rectangle_selection(&initial, paths(&["c"]), modifiers), paths(&["a", "b", "c"]));
    assert_eq!(rectangle_selection(&initial, paths(&[]), modifiers), initial);
}

#[test]
fn control_and_shift_keep_control_toggle_semantics() {
    let modifiers = Modifiers { control: true, shift: true, ..Default::default() };
    assert_eq!(rectangle_selection(&paths(&["a", "b"]), paths(&["b", "c"]), modifiers), paths(&["a", "c"]));
}
