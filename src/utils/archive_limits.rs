//! Cooperative resource limits shared by archive extraction paths.
use super::archive_security::relative_archive_path;
use std::cell::Cell;
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub(crate) struct ArchiveLimits {
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

fn exceeded(resource: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("Archive limit exceeded: {resource}"),
    )
}

impl ArchiveBudget {
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

    pub fn check_time(&self) -> io::Result<()> {
        if self
            .started
            .elapsed()
            .saturating_sub(self.checkpoint_time.get())
            >= self.limits.max_duration
        {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Archive processing time limit exceeded",
            ));
        }
        Ok(())
    }

    pub fn check_entry_count(&self, count: usize) -> io::Result<()> {
        if count > self.limits.max_entries {
            return Err(exceeded("entry count"));
        }
        Ok(())
    }

    pub fn entry(&self, name: &str, size: u64) -> io::Result<()> {
        self.check_time()?;
        let path = relative_archive_path(name)?;
        if path.components().count() > self.limits.max_depth {
            return Err(exceeded("path depth"));
        }
        let count = self
            .entries
            .get()
            .checked_add(1)
            .ok_or_else(|| exceeded("entry count"))?;
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
            .ok_or_else(|| exceeded("declared bytes"))?;
        if total > self.limits.max_bytes {
            return Err(exceeded("declared bytes"));
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
            .ok_or_else(|| exceeded("extracted bytes"))?;
        if total > self.limits.max_bytes {
            return Err(exceeded("extracted bytes"));
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
        self.budget.checkpoint(self.checkpoint)?;
        let remaining = self.budget.decoded_limit() - self.budget.decoded.get();
        let length = buffer
            .len()
            .min(remaining.saturating_add(1).min(usize::MAX as u64) as usize);
        let count = self.reader.read(&mut buffer[..length])?;
        self.budget.check_time()?;
        if count as u64 > remaining {
            return Err(exceeded("decoded stream bytes"));
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
    fn limits() -> ArchiveLimits {
        ArchiveLimits {
            max_bytes: 8,
            max_entries: 2,
            max_depth: 2,
            ..ArchiveLimits::EXTRACTION
        }
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
