use super::{Fade, BACKDROP_FADE, BACKDROP_OPACITY};
use std::time::Instant;

#[test]
fn a_fade_eases_from_its_start_to_the_backdrop_then_stops() {
    let started = Instant::now();
    let fade = Fade { dimmed: true, from: 1.0, started };
    assert_eq!(fade.opacity(started), (1.0, true));
    let (halfway, running) = fade.opacity(started + BACKDROP_FADE / 2);
    assert!(running && halfway < 1.0 && halfway > BACKDROP_OPACITY, "{halfway}");
    // Eased out: most of the way is done at half the time.
    assert!(halfway < (1.0 + BACKDROP_OPACITY) / 2.0, "{halfway}");
    assert_eq!(fade.opacity(started + BACKDROP_FADE), (BACKDROP_OPACITY, false));
    assert_eq!(fade.opacity(started + BACKDROP_FADE * 3), (BACKDROP_OPACITY, false));
}

#[test]
fn a_reversed_fade_leaves_from_where_the_previous_one_was() {
    let started = Instant::now();
    let fade = Fade { dimmed: false, from: 0.7, started };
    assert_eq!(fade.opacity(started).0, 0.7);
    let (end, running) = fade.opacity(started + BACKDROP_FADE);
    assert!((end - 1.0).abs() < 1e-6 && !running, "{end}");
}
