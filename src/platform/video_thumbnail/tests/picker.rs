use super::*;

fn frame(tag: u8) -> RgbaImage {
    RgbaImage::from_pixel(1, 1, image::Rgba([tag, 0, 0, 255]))
}

fn run(frames: &[(u8, u64)]) -> Option<u8> {
    let mut picker = Picker::default();
    for &(tag, ms) in frames {
        if !picker.offer(frame(tag), Duration::from_millis(ms)) {
            break;
        }
    }
    picker.finish().map(|image| image.get_pixel(0, 0).0[0])
}

#[test]
fn shows_the_frame_on_screen_at_two_seconds() {
    assert_eq!(run(&[(1, 0), (2, 1960), (3, 2040)]), Some(2));
    assert_eq!(run(&[(1, 0), (2, 1000), (3, 2000), (4, 2040)]), Some(3));
}

#[test]
fn shorter_videos_show_their_first_frame() {
    assert_eq!(run(&[(1, 0), (2, 500), (3, 1500), (4, 0), (5, 500)]), Some(1));
    assert_eq!(run(&[(1, 0), (2, 500)]), Some(1));
}

#[test]
fn a_first_frame_late_in_the_stream_stays_usable() {
    assert_eq!(run(&[(1, 5000)]), Some(1));
}
