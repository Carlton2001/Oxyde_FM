use super::*;

#[test]
fn an_owned_reply_keeps_its_single_image_and_kinds_match_decoding() {
    let frame = Frame::from_parts(
        RgbaImage::from_pixel(2, 1, image::Rgba([1, 2, 3, 4])),
        0,
        0,
        Delay::from_numer_denom_ms(30, 1),
    );
    let mut bytes = Vec::new();
    encode(Reply::Images { frames: vec![frame], limited: true, natural_size: Some((21, 30)) }, &mut bytes).unwrap();
    assert!(matches!(reply_kind_with(&bytes, PREVIEW).unwrap(), ReplyKind::Images(1)));
    let Reply::Images { frames, limited, natural_size } = decode_owned_with(bytes.clone(), PREVIEW).unwrap() else { panic!() };
    assert!(limited);
    assert_eq!(natural_size, Some((21, 30)));
    assert_eq!(frames[0].buffer().as_raw(), &[1, 2, 3, 4, 1, 2, 3, 4]);
    assert_eq!(frames[0].delay().numer_denom_ms(), (30, 1));
    bytes.push(0);
    assert!(decode_owned_with(bytes, PREVIEW).is_err());
    let mut text = Vec::new();
    encode(Reply::Text(String::new()), &mut text).unwrap();
    assert!(matches!(reply_kind_with(&text, PREVIEW).unwrap(), ReplyKind::Text(true)));
    assert!(matches!(decode_owned_with(text, PREVIEW).unwrap(), Reply::Text(text) if text.is_empty()));
}
