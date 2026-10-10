//! Cooperative resource limits shared by archive extraction paths.
use super::archive_security::relative_archive_path;
use crate::models::error::{ArchiveLimitFailure, ArchiveLimitKind};
use std::cell::Cell;
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
#[doc(hidden)]
pub struct ArchiveLimits {
    pub max_entries: usize,
    pub max_bytes: u64,
    pub max_depth: usize,
    pub max_duration: Duration,
}

impl ArchiveLimits {
    pub const EXTRACTION: Self = Self {
        max_entries: 100_000,
        max_bytes: 64 * 1024 * 1024 * 1024,
        max_depth: 64,
        max_duration: Duration::from_secs(30 * 60),
    };
    pub const WORKSPACE: Self = Self {
        max_bytes: 8 * 1024 * 1024 * 1024,
        ..Self::EXTRACTION
    };
    pub const LISTING: Self = Self {
        max_duration: Duration::from_secs(120),
        ..Self::WORKSPACE
    };
    pub const OPEN_FILE: Self = Self {
        max_bytes: 256 * 1024 * 1024,
        max_duration: Duration::from_secs(120),
        ..Self::EXTRACTION
    };
}

pub(crate) struct ArchiveBudget {
    limits: ArchiveLimits,
    started: Instant,
    checkpoint_time: Cell<Duration>,
    entries: Cell<usize>,
    declared: Cell<u64>,
    written: Cell<u64>,
    decoded: Cell<u64>,
}

fn exceeded(kind: ArchiveLimitKind, limit: u64) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        ArchiveLimitFailure { kind, limit },
    )
}

impl ArchiveBudget {
    pub(crate) fn checkpoint_mut(
        &self,
        checkpoint: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        self.check_time()?;
        let started = Instant::now();
        let result = checkpoint();
        self.checkpoint_time
            .set(self.checkpoint_time.get().saturating_add(started.elapsed()));
        result?;
        self.check_time()
    }
    #[cfg(all(target_os = "windows", not(test)))]
    pub(crate) fn remaining(&self) -> io::Result<Duration> {
        self.check_time()?;
        Ok(self.limits.max_duration.saturating_sub(
            self.started
                .elapsed()
                .saturating_sub(self.checkpoint_time.get()),
        ))
    }
    #[cfg(all(target_os = "windows", not(test)))]
    pub(crate) fn exclude_wait(&self, wait: Duration) {
        self.checkpoint_time
            .set(self.checkpoint_time.get().saturating_add(wait));
    }
    pub(crate) fn limits(&self) -> ArchiveLimits {
        self.limits
    }
    pub fn new(limits: ArchiveLimits) -> Self {
        Self {
            limits,
            started: Instant::now(),
            checkpoint_time: Cell::new(Duration::ZERO),
            entries: Cell::new(0),
            declared: Cell::new(0),
            written: Cell::new(0),
            decoded: Cell::new(0),
        }
    }

    pub fn checkpoint(&self, checkpoint: &dyn Fn() -> io::Result<()>) -> io::Result<()> {
        self.check_time()?;
        let start = Instant::now();
        let result = checkpoint();
        // User pauses inside the checkpoint do not consume the extraction deadline.
        self.checkpoint_time
            .set(self.checkpoint_time.get().saturating_add(start.elapsed()));
        result?;
        self.check_time()
    }

    /// Reader consumers retry Interrupted as a transient IO failure. A stop
    /// requested by the application must escape those retry loops immediately.
    pub fn reader_checkpoint(&self, checkpoint: &dyn Fn() -> io::Result<()>) -> io::Result<()> {
        self.checkpoint(checkpoint).map_err(|error| {
            if error.kind() == io::ErrorKind::Interrupted {
                io::Error::other(error)
            } else {
                error
            }
        })
    }

    pub fn check_time(&self) -> io::Result<()> {
        if self
            .started
            .elapsed()
            .saturating_sub(self.checkpoint_time.get())
            >= self.limits.max_duration
        {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                ArchiveLimitFailure {
                    kind: ArchiveLimitKind::Seconds,
                    limit: self.limits.max_duration.as_secs(),
                },
            ));
        }
        Ok(())
    }

    pub fn check_entry_count(&self, count: usize) -> io::Result<()> {
        if count > self.limits.max_entries {
            return Err(exceeded(
                ArchiveLimitKind::Entries,
                self.limits.max_entries as u64,
            ));
        }
        Ok(())
    }

    pub fn entry(&self, name: &str, size: u64) -> io::Result<()> {
        self.check_time()?;
        let path = relative_archive_path(name)?;
        if path.components().count() > self.limits.max_depth {
            return Err(exceeded(
                ArchiveLimitKind::Depth,
                self.limits.max_depth as u64,
            ));
        }
        let count =
            self.entries.get().checked_add(1).ok_or_else(|| {
                exceeded(ArchiveLimitKind::Entries, self.limits.max_entries as u64)
            })?;
        self.check_entry_count(count)?;
        self.declare_bytes(size)?;
        self.entries.set(count);
        Ok(())
    }

    pub fn declare_bytes(&self, size: u64) -> io::Result<()> {
        let total = self
            .declared
            .get()
            .checked_add(size)
            .ok_or_else(|| exceeded(ArchiveLimitKind::Bytes, self.limits.max_bytes))?;
        if total > self.limits.max_bytes {
            return Err(exceeded(ArchiveLimitKind::Bytes, self.limits.max_bytes));
        }
        self.declared.set(total);
        Ok(())
    }

    pub fn charge_written(&self, count: usize) -> io::Result<()> {
        self.check_time()?;
        let total = self
            .written
            .get()
            .checked_add(count as u64)
            .ok_or_else(|| exceeded(ArchiveLimitKind::Bytes, self.limits.max_bytes))?;
        if total > self.limits.max_bytes {
            return Err(exceeded(ArchiveLimitKind::Bytes, self.limits.max_bytes));
        }
        self.written.set(total);
        Ok(())
    }

    pub fn copy(
        &self,
        reader: &mut (impl Read + ?Sized),
        writer: &mut impl Write,
        checkpoint: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<u64> {
        let mut buffer = [0u8; 64 * 1024];
        let mut copied = 0;
        loop {
            self.checkpoint(checkpoint)?;
            let remaining = self.limits.max_bytes - self.written.get();
            let length = buffer
                .len()
                .min(remaining.saturating_add(1).min(usize::MAX as u64) as usize);
            let count = reader.read(&mut buffer[..length])?;
            self.check_time()?;
            if count == 0 {
                return Ok(copied);
            }
            self.charge_written(count)?;
            writer.write_all(&buffer[..count])?;
            copied += count as u64;
        }
    }

    // TAR also consumes unselected entries, padding and extended metadata. Bound
    // that stream separately, with an allowance for headers of legitimate entries.
    fn decoded_limit(&self) -> u64 {
        self.limits
            .max_bytes
            .saturating_add((self.limits.max_entries as u64).saturating_mul(4096))
    }
}

pub(crate) struct BudgetReader<'a, R> {
    pub reader: R,
    pub budget: &'a ArchiveBudget,
    pub checkpoint: &'a dyn Fn() -> io::Result<()>,
}
impl<R: Read> Read for BudgetReader<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        self.budget.reader_checkpoint(self.checkpoint)?;
        let remaining = self.budget.decoded_limit() - self.budget.decoded.get();
        let length = buffer
            .len()
            .min(remaining.saturating_add(1).min(usize::MAX as u64) as usize);
        let count = self.reader.read(&mut buffer[..length])?;
        self.budget.check_time()?;
        if count as u64 > remaining {
            return Err(exceeded(
                ArchiveLimitKind::Decoded,
                self.budget.decoded_limit(),
            ));
        }
        self.budget
            .decoded
            .set(self.budget.decoded.get() + count as u64);
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retrying_consumers_do_not_swallow_a_checkpoint_cancellation() {
        let mut observed = Vec::new();
        for consumer in ["read_exact", "read_to_end", "copy"] {
            let budget = ArchiveBudget::new(limits());
            let calls = Cell::new(0);
            let checkpoint = || {
                calls.set(calls.get() + 1);
                if calls.get() == 1 {
                    Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled once"))
                } else {
                    Ok(())
                }
            };
            let mut source = io::Cursor::new(b"safe");
            let mut reader = BudgetReader {
                reader: &mut source,
                budget: &budget,
                checkpoint: &checkpoint,
            };
            let result = match consumer {
                "read_exact" => reader.read_exact(&mut [0; 4]),
                "read_to_end" => reader.read_to_end(&mut Vec::new()).map(|_| ()),
                _ => io::copy(&mut reader, &mut io::sink()).map(|_| ()),
            };
            if let Err(error) = &result {
                assert_eq!(error.kind(), io::ErrorKind::Other);
                let cause = error
                    .get_ref()
                    .unwrap()
                    .downcast_ref::<io::Error>()
                    .unwrap();
                assert_eq!(cause.kind(), io::ErrorKind::Interrupted);
                assert_eq!(cause.to_string(), "cancelled once");
            }
            observed.push((consumer, result.is_err(), calls.get(), source.position()));
        }
        assert!(
            observed
                .iter()
                .all(|(_, failed, calls, position)| *failed && *calls == 1 && *position == 0),
            "checkpoint cancellation was retried: {observed:?}"
        );
    }

    #[test]
    fn retrying_consumers_still_retry_underlying_io_interruptions() {
        struct Source {
            bytes: io::Cursor<&'static [u8]>,
            interrupted: bool,
        }
        impl Read for Source {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                if !self.interrupted {
                    self.interrupted = true;
                    Err(io::Error::new(io::ErrorKind::Interrupted, "transient IO"))
                } else {
                    self.bytes.read(bytes)
                }
            }
        }
        let budget = ArchiveBudget::new(limits());
        let calls = Cell::new(0);
        let checkpoint = || {
            calls.set(calls.get() + 1);
            Ok(())
        };
        let mut reader = BudgetReader {
            reader: Source {
                bytes: io::Cursor::new(b"safe"),
                interrupted: false,
            },
            budget: &budget,
            checkpoint: &checkpoint,
        };
        assert_eq!(reader.read(&mut []).unwrap(), 0);
        assert_eq!(calls.get(), 0);
        let mut bytes = [0; 4];
        reader.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"safe");
        assert_eq!(calls.get(), 2);
        assert_eq!(budget.decoded.get(), 4);
    }

    fn limits() -> ArchiveLimits {
        ArchiveLimits {
            max_bytes: 8,
            max_entries: 2,
            max_depth: 2,
            ..ArchiveLimits::EXTRACTION
        }
    }
    #[test]
    fn failures_preserve_the_resource_and_threshold_through_wrapped_errors() {
        let budget = ArchiveBudget::new(limits());
        let errors = [
            (
                budget.entry("a/b/c", 0).unwrap_err(),
                ArchiveLimitKind::Depth,
                2,
            ),
            (
                budget.check_entry_count(3).unwrap_err(),
                ArchiveLimitKind::Entries,
                2,
            ),
            (
                budget.declare_bytes(9).unwrap_err(),
                ArchiveLimitKind::Bytes,
                8,
            ),
        ];
        for (error, kind, limit) in errors {
            let wrapped = format!("Archive Error: decoder failed: {error}");
            assert_eq!(
                ArchiveLimitFailure::from_message(&wrapped),
                Some(ArchiveLimitFailure { kind, limit })
            );
        }
        let expired = ArchiveBudget::new(ArchiveLimits {
            max_duration: Duration::ZERO,
            ..limits()
        });
        let error = expired.check_time().unwrap_err();
        assert_eq!(
            ArchiveLimitFailure::from_message(&error.to_string()),
            Some(ArchiveLimitFailure {
                kind: ArchiveLimitKind::Seconds,
                limit: 0
            })
        );
        let mut source = io::Cursor::new(b"safe");
        let mut expired_reader = BudgetReader {
            reader: &mut source,
            budget: &expired,
            checkpoint: &|| Ok(()),
        };
        let error = expired_reader.read_exact(&mut [0; 4]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert_eq!(
            ArchiveLimitFailure::from_message(&error.to_string()),
            Some(ArchiveLimitFailure {
                kind: ArchiveLimitKind::Seconds,
                limit: 0,
            })
        );
        assert_eq!(source.position(), 0);
        let mut reader = BudgetReader {
            reader: io::repeat(0),
            budget: &budget,
            checkpoint: &|| Ok(()),
        };
        let error = reader.read_to_end(&mut Vec::new()).unwrap_err();
        assert_eq!(
            ArchiveLimitFailure::from_message(&error.to_string()),
            Some(ArchiveLimitFailure {
                kind: ArchiveLimitKind::Decoded,
                limit: 8 + 2 * 4096
            })
        );
        assert!(ArchiveLimitFailure::from_message("Unexpected end of archive").is_none());
    }
    #[test]
    fn enforces_counts_depth_and_declared_total() {
        let budget = ArchiveBudget::new(limits());
        assert!(budget.entry("a/b/c", 0).is_err());
        budget.entry("a", 4).unwrap();
        assert!(budget.entry("b", u64::MAX).is_err());
        budget.entry("b", 4).unwrap();
        assert!(budget.entry("c", 0).is_err());
    }
    #[test]
    fn actual_output_is_bounded_even_when_sizes_are_wrong() {
        let budget = ArchiveBudget::new(limits());
        let mut output = Vec::new();
        budget
            .copy(&mut &b"1234"[..], &mut output, &|| Ok(()))
            .unwrap();
        assert!(budget
            .copy(&mut io::repeat(0), &mut output, &|| Ok(()))
            .is_err());
        assert!(output.len() <= 8);
    }
    #[test]
    fn cancellation_stops_a_copy_after_initial_output() {
        let budget = ArchiveBudget::new(ArchiveLimits {
            max_bytes: 1024 * 1024,
            ..limits()
        });
        let calls = Cell::new(0);
        let mut output = Vec::new();
        let result = budget.copy(&mut io::repeat(0), &mut output, &|| {
            calls.set(calls.get() + 1);
            if calls.get() > 1 {
                Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
        assert_eq!(output.len(), 64 * 1024);
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn limits_skipped_streams_deadline_and_cancellation() {
        let budget = ArchiveBudget::new(limits());
        let mut reader = BudgetReader {
            reader: io::repeat(0),
            budget: &budget,
            checkpoint: &|| Ok(()),
        };
        let mut output = Vec::new();
        assert!(reader.read_to_end(&mut output).is_err());
        assert!(output.len() <= 8 + 2 * 4096);
        let expired = ArchiveBudget::new(ArchiveLimits {
            max_duration: Duration::ZERO,
            ..limits()
        });
        assert_eq!(
            expired.check_time().unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        assert!(budget
            .copy(&mut io::repeat(0), &mut Vec::new(), &|| Err(
                io::Error::other("cancelled")
            ))
            .is_err());
    }
}
