//! Bounded codec work: IPC stays on the calling thread, CPU jobs own only bytes.
use super::super::*;
use std::{collections::VecDeque, io::Cursor, sync::mpsc, thread::JoinHandle};

const MIB: usize = 1024 * 1024;
pub(super) const ZIP_BUFFER: u64 = 256 * MIB as u64;
const ZIP_MEMORY: usize = 768 * MIB;
pub(super) const SOLID_BUFFER: u64 = 128 * MIB as u64;

pub(super) struct Checked<R, T> {
    pub reader: R,
    pub connection: Connection<T>,
    pub since_check: usize,
}
impl<R: Read, T: Exchange> Read for Checked<R, T> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.since_check >= MAX_BLOCK {
            notify(&self.connection, Frame::new(Message::Checkpoint))?;
            self.since_check = 0;
        }
        let count = self.reader.read(bytes)?;
        self.since_check += count;
        Ok(count)
    }
}

pub(super) fn threads(turbo: bool) -> usize {
    if turbo {
        std::thread::available_parallelism()
            .map_or(1, usize::from)
            .min(16)
    } else {
        1
    }
}

pub(super) fn seven_options(
    level: u32,
    turbo: bool,
) -> sevenz_rust2::encoder_options::Lzma2Options {
    use sevenz_rust2::encoder_options::Lzma2Options;
    // Keep dictionaries and each thread's match finder below the existing job budget.
    let dictionary = match level {
        0..=1 => MIB,
        2..=6 => 16 * MIB,
        _ => 32 * MIB,
    };
    // Reserve room in the 2 GiB 7z encode job for source/result queues and codec/runtime
    // overhead as well as the dictionary/match finder estimate used here.
    let count = threads(turbo).min((1536 * MIB / (dictionary * 12 + 32 * MIB)).max(1));
    let mut options = if count > 1 {
        Lzma2Options::from_level_mt(level, count as u32, (dictionary * 2).max(8 * MIB) as u64)
    } else {
        Lzma2Options::from_level(level)
    };
    options.set_dictionary_size(dictionary as u32);
    options
}

struct Job {
    memory: usize,
    handle: Option<JoinHandle<()>>,
    result: mpsc::Receiver<io::Result<Vec<u8>>>,
}
impl Drop for Job {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub(super) struct ZipJobs {
    jobs: VecDeque<Job>,
    memory: usize,
}
impl ZipJobs {
    pub(super) fn new() -> Self {
        Self {
            jobs: VecDeque::new(),
            memory: 0,
        }
    }
    pub(super) fn full(&self, size: u64, count: usize) -> bool {
        !self.jobs.is_empty()
            && (self.jobs.len() >= count
                || self.memory as u64 + size * 3 + 8 * MIB as u64 > ZIP_MEMORY as u64)
    }
    pub(super) fn push(&mut self, entry: Entry, bytes: Vec<u8>, level: i64) -> io::Result<()> {
        // Vec growth can retain twice the encoded capacity for incompressible
        // DEFLATE data. Include it instead of assuming output <= source bytes.
        let memory = bytes.len() * 3 + 8 * MIB;
        let (sender, result) = mpsc::sync_channel(1);
        let handle = std::thread::Builder::new()
            .name("zip-encode".into())
            .spawn(move || {
                let result = (|| {
                    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::with_capacity(
                        bytes.len() + bytes.len() / 100 + 65536,
                    )));
                    let options = zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated)
                        .compression_level(Some(level));
                    if entry.directory {
                        writer
                            .add_directory(
                                format!("{}/", entry.name.trim_end_matches('/')),
                                options,
                            )
                            .map_err(io::Error::other)?;
                    } else {
                        writer
                            .start_file(entry.name, options)
                            .map_err(io::Error::other)?;
                        writer.write_all(&bytes)?;
                    }
                    Ok(writer.finish().map_err(io::Error::other)?.into_inner())
                })();
                let _ = sender.send(result);
            })?;
        self.memory += memory;
        self.jobs.push_back(Job {
            memory,
            handle: Some(handle),
            result,
        });
        Ok(())
    }
    pub(super) fn drain_one<W: Write + Seek>(
        &mut self,
        writer: &mut zip::ZipWriter<W>,
        checkpoint: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<()> {
        let Some(mut job) = self.jobs.pop_front() else {
            return Ok(());
        };
        let bytes = loop {
            checkpoint()?;
            match job.result.recv_timeout(Duration::from_millis(5)) {
                Ok(result) => break result?,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::other("ZIP encoder disconnected"))
                }
            }
        };
        job.handle
            .take()
            .unwrap()
            .join()
            .map_err(|_| io::Error::other("ZIP encoder panicked"))?;
        self.memory -= job.memory;
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(io::Error::other)?;
        let file = archive.by_index(0).map_err(io::Error::other)?;
        writer.raw_copy_file(file).map_err(io::Error::other)
    }
    pub(super) fn drain<W: Write + Seek>(
        &mut self,
        writer: &mut zip::ZipWriter<W>,
        checkpoint: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<()> {
        while !self.jobs.is_empty() {
            self.drain_one(writer, checkpoint)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/parallel.rs"]
mod tests;
