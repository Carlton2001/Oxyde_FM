use super::fitted_video;
use gpui::{point, px, size, Bounds};

#[test]
fn fullscreen_video_fills_matching_screen_without_reserving_controls() {
    let screen = Bounds::new(point(px(0.0), px(0.0)), size(px(1920.0), px(1080.0)));
    assert_eq!(fitted_video(screen, 16.0 / 9.0, px(0.0)), screen);
    let inline = fitted_video(screen, 16.0 / 9.0, px(41.0));
    assert_eq!(inline.size.height, px(1039.0));
    assert!(inline.size.width < screen.size.width);
}

#[test]
fn fullscreen_video_preserves_proportions_on_other_screens() {
    for (width, height, x, y, video_width, video_height) in [
        (1920.0, 1200.0, 0.0, 60.0, 1920.0, 1080.0),
        (2560.0, 1080.0, 320.0, 0.0, 1920.0, 1080.0),
        (1080.0, 1920.0, 0.0, 656.25, 1080.0, 607.5),
    ] {
        let screen = Bounds::new(point(px(0.0), px(0.0)), size(px(width), px(height)));
        let video = fitted_video(screen, 16.0 / 9.0, px(0.0));
        assert_eq!(video, Bounds::new(point(px(x), px(y)), size(px(video_width), px(video_height))));
    }
}
