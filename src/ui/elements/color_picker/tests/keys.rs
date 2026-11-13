use super::{field_key, hue_key, Hsv};

#[test]
fn arrows_move_the_field_by_a_hundredth_and_shift_by_a_tenth() {
    let hsv = Hsv { h: 40.0, s: 0.5, v: 0.5 };
    let right = field_key(hsv, "right", false).unwrap();
    assert!((right.s - 0.51).abs() < 1e-6 && right.v == 0.5 && right.h == 40.0);
    let down = field_key(hsv, "down", true).unwrap();
    assert!((down.v - 0.4).abs() < 1e-6 && down.s == 0.5);
    assert_eq!(field_key(hsv, "home", false), None);
}

#[test]
fn the_field_stops_at_its_edges() {
    let corner = Hsv { h: 0.0, s: 1.0, v: 0.0 };
    assert_eq!(field_key(corner, "right", true), Some(corner));
    assert_eq!(field_key(corner, "down", false), Some(corner));
}

#[test]
fn the_hue_steps_and_reaches_both_reds() {
    assert!((hue_key(40.0, "right", false).unwrap() - 43.6).abs() < 1e-4);
    assert!((hue_key(40.0, "down", true).unwrap() - 4.0).abs() < 1e-4);
    assert_eq!(hue_key(350.0, "up", true), Some(360.0));
    assert_eq!(hue_key(5.0, "left", true), Some(0.0));
    assert_eq!(hue_key(120.0, "home", false), Some(0.0));
    assert_eq!(hue_key(120.0, "end", false), Some(360.0));
    assert_eq!(Hsv { h: 360.0, s: 1.0, v: 1.0 }.to_rgb(), 0xff0000);
    assert_eq!(hue_key(120.0, "space", false), None);
}
