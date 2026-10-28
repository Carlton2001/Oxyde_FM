use super::MediaSession;
use oxyde_lib::platform::ViewerMediaControl;
use std::sync::Arc;

#[test]
fn hiding_video_for_settings_preserves_playback_and_explicit_pause() {
    let mut media = MediaSession {
        control: Arc::new(ViewerMediaControl::default()),
        paused: false,
        cancellation: crate::ui::preview_jobs::Cancellation::default(),
        frame: Default::default(),
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

#[test]
fn external_open_pauses_without_seeking_or_stopping_the_session() {
    for blocked in [false, true] {
        let mut media = MediaSession {
            control: Arc::new(ViewerMediaControl::default()),
            paused: false,
            cancellation: crate::ui::preview_jobs::Cancellation::default(),
            frame: Default::default(),
        };
        media.control.seek(12_345);
        let seek_id = media.control.command.lock().unwrap().seek_id;
        // Repeated opens must keep the player paused, never toggle it.
        for _ in 0..2 {
            media.pause(blocked);
            assert!(media.paused);
            let command = media.control.command.lock().unwrap();
            assert!(command.paused);
            assert_eq!(command.visible, !blocked);
            assert_eq!(command.seek_ms, 12_345);
            assert_eq!(command.seek_id, seek_id);
            assert!(!media.control.is_cancelled());
        }
        media.update_visibility(false);
        assert!(media.control.command.lock().unwrap().paused);
    }
}
