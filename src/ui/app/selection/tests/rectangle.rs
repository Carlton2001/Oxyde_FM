use super::{autoscroll_depth, rectangle_selection};
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

#[test]
fn autoscroll_starts_inside_an_edge_band_and_grows_with_the_depth() {
    // A list from 100 to 500 px, at the baseline scale: 30 px bands.
    assert_eq!(autoscroll_depth(300.0, 100.0, 500.0), 0.0);
    assert_eq!(autoscroll_depth(130.0, 100.0, 500.0), 0.0);
    assert_eq!(autoscroll_depth(470.0, 100.0, 500.0), 0.0);
    assert_eq!(autoscroll_depth(120.0, 100.0, 500.0), -10.0);
    assert_eq!(autoscroll_depth(480.0, 100.0, 500.0), 10.0);
    // Beyond the list the depth keeps growing, up to its maximum.
    assert_eq!(autoscroll_depth(520.0, 100.0, 500.0), 50.0);
    assert_eq!(autoscroll_depth(2000.0, 100.0, 500.0), 150.0);
    assert_eq!(autoscroll_depth(-2000.0, 100.0, 500.0), -150.0);
}
