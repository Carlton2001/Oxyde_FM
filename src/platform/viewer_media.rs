//! Cross-platform contract. Native decoding and IPC run outside the UI thread.
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Mutex,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ViewerMediaCommand {
    pub parent: u64,
    pub rect: [i32; 4],
    pub paused: bool,
    pub visible: bool,
}

pub const MEDIA_LOADING: u8 = 0;
pub const MEDIA_PLAYING: u8 = b'P';
pub const MEDIA_PAUSED: u8 = b'A';
pub const MEDIA_ENDED: u8 = b'E';
pub const MEDIA_FAILED: u8 = b'X';

#[derive(Default)]
pub struct ViewerMediaControl {
    pub command: Mutex<ViewerMediaCommand>,
    pub cancelled: AtomicBool,
    pub status: AtomicU8,
}

impl ViewerMediaControl {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    pub fn status(&self) -> u8 {
        self.status.load(Ordering::Acquire)
    }
}
