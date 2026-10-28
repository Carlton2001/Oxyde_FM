//! Mount notifications: the kernel flags `/proc/self/mountinfo` on every mount and
//! unmount of our namespace, whether UDisks2, `mount` or a network client made it.
//! UDisks2 signals add the devices that are plugged without being mounted, GVfs signals
//! the network shares mounted in its FUSE folder.
use rustix::event::{poll, PollFd, PollFlags};
use rustix::pipe::{pipe_with, PipeFlags};
use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Dropping the guard closes the pipe, which ends the watching thread.
pub(super) struct Watch {
    _stop: OwnedFd,
    _signals: Option<super::signals::Signals>,
    _gvfs: Option<super::signals::Signals>,
}

pub(super) fn start(revision: Arc<AtomicU64>) -> Result<Watch, String> {
    let mounts = std::fs::File::open("/proc/self/mountinfo").map_err(|error| error.to_string())?;
    let (stopped, stop) = pipe_with(PipeFlags::CLOEXEC).map_err(|error| error.to_string())?;
    std::thread::Builder::new()
        .name("oxyde-mount-watch".into())
        .spawn({
            let revision = revision.clone();
            move || run(&mounts, &stopped, &revision)
        })
        .map_err(|error| error.to_string())?;
    // Without UDisks2, the mount table alone still reports every mounted volume.
    let signals = super::signals::start(revision.clone())
        .inspect_err(|error| log::info!("UDisks2 signals unavailable: {error}"))
        .ok();
    let gvfs = super::signals::gvfs(revision)
        .inspect_err(|error| log::info!("GVfs signals unavailable: {error}"))
        .ok();
    Ok(Watch { _stop: stop, _signals: signals, _gvfs: gvfs })
}

fn run(mounts: &std::fs::File, stopped: &OwnedFd, revision: &AtomicU64) {
    // mountinfo is always readable; only PRI, with ERR, signals a change.
    let mut fds = [PollFd::new(mounts, PollFlags::PRI), PollFd::new(stopped, PollFlags::IN)];
    loop {
        match poll(&mut fds, None) {
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => {
                log::warn!("Mount notifications stopped: {error}");
                return;
            }
        }
        if !fds[1].revents().is_empty() {
            return;
        }
        if fds[0].revents().intersects(PollFlags::PRI | PollFlags::ERR) {
            revision.fetch_add(1, Ordering::Release);
        }
    }
}

#[cfg(test)]
#[path = "tests/watch.rs"]
mod tests;
