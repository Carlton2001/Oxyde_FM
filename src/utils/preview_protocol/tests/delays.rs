use super::*;

#[test]
fn a_frame_without_a_usable_delay_waits_100_ms() {
    let frame = Frame::from_parts(
        RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, 4])),
        0,
        0,
        Delay::from_numer_denom_ms(0, 1),
    );
    let mut bytes = Vec::new();
    encode(Reply::Images { frames: vec![frame], limited: false, natural_size: None }, &mut bytes).unwrap();
    let Reply::Images { frames, .. } = decode(&bytes).unwrap() else { panic!() };
    assert_eq!(frames[0].delay().numer_denom_ms(), (100, 1));
}
