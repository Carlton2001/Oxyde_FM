use super::{autoscroll_steps, next_boundary, previous_boundary, span};

#[test]
fn span_keeps_the_anchor_and_says_which_end_moves() {
    assert_eq!(span(4, 9), (4..9, false));
    assert_eq!(span(4, 1), (1..4, true));
    assert_eq!(span(4, 4), (4..4, false));
}

#[test]
fn boundaries_step_whole_graphemes_and_stop_at_the_ends() {
    let text = "aé👍🏽b";
    assert_eq!(next_boundary(text, 0), 1);
    assert_eq!(next_boundary(text, 1), 3);
    assert_eq!(next_boundary(text, 3), 11);
    assert_eq!(next_boundary(text, text.len()), text.len());
    assert_eq!(previous_boundary(text, 11), 3);
    assert_eq!(previous_boundary(text, 0), 0);
}

#[test]
fn autoscroll_speeds_up_with_the_distance_past_the_edge() {
    assert_eq!(autoscroll_steps(2.0, 16.0), 1);
    assert_eq!(autoscroll_steps(-40.0, 16.0), 3);
    assert_eq!(autoscroll_steps(5.0, 0.0), 6);
}
