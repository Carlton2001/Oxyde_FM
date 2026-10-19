use super::MediaSession;
use oxyde_lib::platform::ViewerMediaControl;
use std::sync::Arc;

#[test]
fn hiding_video_for_settings_preserves_playback_and_explicit_pause() {
    let mut media = MediaSession {
        control: Arc::new(ViewerMediaControl::default()),
        paused: false,
        cancellation: crate::ui::preview_jobs::Cancellation::default(),
    };
    media.control.seek(12_345);
    for blocked in [false, true, false] {
        media.update_visibility(blocked);
        let command = media.control.command.lock().unwrap();
        assert_eq!(command.visible, !blocked);
        assert!(!command.paused, "Settings must not interrupt playback");
        assert_eq!(command.seek_ms, 12_345);
        assert!(!media.control.is_cancelled());
    }
    media.paused = true;
    for blocked in [true, false] {
        media.update_visibility(blocked);
        assert!(media.control.command.lock().unwrap().paused);
    }
}
