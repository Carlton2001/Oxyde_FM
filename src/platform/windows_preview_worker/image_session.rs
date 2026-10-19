//! One bounded LPAC session amortizes runtime staging (about 1.4 s per process)
//! across nearby image requests. Tooltips, the Viewer and audio covers use separate sessions:
//! a Viewer image needs a larger job, and neither waits for the other.
//! A compromised decoder can retain earlier images until recycling; it gains no
//! path access. Never reuse this session for Office, archives, PDF or video.
use super::{native, DEADLINE, MEMORY_BYTES};
use crate::commands::image_handle_worker::{SESSION_FLAG, SESSION_REQUESTS};
use crate::utils::preview_protocol::{self, Reply};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, OnceLock,
};
use std::{
    io::{self, Read, Write},
    os::windows::io::AsRawHandle,
    path::Path,
    process::Command,
    sync::Mutex,
    time::{Duration, Instant},
};
use windows::Win32::Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS, HANDLE};
use windows::Win32::System::Pipes::PeekNamedPipe;
use windows::Win32::System::Threading::GetCurrentProcess;

static STOPPING: AtomicBool = AtomicBool::new(false);
static SERVICE: OnceLock<Result<Service, String>> = OnceLock::new();
static VIEWER_SERVICE: OnceLock<Result<Service, String>> = OnceLock::new();
static AUDIO_SERVICE: OnceLock<Result<Service, String>> = OnceLock::new();
pub(super) const AUDIO_KIND: u8 = 3;
pub(super) const AUDIO_THUMBNAIL_KIND: u8 = 4;
/// Request kind of a full-resolution Viewer image.
pub(super) const VIEWER_KIND: u8 = 2;

/// Resource envelope and recycling of one session.
#[derive(Clone, Copy)]
struct Envelope {
    memory: usize,
    deadline: Duration,
    encoded_bytes: u64,
    max_reply: usize,
    idle: Duration,
    max_age: Duration,
}

const PREVIEW: Envelope = Envelope {
    memory: MEMORY_BYTES,
    deadline: DEADLINE,
    encoded_bytes: crate::utils::image_preview_decode::POLICY.encoded_bytes as u64,
    max_reply: preview_protocol::MAX_REPLY,
    idle: Duration::from_secs(15),
    max_age: Duration::from_secs(60),
};
/// Holds the encoded source, decoded pixels and their BGRA copy at once
/// (128 + 256 + 256 MiB). A Viewer is browsed for longer stretches, so its
/// session lives longer; it is released when the Viewer shows other content,
/// leaving the global budget to PDF sessions, players and video previews.
const VIEWER: Envelope = Envelope {
    memory: 1024 * 1024 * 1024,
    deadline: Duration::from_secs(30),
    encoded_bytes: crate::utils::image_preview_decode::VIEWER_POLICY.encoded_bytes as u64,
    max_reply: preview_protocol::VIEWER_MAX_REPLY,
    idle: Duration::from_secs(60),
    max_age: Duration::from_secs(300),
};

const AUDIO: Envelope = Envelope {
    memory: 256 * 1024 * 1024,
    encoded_bytes: u64::MAX, // Lofty seeks over audio data; only embedded pictures are decoded.
    max_reply: 512 * 512 * 4 + 16 * 1024,
    ..PREVIEW
};

fn envelope(kind: u8) -> Envelope {
    if kind == VIEWER_KIND { VIEWER } else if kind == AUDIO_KIND || kind == AUDIO_THUMBNAIL_KIND { AUDIO } else { PREVIEW }
}

struct Request {
    source: super::super::RetainedArchiveSource,
    kind: u8,
    reply: mpsc::Sender<io::Result<Vec<u8>>>,
}
enum Message {
    Request(Request),
    /// Starts the process ahead of the first request.
    Warm,
    /// Ends the process and returns its budget.
    Release,
    Stop,
}
struct Service {
    sender: mpsc::SyncSender<Message>,
    done: Mutex<mpsc::Receiver<()>>,
}

fn service(kind: u8) -> io::Result<&'static Service> {
    let (cell, name) = if kind == AUDIO_KIND || kind == AUDIO_THUMBNAIL_KIND {
        (&AUDIO_SERVICE, "audio-cover-session")
    } else if kind == VIEWER_KIND {
        (&VIEWER_SERVICE, "image-viewer-session")
    } else {
        (&SERVICE, "image-preview-session")
    };
    let envelope = envelope(kind);
    cell.get_or_init(|| {
        let executable = std::env::current_exe()
            .map_err(|error| error.to_string())?
            .with_file_name("oxyde-image-worker.exe");
        let (sender, requests) = mpsc::sync_channel(1);
        let (done, completion) = mpsc::channel();
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                serve_with(requests, &executable, envelope);
                let _ = done.send(());
            })
            .map_err(|error| error.to_string())?;
        Ok(Service {
            sender,
            done: Mutex::new(completion),
        })
    })
    .as_ref()
    .map_err(|error| io::Error::other(error.clone()))
}

/// Ends the Viewer's session; the next image or warm-up starts a new one.
pub(super) fn release_viewer() {
    if let Some(Ok(service)) = VIEWER_SERVICE.get() {
        let _ = service.sender.try_send(Message::Release);
    }
}

/// Prepares the Viewer's session so its first image skips process startup.
pub(super) fn warm_viewer() {
    if checkpoint().is_ok() {
        if let Ok(service) = service(VIEWER_KIND) {
            let _ = service.sender.try_send(Message::Warm);
        }
    }
}

pub(super) fn run(path: &Path, kind: u8) -> io::Result<Vec<u8>> {
    checkpoint()?;
    let source = crate::platform::platform().retain_archive_source(path)?;
    if source.file.metadata()?.len() > envelope(kind).encoded_bytes {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "Image source exceeds its limit",
        ));
    }
    let service = service(kind)?;
    let (reply, response) = mpsc::channel();
    service
        .sender
        .send(Message::Request(Request {
            source,
            kind,
            reply,
        }))
        .map_err(|_| io::Error::other("Image session stopped"))?;
    response
        .recv()
        .map_err(|_| io::Error::other("Image session stopped"))?
}

fn checkpoint() -> io::Result<()> {
    if STOPPING.load(Ordering::Acquire) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Image session shutting down",
        ));
    }
    super::super::windows_resource_pressure::check()
}

#[cfg(test)]
fn serve(requests: mpsc::Receiver<Message>, executable: &Path) {
    serve_with(requests, executable, PREVIEW)
}

fn serve_with(requests: mpsc::Receiver<Message>, executable: &Path, envelope: Envelope) {
    let mut session: Option<Session> = None;
    loop {
        if STOPPING.load(Ordering::Acquire) {
            break;
        }
        match requests.recv_timeout(Duration::from_secs(1)) {
            Ok(Message::Request(request)) => {
                let started = Instant::now();
                let result = (|| {
                    checkpoint()?;
                    if session.as_mut().is_some_and(|session| !session.reusable()) {
                        session = None;
                    }
                    if session.is_none() {
                        session = Some(Session::start_with(executable, started, envelope)?);
                    }
                    session.as_mut().unwrap().request(
                        &request.source.file,
                        request.kind,
                        started,
                        envelope.deadline,
                    )
                })();
                // Even a well-framed decoder error discards its process. A later
                // request starts fresh; never retry a failing file locally.
                if result.is_err()
                    || session
                        .as_ref()
                        .is_some_and(|session| session.requests >= SESSION_REQUESTS)
                {
                    session = None;
                }
                log::debug!("Image session request completed in {:?}", started.elapsed());
                let _ = request.reply.send(result);
            }
            Ok(Message::Warm) => {
                if session.as_mut().is_some_and(|session| !session.reusable()) {
                    session = None;
                }
                if session.is_none() && checkpoint().is_ok() {
                    session = Session::start_with(executable, Instant::now(), envelope).ok();
                }
            }
            Ok(Message::Release) => session = None,
            Ok(Message::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if session.as_mut().is_some_and(|session| !session.reusable())
                    || checkpoint().is_err()
                {
                    session = None;
                }
            }
        }
    }
    // Worker::drop kills/reaps the child before deleting its private runtime.
    drop(session);
}

pub(super) fn shutdown() {
    STOPPING.store(true, Ordering::Release);
    for cell in [&SERVICE, &VIEWER_SERVICE, &AUDIO_SERVICE] {
        if let Some(Ok(service)) = cell.get() {
            let _ = service.sender.try_send(Message::Stop);
            if let Ok(done) = service.done.lock() {
                if done.recv_timeout(Duration::from_secs(2)).is_err() {
                    log::warn!("Image session cleanup exceeded shutdown deadline; resource job closes on process exit");
                }
            }
        }
    }
}

struct Session {
    worker: super::super::windows_worker_sandbox::Worker,
    output: std::fs::File,
    input: std::fs::File,
    created: Instant,
    last_used: Instant,
    requests: u64,
    envelope: Envelope,
}
impl Session {
    #[cfg(test)]
    fn start(executable: &Path, started: Instant, deadline: Duration) -> io::Result<Self> {
        Self::start_with(executable, started, Envelope { deadline, ..PREVIEW })
    }

    fn start_with(executable: &Path, started: Instant, envelope: Envelope) -> io::Result<Self> {
        let mut command = Command::new(executable);
        command.arg(SESSION_FLAG);
        let checked = || check_deadline(started, envelope.deadline);
        let (worker, output, input) =
            super::super::windows_worker_sandbox::Worker::start_duplex_checked(
                command,
                envelope.memory,
                &checked,
            )?;
        Ok(Self {
            worker,
            output,
            input,
            created: Instant::now(),
            last_used: Instant::now(),
            requests: 0,
            envelope,
        })
    }

    fn reusable(&mut self) -> bool {
        self.requests < SESSION_REQUESTS
            && self.last_used.elapsed() < self.envelope.idle
            && self.created.elapsed() < self.envelope.max_age
            && matches!(self.worker.child.try_wait(), Ok(None))
    }

    fn request(
        &mut self,
        source: &std::fs::File,
        kind: u8,
        started: Instant,
        deadline: Duration,
    ) -> io::Result<Vec<u8>> {
        check_deadline(started, deadline)?;
        if self.requests >= SESSION_REQUESTS || self.worker.child.try_wait()?.is_some() {
            return Err(io::Error::other("Image session no longer available"));
        }
        if available(&self.output)? != 0 {
            return Err(io::Error::other("Unsolicited image session reply"));
        }
        let handle = self.grant_source(source)?;
        self.requests += 1;
        let id = self.requests;
        let mut request = [0; 17];
        request[..8].copy_from_slice(&id.to_le_bytes());
        request[8..16].copy_from_slice(&(handle.0 as usize as u64).to_le_bytes());
        request[16] = kind;
        // At most 32 * 17 bytes are ever written to this >=4KiB pipe. Even a
        // compromised child refusing to read cannot fill it and block the parent.
        self.input.write_all(&request)?;
        let mut header = [0; 12];
        self.read_exact(&mut header, started, deadline)?;
        let length = reply_length_within(header, id, self.envelope.max_reply)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(io::Error::other)?;
        bytes.resize(length, 0);
        self.read_exact(&mut bytes, started, deadline)?;
        let limits = if kind == VIEWER_KIND { preview_protocol::VIEWER }
            else if kind == AUDIO_KIND || kind == AUDIO_THUMBNAIL_KIND {
                let side = if kind == AUDIO_THUMBNAIL_KIND { 128 } else { 512 };
                preview_protocol::Limits { side, pixels: (side * side * 4) as usize }
            } else { preview_protocol::PREVIEW };
        match preview_protocol::decode_with(&bytes, limits)? {
            Reply::Images { frames, .. } => {
                if (kind == AUDIO_KIND || kind == AUDIO_THUMBNAIL_KIND) && frames.len() != 1 {
                    return Err(io::Error::other("Unexpected audio cover frame count"));
                }
            }
            Reply::Text(text) if (kind == AUDIO_KIND || kind == AUDIO_THUMBNAIL_KIND) && text.is_empty() => {}
            // Return the typed error to the UI while ensuring the session is
            // recycled before another document is granted to it.
            Reply::Error { .. } => self.requests = SESSION_REQUESTS,
            _ => return Err(io::Error::other("Unexpected image session reply")),
        }
        self.last_used = Instant::now();
        Ok(bytes)
    }

    fn grant_source(&self, source: &std::fs::File) -> io::Result<HANDLE> {
        let mut handle = HANDLE::default();
        // Source was retained read-only by Platform. No additional rights, no
        // inheritance, and no parent-process handle is granted to the decoder.
        unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                HANDLE(source.as_raw_handle()),
                HANDLE(self.worker.child.as_raw_handle()),
                &mut handle,
                0,
                false,
                DUPLICATE_SAME_ACCESS,
            )
        }
        .map_err(native)?;
        Ok(handle)
    }

    fn read_exact(
        &mut self,
        mut bytes: &mut [u8],
        started: Instant,
        deadline: Duration,
    ) -> io::Result<()> {
        while !bytes.is_empty() {
            check_deadline(started, deadline)?;
            super::super::windows_resource_pressure::check_worker(HANDLE(
                self.worker.child.as_raw_handle(),
            ))?;
            let count = available(&self.output)?.min(bytes.len());
            if count > 0 {
                let read = self.output.read(&mut bytes[..count])?;
                if read == 0 {
                    return Err(io::Error::other("Image session output closed"));
                }
                bytes = &mut bytes[read..];
            } else if self.worker.child.try_wait()?.is_some() {
                return Err(io::Error::other("Image session exited before its reply"));
            } else {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
fn reply_length(header: [u8; 12], expected: u64) -> io::Result<usize> {
    reply_length_within(header, expected, preview_protocol::MAX_REPLY)
}

fn reply_length_within(header: [u8; 12], expected: u64, max_reply: usize) -> io::Result<usize> {
    let id = u64::from_le_bytes(header[..8].try_into().unwrap());
    let length = u32::from_le_bytes(header[8..].try_into().unwrap()) as usize;
    if id != expected || length == 0 || length > max_reply {
        Err(io::Error::other("Invalid image session frame"))
    } else {
        Ok(length)
    }
}

fn check_deadline(started: Instant, deadline: Duration) -> io::Result<()> {
    checkpoint()?;
    if started.elapsed() >= deadline {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "PreviewWorkerTimeout: Image session exceeded its deadline",
        ))
    } else {
        Ok(())
    }
}
fn available(output: &std::fs::File) -> io::Result<usize> {
    let mut count = 0;
    unsafe {
        PeekNamedPipe(
            HANDLE(output.as_raw_handle()),
            None,
            0,
            None,
            Some(&mut count),
            None,
        )
    }
    .map_err(native)?;
    Ok(count as usize)
}

#[cfg(test)]
#[path = "image_session/tests/image_session.rs"]
mod tests;

#[cfg(test)]
#[path = "image_session/tests/audio.rs"]
mod audio_tests;
