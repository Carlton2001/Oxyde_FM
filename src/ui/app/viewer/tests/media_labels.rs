use super::definition;

#[test]
fn definitions_follow_the_larger_side() {
    assert_eq!(definition(1920, 1080), Some("1080p"));
    assert_eq!(definition(1920, 800), Some("1080p"));
    assert_eq!(definition(1080, 1920), Some("1080p"));
    assert_eq!(definition(1280, 720), Some("720p"));
    assert_eq!(definition(3840, 1600), Some("4K"));
    assert_eq!(definition(720, 576), Some("576p"));
    assert_eq!(definition(320, 240), None);
}
