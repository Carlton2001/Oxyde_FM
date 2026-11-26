//! Isolated data-copy benchmark. Never used by normal file operations/history.
use super::{copy_output::Output, path_parents::HeldParents};
use crate::platform::platform;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

fn at(step: &str, error: io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("{step}: {error}"))
}

#[derive(Clone, Copy, Debug)]
pub enum Mode {
    NativeHash,
    Native,
    StreamHash,
    Stream,
    NativeSourceHash,
}
impl Mode {
    pub const ALL: [Self; 5] = [
        Self::NativeHash,
        Self::Native,
        Self::StreamHash,
        Self::Stream,
        Self::NativeSourceHash,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::NativeHash => "native_blake3",
            Self::Native => "native_plain",
            Self::StreamHash => "stream_blake3",
            Self::Stream => "stream_plain",
            Self::NativeSourceHash => "native_source_blake3",
        }
    }
}

#[derive(Default, Debug)]
pub struct Measurement {
    pub bytes: u64,
    pub total: Duration,
    /// Includes native flushes, or inline BLAKE3 for the streaming engine.
    pub transfer: Duration,
    /// Explicit streaming flush only; native flushes are inside transfer.
    pub flush: Duration,
    pub post_hash: Duration,
    /// Source hashing runs concurrently with native copy; not additive.
    pub source_hash: Duration,
    /// Join wait after native copy, before opening/publishing the output.
    pub hash_wait: Duration,
    pub publish: Duration,
    pub digest: Option<String>,
}

/// Destination must be absent and its parent must already exist.
/// Sources are held using the same Platform abstraction as production copies.
pub fn copy_one(source: &Path, destination: &Path, mode: Mode) -> io::Result<Measurement> {
    let started = Instant::now();
    let source_parents = HeldParents::new(source).map_err(|e| at("hold source parents", e))?;
    let source = source_parents.resolved_path(source)?;
    let mut input = platform().open_archive_source(&source, false)?;
    let metadata = input.metadata()?;
    if !metadata.is_file() || platform().file_attributes(&metadata, "").2 {
        return Err(io::Error::other("Benchmark requires ordinary files"));
    }
    let parents = HeldParents::new(destination).map_err(|e| at("hold destination parents", e))?;
    let destination = parents.resolved_path(destination)?;
    let written = Output::new(&parents, &destination, false).map_err(|e| at("prepare output", e))?;
    let mut result = Measurement {
        bytes: metadata.len(),
        ..Measurement::default()
    };
    let mut buffer = vec![0; 1024 * 1024];
    let transfer = Instant::now();
    match mode {
        Mode::NativeHash | Mode::Native => {
            platform()
                .copy_file_with_progress(
                    &source,
                    written.path(),
                    Some(&input),
                    parents.directory(),
                    &mut |file| written.capture(file),
                    &mut |_| Ok(()),
                )
                .map_err(|e| at("native copy/flush", e))?;
            result.transfer = transfer.elapsed();
        }
        Mode::NativeSourceHash => {
            // Independent reader: never share a mutable file offset with copying.
            // Both read handles deny source writes/deletion on Windows. The
            // original input remains held through publication as well.
            let mut reader = platform().open_archive_source(&source, false)?;
            if platform().file_identity(&reader)? != platform().file_identity(&input)? {
                return Err(io::Error::other("Source changed before parallel hashing"));
            }
            let parallel = run_parallel(
                || {
                    platform().copy_file_with_progress(
                        &source,
                        written.path(),
                        Some(&input),
                        parents.directory(),
                        &mut |file| written.capture(file),
                        &mut |_| Ok(()),
                    )
                },
                || digest_reader(&mut reader),
            )?;
            result.transfer = parallel.transfer;
            result.source_hash = parallel.hash;
            result.hash_wait = parallel.wait;
            result.digest = Some(parallel.digest);
        }
        Mode::StreamHash | Mode::Stream => {
            let mut output =
                platform().create_operation_output(parents.directory(), written.path())?;
            written.capture(&output)?;
            let mut hasher = matches!(mode, Mode::StreamHash).then(blake3::Hasher::new);
            let mut bytes = 0_u64;
            loop {
                let count = input.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                output.write_all(&buffer[..count])?;
                if let Some(hasher) = &mut hasher {
                    hasher.update(&buffer[..count]);
                }
                bytes += count as u64;
            }
            result.digest = hasher.map(|hash| hash.finalize().to_hex().to_string());
            result.transfer = transfer.elapsed();
            let flush = Instant::now();
            output.sync_all()?;
            result.flush = flush.elapsed();
            if bytes != result.bytes {
                return Err(io::Error::other("Source size changed"));
            }
        }
    }
    let mut output = written
        .open_verified()
        .map_err(|e| at("open written output", e))?;
    if output.metadata()?.len() != result.bytes {
        return Err(io::Error::other("Copy size mismatch"));
    }
    if matches!(mode, Mode::NativeHash) {
        let hash_started = Instant::now();
        let mut hasher = blake3::Hasher::new();
        loop {
            let count = output
                .read(&mut buffer[..128 * 1024])
                .map_err(|e| at("hash destination", e))?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        result.digest = Some(hasher.finalize().to_hex().to_string());
        result.post_hash = hash_started.elapsed();
    }
    let publish = Instant::now();
    written
        .keep(&parents, &output, false)
        .map_err(|e| at("keep output", e))?;
    result.publish = publish.elapsed();
    drop(output);
    drop(written);
    result.total = started.elapsed();
    Ok(result)
}

/// Independent validation, deliberately excluded from the measured copy.
pub fn digest(path: &Path) -> io::Result<String> {
    let mut input = File::open(path)?;
    digest_reader(&mut input)
}

fn digest_reader(input: &mut File) -> io::Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

struct ParallelResult {
    transfer: Duration,
    hash: Duration,
    wait: Duration,
    digest: String,
}

/// Always join the reader, including on copy failure, before output cleanup.
fn run_parallel(
    copy: impl FnOnce() -> io::Result<()>,
    hash: impl FnOnce() -> io::Result<String> + Send,
) -> io::Result<ParallelResult> {
    std::thread::scope(|scope| {
        let reader = std::thread::Builder::new()
            .name("copy-benchmark-hash".into())
            .spawn_scoped(scope, || {
                let started = Instant::now();
                let result = hash();
                (started.elapsed(), result)
            })?;
        let started = Instant::now();
        let copied = copy();
        let transfer = started.elapsed();
        let waiting = Instant::now();
        let hashed = reader.join();
        let wait = waiting.elapsed();
        copied.map_err(|error| at("native copy/flush", error))?;
        let (hash, digest) = hashed.map_err(|_| io::Error::other("Source hash thread panicked"))?;
        Ok(ParallelResult {
            transfer,
            hash,
            wait,
            digest: digest.map_err(|error| at("hash source", error))?,
        })
    })
}

#[cfg(test)]
#[path = "copy_benchmark/tests/copy_tests.rs"]
mod tests;
