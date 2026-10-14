//! Dedicated Shell session; ordinary cached icons never need a process.
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
const IDLE: Duration = Duration::from_secs(15);
const MEMORY: usize = 256 * 1024 * 1024;

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
    checkpoint()?;
    let path = path
        .to_str()
        .ok_or_else(|| io::Error::other("Icon path is not Unicode"))?
        .to_owned();
    if path.is_empty() || path.len() + 5 > MAX_INPUT {
        return Err(io::Error::other("Icon path exceeds session request limit"));
    }
    let service = SERVICE
        .get_or_init(|| {
            let (sender, requests) = mpsc::sync_channel(1);
            let (done, completion) = mpsc::channel();
            std::thread::Builder::new()
                .name("shell-icon-session".into())
                .spawn(move || {
                    serve(requests);
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
        .map_err(|_| io::Error::other("Icon session stopped"))?;
    response
        .recv()
        .map_err(|_| io::Error::other("Icon session stopped"))?
}

fn checkpoint() -> io::Result<()> {
    if STOPPING.load(Ordering::Acquire) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Icon session shutting down",
        ));
    }
    super::super::windows_resource_pressure::check()
}

fn serve(requests: mpsc::Receiver<Request>) {
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
                        session = Some(Session::start(started)?);
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
    if let Some(Ok(service)) = SERVICE.get() {
        if let Ok(done) = service.done.lock() {
            if done.recv_timeout(Duration::from_secs(2)).is_err() {
                log::warn!(
                    "Icon session shutdown exceeded deadline; resource job closes on process exit"
                );
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
}
impl Session {
    fn start(started: Instant) -> io::Result<Self> {
        Self::start_in(&std::env::current_exe()?, started)
    }
    fn start_in(executable: &Path, started: Instant) -> io::Result<Self> {
        let mut command = Command::new(executable);
        command.arg(FLAG);
        let (worker, output, input) =
            Worker::start_duplex_checked(command, MEMORY, &|| checked(started))?;
        Ok(Self {
            worker,
            output,
            input,
            last_used: Instant::now(),
            requests: 0,
            written: 0,
            revision: REVISION.load(Ordering::Acquire),
        })
    }
    fn reusable(&mut self, next_length: usize) -> bool {
        self.revision == REVISION.load(Ordering::Acquire)
            && self.requests < MAX_REQUESTS
            && self.written + next_length <= MAX_INPUT
            && self.last_used.elapsed() < IDLE
            && matches!(self.worker.child.try_wait(), Ok(None))
    }
    fn request(&mut self, path: &str, large: bool, started: Instant) -> io::Result<Vec<u8>> {
        checked(started)?;
        if !self.reusable(path.len() + 5) {
            return Err(io::Error::other("Icon session requires recycling"));
        }
        if available(&self.output)? != 0 {
            return Err(io::Error::other("Unsolicited icon session output"));
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
        let length = reply_length(header, self.requests)?;
        let mut bytes = vec![0; length];
        self.read_exact(&mut bytes, started)?;
        if self.revision != REVISION.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Icon cache was cleared",
            ));
        }
        match crate::utils::preview_protocol::decode(&bytes)? {
            crate::utils::preview_protocol::Reply::Images { frames, .. }
                if frames.len() == 1
                    && frames[0].buffer().width() <= 256
                    && frames[0].buffer().height() <= 256 => {}
            crate::utils::preview_protocol::Reply::Error { detail, .. } => {
                return Err(io::Error::other(detail))
            }
            _ => return Err(io::Error::other("Invalid icon session pixels")),
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
                    return Err(io::Error::other("Icon session output closed"));
                }
                bytes = &mut bytes[count..];
            } else if self.worker.child.try_wait()?.is_some() {
                return Err(io::Error::other("Icon session exited before reply"));
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
            "Icon session timed out",
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
fn reply_length(header: [u8; 12], expected: u64) -> io::Result<usize> {
    let id = u64::from_le_bytes(header[..8].try_into().unwrap());
    let length = u32::from_le_bytes(header[8..].try_into().unwrap()) as usize;
    if id != expected || length == 0 || length > MAX_REPLY {
        return Err(io::Error::other("Invalid icon session frame"));
    }
    Ok(length)
}

#[cfg(test)]
#[path = "icon_session/tests/session.rs"]
mod tests;
