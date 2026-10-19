//! System-codec A/V playback in the same bounded, kill-on-close job as previews.
//! The worker owns its passive video surface and synchronized native renderer.
use super::windows_preview_worker::Worker;
use super::{
    ViewerMediaCommand, ViewerMediaControl, MEDIA_ENDED, MEDIA_FAILED, MEDIA_PAUSED, MEDIA_PLAYING,
};
use std::{
    io::{self, BufRead, Read, Write},
    os::windows::io::AsRawHandle,
    path::Path,
    process::Command,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use windows::{
    core::{implement, w, HSTRING},
    Win32::{
        Foundation::{HANDLE, HWND, POINT, RECT},
        Graphics::Gdi::ClientToScreen,
        Media::MediaFoundation::*,
        System::Pipes::PeekNamedPipe,
        UI::WindowsAndMessaging::*,
    },
};

const DEADLINE: Duration = Duration::from_secs(15);

pub(super) fn supervise(path: &Path, control: &ViewerMediaControl) -> io::Result<()> {
    supervise_in(&std::env::current_exe()?, path, control)
}

fn supervise_in(executable: &Path, path: &Path, control: &ViewerMediaControl) -> io::Result<()> {
    if control.is_cancelled() {
        return Ok(());
    }
    let mut command = Command::new(executable);
    command
        .arg("--oxyde-preview-worker-v1")
        .arg("viewer-media")
        .arg(path);
    let (mut worker, mut output, mut input) =
        Worker::start_duplex_checked(command, 1024 * 1024 * 1024, &|| {
            if control.is_cancelled() {
                Err(io::Error::from(io::ErrorKind::Interrupted))
            } else {
                Ok(())
            }
        })?;
    let mut previous = None;
    let mut heartbeat = Instant::now();
    loop {
        if control.is_cancelled() {
            return Ok(());
        }
        super::windows_resource_pressure::check()?;
        let command = *control
            .command
            .lock()
            .map_err(|_| io::Error::other("Media control poisoned"))?;
        if previous != Some(command) {
            serde_json::to_writer(&mut input, &command)?;
            input.write_all(b"\n")?;
            input.flush()?;
            previous = Some(command);
        }
        let mut available = 0;
        unsafe {
            PeekNamedPipe(
                HANDLE(output.as_raw_handle()),
                None,
                0,
                None,
                Some(&mut available),
                None,
            )
        }
        .map_err(native)?;
        if available > 0 {
            let mut status = [0; 64];
            let amount = output.read(&mut status[..(available as usize).min(64)])?;
            for value in &status[..amount] {
                if !matches!(
                    *value,
                    0 | MEDIA_PLAYING | MEDIA_PAUSED | MEDIA_ENDED | MEDIA_FAILED
                ) {
                    return Err(io::Error::other("Invalid media worker status"));
                }
                control.status.store(*value, Ordering::Release);
                if *value == MEDIA_FAILED {
                    return Err(io::Error::other("System codecs could not play the media"));
                }
            }
            heartbeat = Instant::now();
        }
        if heartbeat.elapsed() > DEADLINE {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Media worker stopped responding",
            ));
        }
        if worker.child.try_wait()?.is_some() {
            return Err(io::Error::other("Media worker exited"));
        }
        super::windows_resource_pressure::check_worker(HANDLE(worker.child.as_raw_handle()))?;
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn native(error: windows::core::Error) -> io::Error {
    io::Error::other(error.to_string())
}

#[implement(IMFPMediaPlayerCallback)]
struct PlayerEvents {
    status: Arc<AtomicU8>,
}
impl IMFPMediaPlayerCallback_Impl for PlayerEvents_Impl {
    fn OnMediaPlayerEvent(&self, event: *const MFP_EVENT_HEADER) {
        let Some(event) = (unsafe { event.as_ref() }) else {
            return;
        };
        let status = if event.hrEvent.is_err() {
            Some(MEDIA_FAILED)
        } else {
            match event.eEventType {
                MFP_EVENT_TYPE_PLAY => Some(MEDIA_PLAYING),
                MFP_EVENT_TYPE_PAUSE => Some(MEDIA_PAUSED),
                MFP_EVENT_TYPE_PLAYBACK_ENDED => Some(MEDIA_ENDED),
                _ => None,
            }
        };
        if let Some(status) = status {
            self.status.store(status, Ordering::Release);
        }
    }
}

struct Player(IMFPMediaPlayer);
impl Drop for Player {
    fn drop(&mut self) {
        unsafe {
            let _ = self.0.Shutdown();
        }
    }
}
struct Surface(HWND);
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

/// A fixed-size JSON command carries only placement and playback controls.
fn read_command(reader: &mut impl BufRead) -> io::Result<Option<ViewerMediaCommand>> {
    let mut line = Vec::new();
    let count = reader.take(513).read_until(b'\n', &mut line)?;
    if count == 0 {
        return Ok(None);
    }
    if count > 512 || line.last() != Some(&b'\n') {
        return Err(io::Error::other("Invalid media command size"));
    }
    let command: ViewerMediaCommand = serde_json::from_slice(&line)?;
    if command.parent == 0
        || command
            .rect
            .iter()
            .any(|value| value.unsigned_abs() > 131_072)
        || command.rect[2] < 0
        || command.rect[3] < 0
    {
        return Err(io::Error::other("Invalid media placement"));
    }
    Ok(Some(command))
}

pub(super) fn run(path: &Path) -> io::Result<()> {
    let _com = super::windows_com::ComApartment::mta_or_existing().map_err(native)?;
    let first = read_command(&mut io::stdin().lock())?
        .ok_or_else(|| io::Error::other("Missing media placement"))?;
    // Only ordinary local/UNC file paths reach the system codecs, never URLs.
    if !path.is_absolute() || !path.is_file() {
        return Err(io::Error::other("Invalid media source"));
    }
    let latest = Arc::new(Mutex::new(first));
    let closed = Arc::new(AtomicBool::new(false));
    let input_latest = latest.clone();
    let input_closed = closed.clone();
    std::thread::spawn(move || {
        let mut input = io::stdin().lock();
        while let Ok(Some(command)) = read_command(&mut input) {
            if let Ok(mut value) = input_latest.lock() {
                *value = command;
            } else {
                break;
            }
        }
        input_closed.store(true, Ordering::Release);
    });
    unsafe {
        let surface = Surface(
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!("Oxyde media surface"),
                // GPUI's DirectComposition target is above child HWNDs. An owned,
                // passive popup keeps video visible without changing GPUI or focus.
                WS_POPUP | WS_DISABLED,
                0,
                0,
                1,
                1,
                Some(HWND(first.parent as usize as *mut _)),
                None,
                None,
                None,
            )
            .map_err(native)?,
        );
        let status = Arc::new(AtomicU8::new(0));
        let callback: IMFPMediaPlayerCallback = PlayerEvents {
            status: status.clone(),
        }
        .into();
        let mut player = None;
        // Windows resolves the source and chooses its installed audio/video codecs.
        MFPCreateMediaPlayer(
            &HSTRING::from(path.as_os_str()),
            true,
            MFP_OPTION_FREE_THREADED_CALLBACK,
            &callback,
            Some(surface.0),
            Some(&mut player),
        )
        .map_err(native)?;
        let player = Player(player.ok_or_else(|| io::Error::other("Missing system media player"))?);
        let mut applied = None;
        let started = Instant::now();
        let mut heartbeat = Instant::now() - Duration::from_secs(1);
        let mut output = io::stdout().lock();
        let mut paused = false;
        let mut has_video = false;
        let mut checked_video = false;
        while !closed.load(Ordering::Acquire) {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            let command = *latest
                .lock()
                .map_err(|_| io::Error::other("Media controls poisoned"))?;
            if command.parent != first.parent {
                return Err(io::Error::other("Media owner changed"));
            }
            let value = status.load(Ordering::Acquire);
            if value != 0 && !checked_video {
                if let Ok(item) = player.0.GetMediaItem() {
                    let mut video = windows::core::BOOL(0);
                    item.HasVideo(Some(&mut video), None).map_err(native)?;
                    has_video = video.as_bool();
                    checked_video = true;
                }
            }
            if value == MEDIA_FAILED || (value == 0 && started.elapsed() > DEADLINE) {
                output.write_all(&[MEDIA_FAILED])?;
                output.flush()?;
                return Err(io::Error::other("System media playback failed"));
            }
            let parent = HWND(first.parent as usize as *mut _);
            if !IsWindow(Some(parent)).as_bool() {
                break;
            }
            let [x, y, width, height] = command.rect;
            // Native resize can precede GPUI's next paint. Keep the passive surface
            // inside the current client even while its last layout is still queued.
            let mut client = RECT::default();
            GetClientRect(parent, &mut client).map_err(native)?;
            let right = x.saturating_add(width).min(client.right);
            let bottom = y.saturating_add(height).min(client.bottom);
            let x = x.max(client.left).min(client.right);
            let y = y.max(client.top).min(client.bottom);
            let width = (right - x).max(0);
            let height = (bottom - y).max(0);
            let mut origin = POINT { x, y };
            if !ClientToScreen(parent, &mut origin).as_bool() {
                return Err(io::Error::last_os_error());
            }
            let visible = command.visible
                && has_video
                && width > 0
                && height > 0
                && IsWindowVisible(parent).as_bool()
                && !IsIconic(parent).as_bool()
                && windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(parent).as_bool();
            let placement = (origin.x, origin.y, width, height, visible);
            if applied != Some(placement) {
                let visibility_changed = applied
                    .is_none_or(|previous: (i32, i32, i32, i32, bool)| previous.4 != visible);
                SetWindowPos(
                    surface.0,
                    None,
                    origin.x,
                    origin.y,
                    width.max(1),
                    height.max(1),
                    SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOOWNERZORDER,
                )
                .map_err(native)?;
                if visibility_changed {
                    let _ =
                        ShowWindow(surface.0, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
                }
                let _ = player.0.UpdateVideo();
                applied = Some(placement);
            }
            if value != 0 && command.paused != paused && value != MEDIA_ENDED {
                if command.paused {
                    player.0.Pause().map_err(native)?;
                } else {
                    player.0.Play().map_err(native)?;
                }
                paused = command.paused;
            }
            if heartbeat.elapsed() >= Duration::from_millis(100) {
                output.write_all(&[value])?;
                output.flush()?;
                heartbeat = Instant::now();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "windows_viewer_media/tests/protocol.rs"]
mod tests;

#[cfg(test)]
#[path = "windows_viewer_media/tests/native.rs"]
mod native_tests;
