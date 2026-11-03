//! Grid video thumbnails: the frame shown at two seconds, or the first frame of a
//! shorter video. Frames come from the hover decoder, which loops shorter streams.
use crate::models::CommandError;
use image::RgbaImage;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Moment of the video the grid shows.
pub(super) const AT: Duration = Duration::from_secs(2);
/// Bounds decoding when timestamps never reach `AT`.
const MAX_FRAMES: usize = 600;

/// Keeps the first frame and stops at the frame shown at `AT`, or when the stream
/// loops back before reaching it.
#[derive(Default)]
pub(super) struct Picker {
    first: Option<RgbaImage>,
    previous: Option<RgbaImage>,
    last_pts: Option<Duration>,
    frames: usize,
    chosen: Option<RgbaImage>,
}

impl Picker {
    /// Returns whether more frames are needed.
    pub(super) fn offer(&mut self, image: RgbaImage, pts: Duration) -> bool {
        if self.chosen.is_some() {
            return false;
        }
        if self.last_pts.is_some_and(|last| pts < last) {
            // Looped: the video is shorter than `AT`.
            self.chosen = self.first.take();
            return false;
        }
        self.frames += 1;
        if pts > AT {
            // The previous frame is still on screen at `AT`.
            self.chosen = Some(self.previous.take().unwrap_or(image));
            return false;
        }
        if pts == AT || self.frames >= MAX_FRAMES {
            self.chosen = Some(image);
            return false;
        }
        if self.first.is_none() {
            self.first = Some(image.clone());
        }
        self.previous = Some(image);
        self.last_pts = Some(pts);
        true
    }

    /// The chosen frame; a stream that stops early gives its first one.
    pub(super) fn finish(self) -> Option<RgbaImage> {
        self.chosen.or(self.first)
    }
}

pub(super) fn grid_frame(
    path: String,
    frames: impl FnOnce(String, Box<dyn FnMut(RgbaImage, Duration) -> bool + Send>) -> Result<(), CommandError>,
) -> Result<RgbaImage, CommandError> {
    let picker = Arc::new(Mutex::new(Picker::default()));
    let sink = picker.clone();
    let result = frames(path, Box::new(move |image, pts| {
        sink.lock().unwrap_or_else(std::sync::PoisonError::into_inner).offer(image, pts)
    }));
    let picker = std::mem::take(&mut *picker.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
    match (picker.finish(), result) {
        (Some(image), _) => Ok(image),
        (None, Err(error)) => Err(error),
        (None, Ok(())) => Err(CommandError::Other("Video produced no frame".into())),
    }
}

#[cfg(test)]
#[path = "video_thumbnail/tests/picker.rs"]
mod tests;
