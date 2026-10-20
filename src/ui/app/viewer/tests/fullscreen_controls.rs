use super::should_hide;
use std::time::Duration;

#[test]
fn controls_hide_after_idle_but_remain_during_hover_and_seeking() {
    assert!(!should_hide(Duration::from_millis(1999), false, false));
    assert!(should_hide(Duration::from_secs(2), false, false));
    assert!(!should_hide(Duration::from_secs(30), true, false));
    assert!(!should_hide(Duration::from_secs(30), false, true));
}
