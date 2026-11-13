use super::{Fade, BACKDROP_FADE};
use std::time::Instant;

#[test]
fn a_fade_eases_from_its_start_to_the_backdrop_then_stops() {
    let started = Instant::now();
    let fade = Fade { active: false, from: 1.0, started };
    assert_eq!(fade.activity(started), (1.0, true));
    let (halfway, running) = fade.activity(started + BACKDROP_FADE / 2);
    assert!(running && halfway < 1.0 && halfway > 0.0, "{halfway}");
    // Eased out: most of the way is done at half the time.
    assert!(halfway < 0.5, "{halfway}");
    assert_eq!(fade.activity(started + BACKDROP_FADE), (0.0, false));
    assert_eq!(fade.activity(started + BACKDROP_FADE * 3), (0.0, false));
}

#[test]
fn a_reversed_fade_leaves_from_where_the_previous_one_was() {
    let started = Instant::now();
    let fade = Fade { active: true, from: 0.4, started };
    assert_eq!(fade.activity(started).0, 0.4);
    let (end, running) = fade.activity(started + BACKDROP_FADE);
    assert!((end - 1.0).abs() < 1e-6 && !running, "{end}");
}
