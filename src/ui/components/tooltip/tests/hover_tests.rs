use super::{Timing, BRIDGE_DELAY, SHOW_DELAY};
use std::time::Duration;

#[test]
fn first_hover_waits_then_direct_switch_is_immediate() {
    let mut timing = Timing::default();
    assert!(!timing.enter("a".into()));
    assert!(timing.show(timing.revision, "a"));
    assert!(timing.enter("b".into()));
    assert_eq!(SHOW_DELAY, Duration::from_millis(750));
}

#[test]
fn bridge_keeps_session_warm_and_new_hover_invalidates_expiry() {
    let mut timing = Timing::default();
    timing.enter("a".into());
    timing.show(timing.revision, "a");
    assert!(timing.leave("a"));
    let expiry = timing.revision;
    assert!(timing.warm);
    assert!(timing.in_gap(expiry));
    assert!(timing.enter("b".into()));
    assert!(!timing.in_gap(expiry));
    assert!(!timing.expire(expiry));
    assert!(timing.warm);
    assert_eq!(BRIDGE_DELAY, Duration::from_millis(300));
}

#[test]
fn stale_load_and_late_leave_cannot_replace_new_target() {
    let mut timing = Timing::default();
    timing.enter("a".into());
    let revision = timing.revision;
    assert!(!timing.enter("b".into()));
    assert!(!timing.show(revision, "a"));
    assert!(!timing.leave("a"));
    assert!(timing.show(timing.revision, "b"));
}

#[test]
fn expired_bridge_requires_initial_delay_again() {
    let mut timing = Timing::default();
    timing.enter("a".into());
    timing.show(timing.revision, "a");
    timing.leave("a");
    assert!(timing.expire(timing.revision));
    assert!(!timing.enter("b".into()));
}

#[test]
fn region_change_restarts_delay_and_invalidates_old_timers() {
    let mut timing = Timing::default();
    timing.enter("directory:a".into());
    timing.show(timing.revision, "directory:a");
    timing.leave("directory:a");
    let previous_revision = timing.revision;
    timing.reset();
    assert!(!timing.enter("file:b".into()));
    assert!(!timing.expire(previous_revision));
    assert!(!timing.show(previous_revision, "directory:a"));
    assert!(timing.show(timing.revision, "file:b"));
}
