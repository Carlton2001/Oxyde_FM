use std::time::Duration;

pub(super) fn send_video_frame<T>(
    sender: &tokio::sync::mpsc::Sender<T>,
    mut frame: T,
    cancelled: &crate::ui::preview_jobs::Cancellation,
) -> bool {
    loop {
        if cancelled.is_cancelled() {
            return false;
        }
        match sender.try_send(frame) {
            Ok(()) => return true,
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => return false,
            Err(tokio::sync::mpsc::error::TrySendError::Full(value)) => {
                frame = value;
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

pub(super) struct CancelVideoPreview(pub(super) crate::ui::preview_jobs::Cancellation);
impl Drop for CancelVideoPreview {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

#[derive(Default)]
pub(super) struct VideoClock {
    origin: Option<(std::time::Instant, Duration)>,
    previous: Option<(Duration, std::time::Instant)>,
    interval: Duration,
}

impl VideoClock {
    pub(super) fn presentation_time(
        &mut self,
        pts: Duration,
        now: std::time::Instant,
    ) -> Option<std::time::Instant> {
        if let Some((previous_pts, previous_due)) = self.previous {
            if pts < previous_pts {
                // Keep the last frame visible for a full interval on loop restart.
                let restart = previous_due.checked_add(self.interval)?.max(now);
                self.origin = Some((restart, pts));
            } else if pts > previous_pts {
                self.interval = pts - previous_pts;
            }
        }
        let (origin, first_pts) = *self.origin.get_or_insert((now, pts));
        let due = origin.checked_add(pts.checked_sub(first_pts)?)?;
        self.previous = Some((pts, due));
        Some(due)
    }
}

#[cfg(test)]
mod video_cancellation_tests {
    use super::{send_video_frame, CancelVideoPreview};

    #[test]
    fn video_clock_keeps_both_avi_colors_visible_for_one_second_on_every_loop() {
        let start = std::time::Instant::now();
        let mut clock = super::VideoClock::default();
        for (pts, due) in [(0, 0), (1, 1), (0, 2), (1, 3), (0, 4)] {
            assert_eq!(
                clock.presentation_time(Duration::from_secs(pts), start),
                Some(start + Duration::from_secs(due))
            );
        }
    }

    #[test]
    fn video_clock_normalizes_initial_timestamp_and_restarts_short_clips() {
        let start = std::time::Instant::now();
        let mut clock = super::VideoClock::default();
        assert_eq!(clock.presentation_time(Duration::from_secs(3600), start), Some(start));
        assert_eq!(
            clock.presentation_time(Duration::from_millis(3_600_100), start),
            Some(start + Duration::from_millis(100))
        );
        assert_eq!(clock.presentation_time(Duration::from_secs(3600), start),
            Some(start + Duration::from_millis(200)));
    }
    use std::{sync::Arc, time::Duration};
    #[test]
    fn a_full_frame_queue_can_be_cancelled_without_a_consumer() {
        let (sender, _receiver) = tokio::sync::mpsc::channel(1);
        sender.try_send(1).unwrap();
        let signal = crate::ui::preview_jobs::Cancellation::default();
        let worker_signal = signal.clone();
        let (done, result) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            done.send(send_video_frame(&sender, 2, &worker_signal))
                .unwrap();
        });
        std::thread::sleep(Duration::from_millis(20));
        signal.cancel();
        assert!(!result.recv_timeout(Duration::from_secs(1)).unwrap());
        worker.join().unwrap();
    }
    #[test]
    fn final_preview_owner_cancels_without_receiving_any_frame() {
        let signal = crate::ui::preview_jobs::Cancellation::default();
        let owner = Arc::new(CancelVideoPreview(signal.clone()));
        let clone = owner.clone();
        drop(owner);
        assert!(!signal.is_cancelled());
        drop(clone);
        assert!(signal.is_cancelled());
    }
}
