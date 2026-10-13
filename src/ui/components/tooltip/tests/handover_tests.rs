use super::{can_publish, CHANGE_DELAY};
use std::time::Duration;

#[test]
fn fresh_preview_waits_for_stable_hover_before_replacing_previous_bubble() {
    assert!(!can_publish(Duration::ZERO, true, false));
    assert!(!can_publish(CHANGE_DELAY - Duration::from_millis(1), true, false));
    assert!(can_publish(CHANGE_DELAY, true, false));
}

#[test]
fn delay_alone_does_not_publish_an_incomplete_preview() {
    assert!(!can_publish(CHANGE_DELAY, false, false));
    assert!(!can_publish(Duration::from_secs(1), false, false));
    assert!(can_publish(Duration::from_secs(1), true, false));
}

#[test]
fn cached_preview_is_published_without_hover_delay_but_never_before_ready() {
    assert!(can_publish(Duration::ZERO, true, true));
    assert!(can_publish(Duration::from_millis(16), true, true));
    assert!(!can_publish(Duration::ZERO, false, true));
    assert!(!can_publish(CHANGE_DELAY, false, true));
}
