use super::*;

fn screen(width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(0.0), px(0.0)), size(px(width), px(height)))
}

#[test]
fn minimum_size_gives_way_only_to_a_display_too_small_for_it() {
    let min = size(px(720.0), px(480.0));
    let reserved = px(48.0);
    assert_eq!(fitting_min_size(min, &[screen(2048.0, 1280.0)], reserved), min);
    assert_eq!(fitting_min_size(min, &[screen(800.0, 480.0)], reserved), size(px(720.0), px(432.0)));
    assert_eq!(fitting_min_size(min, &[screen(640.0, 360.0)], reserved), size(px(640.0), px(312.0)));
    assert_eq!(fitting_min_size(min, &[], reserved), min);
}

#[test]
fn minimum_size_follows_the_largest_display() {
    let min = size(px(720.0), px(480.0));
    let screens = [screen(800.0, 480.0), screen(2048.0, 1280.0)];
    assert_eq!(fitting_min_size(min, &screens, px(48.0)), min);
}
