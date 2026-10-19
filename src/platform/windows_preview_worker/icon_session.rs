//! Bounded native path sessions. Shell icons and PDF previews use separate processes.
use super::{native, Worker, DEADLINE};
use crate::commands::icon_session_worker::{FLAG, MAX_INPUT, MAX_REPLY, MAX_REQUESTS};
use std::{
    io::{self, Read, Write},
    os::windows::io::AsRawHandle,
    path::Path,
    process::{ChildStdin, Command},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use windows::Win32::{Foundation::HANDLE, System::Pipes::PeekNamedPipe};

static STOPPING: AtomicBool = AtomicBool::new(false);
static REVISION: AtomicU64 = AtomicU64::new(0);
static SERVICE: OnceLock<Result<Service, String>> = OnceLock::new();
static PDF_SERVICE: OnceLock<Result<Service, String>> = OnceLock::new();
const IDLE: Duration = Duration::from_secs(15);
const MEMORY: usize = 256 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Icons,
    Pdf,
}
impl Kind {
    fn max_requests(self) -> u64 {
        if self == Self::Pdf {
            32
        } else {
            MAX_REQUESTS
        }
    }
    fn max_reply(self) -> usize {
        if self == Self::Pdf {
            1024 * 1024 * 4 + 16 * 1024
        } else {
            MAX_REPLY
        }
    }
    fn revision(self) -> u64 {
        if self == Self::Icons {
            REVISION.load(Ordering::Acquire)
        } else {
            0
        }
    }
}

pub(super) fn reset() {
    REVISION.fetch_add(1, Ordering::AcqRel);
}

struct Request {
    path: String,
    large: bool,
    reply: mpsc::Sender<io::Result<Vec<u8>>>,
}
struct Service {
    sender: mpsc::SyncSender<Request>,
    done: Mutex<mpsc::Receiver<()>>,
}

pub(super) fn run(path: &Path, large: bool) -> io::Result<Vec<u8>> {
    run_kind(path, large, Kind::Icons)
}

pub(super) fn run_pdf(path: &Path, thumbnail: bool) -> io::Result<Vec<u8>> {
    // Retain a read-only source while the native renderer opens its path.
    let _source = crate::platform::platform().retain_archive_source(path)?;
    run_kind(path, !thumbnail, Kind::Pdf)
}

fn run_kind(path: &Path, large: bool, kind: Kind) -> io::Result<Vec<u8>> {
    checkpoint()?;
    let path = path
        .to_str()
        .ok_or_else(|| io::Error::other("Native preview path is not Unicode"))?
        .to_owned();
    if path.is_empty() || path.len() + 5 > MAX_INPUT {
        return Err(io::Error::other("Native preview path exceeds session request limit"));
    }
    let cell = if kind == Kind::Pdf {
        &PDF_SERVICE
    } else {
        &SERVICE
    };
    let service = cell
        .get_or_init(|| {
            let (sender, requests) = mpsc::sync_channel(1);
            let (done, completion) = mpsc::channel();
            std::thread::Builder::new()
                .name(
                    if kind == Kind::Pdf {
                        "pdf-preview-session"
                    } else {
                        "shell-icon-session"
                    }
                    .into(),
                )
                .spawn(move || {
                    serve_kind(requests, kind);
                    let _ = done.send(());
                })
                .map_err(|error| error.to_string())?;
            Ok(Service {
                sender,
                done: Mutex::new(completion),
            })
        })
        .as_ref()
        .map_err(|error| io::Error::other(error.clone()))?;
    let (reply, response) = mpsc::channel();
    service
        .sender
        .send(Request { path, large, reply })
        .map_err(|_| io::Error::other("Native preview session stopped"))?;
    response
        .recv()
        .map_err(|_| io::Error::other("Native preview session stopped"))?
}

fn checkpoint() -> io::Result<()> {
    if STOPPING.load(Ordering::Acquire) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Native preview session shutting down",
        ));
    }
    super::super::windows_resource_pressure::check()
}

fn serve_kind(requests: mpsc::Receiver<Request>, kind: Kind) {
    let mut session: Option<Session> = None;
    while !STOPPING.load(Ordering::Acquire) {
        match requests.recv_timeout(Duration::from_millis(100)) {
            Ok(request) => {
                let started = Instant::now();
                let result = (|| {
                    checkpoint()?;
                    let length = request.path.len() + 5;
                    if session
                        .as_mut()
                        .is_some_and(|session| !session.reusable(length))
                    {
                        session = None;
                    }
                    if session.is_none() {
                        session = Some(Session::start_kind(
                            &std::env::current_exe()?,
                            started,
                            kind,
                        )?);
                    }
                    session
                        .as_mut()
                        .unwrap()
                        .request(&request.path, request.large, started)
                })();
                if result.is_err() {
                    session = None;
                }
                let _ = request.reply.send(result);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if session.as_mut().is_some_and(|session| !session.reusable(0)) {
                    session = None;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

pub(super) fn shutdown() {
    STOPPING.store(true, Ordering::Release);
    for cell in [&SERVICE, &PDF_SERVICE] {
        if let Some(Ok(service)) = cell.get() {
            if let Ok(done) = service.done.lock() {
                if done.recv_timeout(Duration::from_secs(2)).is_err() {
                    log::warn!(
                    "Native preview session shutdown exceeded deadline; resource job closes on process exit"
                );
                }
            }
        }
    }
}

struct Session {
    worker: Worker,
    output: std::fs::File,
    input: ChildStdin,
    last_used: Instant,
    requests: u64,
    written: usize,
    revision: u64,
    kind: Kind,
    created: Instant,
}
impl Session {
    #[cfg(test)]
    fn start_in(executable: &Path, started: Instant) -> io::Result<Self> {
        Self::start_kind(executable, started, Kind::Icons)
    }
    fn start_kind(executable: &Path, started: Instant, kind: Kind) -> io::Result<Self> {
        let mut command = Command::new(executable);
        if kind == Kind::Pdf {
            command.args(["--oxyde-preview-worker-v1", "pdf-previews"]);
        } else {
            command.arg(FLAG);
        }
        let memory = if kind == Kind::Pdf {
            512 * 1024 * 1024
        } else {
            MEMORY
        };
        let (worker, output, input) =
            Worker::start_duplex_checked(command, memory, &|| checked(started))?;
        Ok(Self {
            worker,
            output,
            input,
            last_used: Instant::now(),
            requests: 0,
            written: 0,
            revision: kind.revision(),
            kind,
            created: Instant::now(),
        })
    }
    fn reusable(&mut self, next_length: usize) -> bool {
        self.revision == self.kind.revision()
            && self.requests < self.kind.max_requests()
            && self.written + next_length <= MAX_INPUT
            && self.last_used.elapsed() < IDLE
            && (self.kind != Kind::Pdf || self.created.elapsed() < Duration::from_secs(60))
            && matches!(self.worker.child.try_wait(), Ok(None))
    }
    fn request(&mut self, path: &str, large: bool, started: Instant) -> io::Result<Vec<u8>> {
        checked(started)?;
        if !self.reusable(path.len() + 5) {
            return Err(io::Error::other("Native preview session requires recycling"));
        }
        if available(&self.output)? != 0 {
            return Err(io::Error::other("Unsolicited native preview session output"));
        }
        // The sum of all requests fits one stdin pipe, even if a faulty child
        // produces replies without reading requests. Writes cannot fill the pipe.
        let mut packet = Vec::with_capacity(path.len() + 5);
        packet.push(u8::from(large));
        packet.extend_from_slice(&(path.len() as u32).to_le_bytes());
        packet.extend_from_slice(path.as_bytes());
        self.input.write_all(&packet)?;
        self.written += packet.len();
        self.requests += 1;
        let mut header = [0; 12];
        self.read_exact(&mut header, started)?;
        let length = reply_length_with_limit(header, self.requests, self.kind.max_reply())?;
        let mut bytes = vec![0; length];
        self.read_exact(&mut bytes, started)?;
        if self.revision != self.kind.revision() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Icon cache was cleared",
            ));
        }
        let side = if self.kind == Kind::Pdf {
            if large {
                1024
            } else {
                128
            }
        } else {
            256
        };
        let limits = crate::utils::preview_protocol::Limits {
            side,
            pixels: (side * side * 4) as usize,
        };
        match crate::utils::preview_protocol::decode_with(&bytes, limits)? {
            crate::utils::preview_protocol::Reply::Images { frames, .. }
                if frames.len() == 1
                    && frames[0].buffer().width() <= side
                    && frames[0].buffer().height() <= side => {}
            crate::utils::preview_protocol::Reply::Error { detail, .. } => {
                return Err(io::Error::other(detail))
            }
            _ => return Err(io::Error::other("Invalid native preview session pixels")),
        }
        self.last_used = Instant::now();
        Ok(bytes)
    }
    fn read_exact(&mut self, mut bytes: &mut [u8], started: Instant) -> io::Result<()> {
        while !bytes.is_empty() {
            checked(started)?;
            super::super::windows_resource_pressure::check_worker(HANDLE(
                self.worker.child.as_raw_handle(),
            ))?;
            let count = available(&self.output)?.min(bytes.len());
            if count > 0 {
                let count = self.output.read(&mut bytes[..count])?;
                if count == 0 {
                    return Err(io::Error::other("Native preview session output closed"));
                }
                bytes = &mut bytes[count..];
            } else if self.worker.child.try_wait()?.is_some() {
                return Err(io::Error::other("Native preview session exited before reply"));
            } else {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        Ok(())
    }
}

fn checked(started: Instant) -> io::Result<()> {
    checkpoint()?;
    if started.elapsed() >= DEADLINE {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "Native preview session timed out",
        ));
    }
    Ok(())
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
fn reply_length(header: [u8; 12], expected: u64) -> io::Result<usize> {
    reply_length_with_limit(header, expected, MAX_REPLY)
}
fn reply_length_with_limit(header: [u8; 12], expected: u64, max_reply: usize) -> io::Result<usize> {
    let id = u64::from_le_bytes(header[..8].try_into().unwrap());
    let length = u32::from_le_bytes(header[8..].try_into().unwrap()) as usize;
    if id != expected || length == 0 || length > max_reply {
        return Err(io::Error::other("Invalid native preview session frame"));
    }
    Ok(length)
}

#[cfg(test)]
#[path = "icon_session/tests/session.rs"]
mod tests;

#[cfg(test)]
#[path = "icon_session/tests/pdf.rs"]
mod pdf_tests;
