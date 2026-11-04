use super::Hsv;

#[test]
fn rgb_survives_a_round_trip_through_hsv() {
    for color in [0x000000, 0xffffff, 0xff0000, 0x00ff00, 0x0000ff, 0x3d5af1, 0xd4a54a, 0x12101b, 0x808080] {
        assert_eq!(Hsv::from_rgb(color).to_rgb(), color, "{color:06x}");
    }
}

#[test]
fn grays_have_no_saturation_and_hue_wraps() {
    let gray = Hsv::from_rgb(0x808080);
    assert_eq!(gray.s, 0.0);
    assert_eq!(Hsv { h: 360.0, s: 1.0, v: 1.0 }.to_rgb(), 0xff0000);
    assert_eq!(Hsv { h: 120.0, s: 1.0, v: 1.0 }.to_rgb(), 0x00ff00);
}
