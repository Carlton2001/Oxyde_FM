//! System-codec A/V playback in the same bounded, kill-on-close job as previews.
//! The worker owns its passive video surface and an IMFMediaEngine presenting
//! synchronized audio and video into it.
use super::windows_preview_worker::Worker;
use super::{
    decode_media_report, encode_media_report, ViewerMediaCommand, ViewerMediaControl, MEDIA_ENDED,
    MEDIA_FAILED, MEDIA_PAUSED, MEDIA_PLAYING, MEDIA_REPORT_BYTES,
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
        Foundation::{HANDLE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{BeginPaint, ClientToScreen, EndPaint, PAINTSTRUCT},
        System::LibraryLoader::GetModuleHandleW,
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
    let mut pending: Vec<u8> = Vec::new();
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
            let mut bytes = [0; MEDIA_REPORT_BYTES * 8];
            let amount = output.read(&mut bytes[..(available as usize).min(MEDIA_REPORT_BYTES * 8)])?;
            pending.extend_from_slice(&bytes[..amount]);
            while pending.len() >= MEDIA_REPORT_BYTES {
                let report: [u8; MEDIA_REPORT_BYTES] =
                    pending[..MEDIA_REPORT_BYTES].try_into().expect("report length");
                pending.drain(..MEDIA_REPORT_BYTES);
                let (status, position, duration) = decode_media_report(&report)
                    .ok_or_else(|| io::Error::other("Invalid media worker status"))?;
                control.status.store(status, Ordering::Release);
                control.position_ms.store(position, Ordering::Release);
                control.duration_ms.store(duration, Ordering::Release);
                if status == MEDIA_FAILED {
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

#[implement(IMFMediaEngineNotify)]
struct EngineEvents {
    status: Arc<AtomicU8>,
    metadata: Arc<AtomicBool>,
}
impl IMFMediaEngineNotify_Impl for EngineEvents_Impl {
    fn EventNotify(&self, event: u32, _param1: usize, _param2: u32) -> windows::core::Result<()> {
        let event = MF_MEDIA_ENGINE_EVENT(event as i32);
        let status = match event {
            MF_MEDIA_ENGINE_EVENT_PLAYING => Some(MEDIA_PLAYING),
            MF_MEDIA_ENGINE_EVENT_PAUSE => Some(MEDIA_PAUSED),
            MF_MEDIA_ENGINE_EVENT_ENDED => Some(MEDIA_ENDED),
            MF_MEDIA_ENGINE_EVENT_ERROR | MF_MEDIA_ENGINE_EVENT_ABORT => Some(MEDIA_FAILED),
            MF_MEDIA_ENGINE_EVENT_LOADEDMETADATA => {
                self.metadata.store(true, Ordering::Release);
                None
            }
            _ => None,
        };
        if let Some(status) = status {
            self.status.store(status, Ordering::Release);
        }
        Ok(())
    }
}

/// Media Foundation stays started for the engine's whole lifetime.
struct Platform;
impl Platform {
    fn start() -> io::Result<Self> {
        unsafe { MFStartup(MF_SDK_VERSION << 16 | MF_API_VERSION, MFSTARTUP_FULL) }.map_err(native)?;
        Ok(Self)
    }
}
impl Drop for Platform {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
        }
    }
}

struct Engine {
    engine: IMFMediaEngine,
    extended: IMFMediaEngineEx,
}
impl Drop for Engine {
    fn drop(&mut self) {
        unsafe {
            let _ = self.engine.Shutdown();
        }
    }
}
impl Engine {
    /// Windows resolves the source and chooses its installed audio/video codecs.
    /// The engine presents directly into the passive surface (HWND mode).
    fn create(path: &Path, surface: HWND, events: IMFMediaEngineNotify) -> io::Result<Self> {
        use windows::core::Interface;
        unsafe {
            let factory: IMFMediaEngineClassFactory = windows::Win32::System::Com::CoCreateInstance(
                &CLSID_MFMediaEngineClassFactory,
                None,
                windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
            )
            .map_err(native)?;
            let mut attributes = None;
            MFCreateAttributes(&mut attributes, 2).map_err(native)?;
            let attributes =
                attributes.ok_or_else(|| io::Error::other("Missing media engine attributes"))?;
            attributes.SetUnknown(&MF_MEDIA_ENGINE_CALLBACK, &events).map_err(native)?;
            attributes
                .SetUINT64(&MF_MEDIA_ENGINE_PLAYBACK_HWND, surface.0 as usize as u64)
                .map_err(native)?;
            let engine = factory
                .CreateInstance(MF_MEDIA_ENGINE_REAL_TIME_MODE.0 as u32, &attributes)
                .map_err(native)?;
            let extended: IMFMediaEngineEx = engine.cast().map_err(native)?;
            let engine = Self { engine, extended };
            // Playback starts once metadata is loaded, after any resume seek.
            engine
                .engine
                .SetSource(&windows::core::BSTR::from_wide(&HSTRING::from(path.as_os_str())))
                .map_err(native)?;
            Ok(engine)
        }
    }

    /// Position and duration in milliseconds; an unknown duration is zero.
    fn progress(&self) -> (u64, u64) {
        let milliseconds = |seconds: f64| {
            if seconds.is_finite() && seconds > 0.0 { (seconds * 1000.0) as u64 } else { 0 }
        };
        unsafe { (milliseconds(self.engine.GetCurrentTime()), milliseconds(self.engine.GetDuration())) }
    }

    fn place(&self, width: i32, height: i32) {
        let target = RECT { left: 0, top: 0, right: width.max(1), bottom: height.max(1) };
        unsafe {
            let _ = self.extended.UpdateVideoStream(None, Some(&target), None);
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

/// Set when Windows asks the surface to repaint; the engine then redraws the frame.
static REPAINT: AtomicBool = AtomicBool::new(false);

/// The video renderer owns every pixel of this surface. A GDI control class
/// (such as STATIC) paints its own background over the presented frames,
/// which leaves a stale image while audio continues.
unsafe extern "system" fn surface_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            let _ = unsafe { BeginPaint(hwnd, &mut paint) };
            let _ = unsafe { EndPaint(hwnd, &paint) };
            REPAINT.store(true, Ordering::Release);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn surface_class() -> io::Result<windows::core::PCWSTR> {
    let class = w!("OxydeMediaSurface");
    let instance = unsafe { GetModuleHandleW(None) }.map_err(native)?;
    let description = WNDCLASSW {
        lpfnWndProc: Some(surface_proc),
        hInstance: instance.into(),
        lpszClassName: class,
        ..Default::default()
    };
    // A second registration in the same worker reports ERROR_CLASS_ALREADY_EXISTS.
    let _ = unsafe { RegisterClassW(&description) };
    Ok(class)
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
    let class = surface_class()?;
    unsafe {
        let surface = Surface(
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                class,
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
        let _platform = Platform::start()?;
        let status = Arc::new(AtomicU8::new(0));
        let metadata = Arc::new(AtomicBool::new(false));
        let events: IMFMediaEngineNotify = EngineEvents {
            status: status.clone(),
            metadata: metadata.clone(),
        }
        .into();
        let player = Engine::create(path, surface.0, events)?;
        let mut applied = None;
        let started = Instant::now();
        let mut heartbeat = Instant::now() - Duration::from_secs(1);
        let mut output = io::stdout().lock();
        // The engine is created paused; the first command decides playback.
        let mut paused = true;
        let mut has_video = false;
        let mut checked_video = false;
        let mut applied_seek = 0;
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
            if REPAINT.swap(false, Ordering::AcqRel) {
                if let Some((_, _, width, height, _)) = applied {
                    player.place(width, height);
                }
            }
            if !checked_video && metadata.load(Ordering::Acquire) {
                has_video = player.engine.HasVideo().as_bool();
                checked_video = true;
                // The placement may already be current; size the stream now.
                applied = None;
                let _ = status.compare_exchange(0, MEDIA_PAUSED, Ordering::AcqRel, Ordering::Acquire);
            }
            if checked_video && command.seek_id != applied_seek {
                player.engine.SetCurrentTime(command.seek_ms as f64 / 1000.0).map_err(native)?;
                applied_seek = command.seek_id;
                // Seeking after the end makes the media playable again.
                if status.load(Ordering::Acquire) == MEDIA_ENDED {
                    status.store(MEDIA_PAUSED, Ordering::Release);
                    paused = true;
                }
            }
            let value = status.load(Ordering::Acquire);
            if value == MEDIA_FAILED || (value == 0 && started.elapsed() > DEADLINE) {
                output.write_all(&encode_media_report(MEDIA_FAILED, 0, 0))?;
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
                player.place(width, height);
                applied = Some(placement);
            }
            if checked_video && command.paused != paused && value != MEDIA_ENDED {
                if command.paused {
                    player.engine.Pause().map_err(native)?;
                } else {
                    player.engine.Play().map_err(native)?;
                }
                paused = command.paused;
            }
            if heartbeat.elapsed() >= Duration::from_millis(100) {
                let (position, duration) = player.progress();
                output.write_all(&encode_media_report(value, position, duration))?;
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
