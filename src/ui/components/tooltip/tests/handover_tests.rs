use super::{can_publish, CHANGE_DELAY};
use std::time::Duration;

#[test]
fn cached_preview_waits_for_stable_hover_before_replacing_previous_bubble() {
    assert!(!can_publish(Duration::ZERO, true));
    assert!(!can_publish(CHANGE_DELAY - Duration::from_millis(1), true));
    assert!(can_publish(CHANGE_DELAY, true));
}

#[test]
fn delay_alone_does_not_publish_an_incomplete_preview() {
    assert!(!can_publish(CHANGE_DELAY, false));
    assert!(!can_publish(Duration::from_secs(1), false));
    assert!(can_publish(Duration::from_secs(1), true));
}
