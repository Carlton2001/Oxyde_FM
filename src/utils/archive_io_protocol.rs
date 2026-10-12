//! Versioned archive broker frames. Only session-local resource IDs cross the wire.
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

pub const MAX_BLOCK: usize = 256 * 1024;
pub const MAX_FRAME: usize = 1024 * 1024;
pub const MAX_NAME: usize = 32 * 1024;
pub const MAX_ERROR: usize = 16 * 1024;
pub const MAX_METADATA: usize = 256 * 1024;
pub const HEADER: usize = 40;
const MAGIC: &[u8; 4] = b"OXAI";
const VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Default)]
pub struct Control {
    pub cancelled: bool,
    pub paused: bool,
    pub turbo: bool,
}

thread_local! {
    static TEST_EXECUTABLE: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Scoped integration-test injection; Windows production uses the dedicated
/// archive executable next to the application, never a local-codec fallback.
#[doc(hidden)]
pub fn with_worker_executable_for_tests<T>(path: &std::path::Path, run: impl FnOnce() -> T) -> T {
    struct Restore(Option<std::path::PathBuf>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_EXECUTABLE.with(|value| *value.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(TEST_EXECUTABLE.with(|value| value.replace(Some(path.into()))));
    run()
}

#[cfg(target_os = "windows")]
#[doc(hidden)]
pub fn worker_executable() -> std::io::Result<std::path::PathBuf> {
    TEST_EXECUTABLE
        .with(|value| value.borrow().clone())
        .map_or_else(
            || {
                Ok(std::env::current_exe()?
                    .with_file_name("oxyde-archive-io-worker.exe"))
            },
            Ok,
        )
}

pub fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid archive broker protocol",
    )
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Format {
    Zip,
    SevenZip,
    Tar,
    TarGz,
    TarXz,
    TarBz2,
    TarZst,
    Iso,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Quality {
    Fast,
    Normal,
    Best,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub entries: u64,
    pub bytes: u64,
    pub depth: u64,
    pub seconds: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", deny_unknown_fields)]
pub enum Operation {
    Decode {
        format: Format,
        source_size: u64,
        selected: Option<String>,
        limits: Limits,
    },
    Encode {
        format: Format,
        quality: Quality,
        entries: u64,
        output_limit: u64,
        limits: Limits,
    },
}

/// Seconds relative to UNIX_EPOCH; subsecond precision does not depend on the OS.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Timestamp {
    pub seconds: i64,
    pub nanos: u32,
}

impl Timestamp {
    pub fn from_system(time: std::time::SystemTime) -> io::Result<Self> {
        let nanos = match time.duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos() as i128,
            Err(error) => -(error.duration().as_nanos() as i128),
        };
        Ok(Self {
            seconds: nanos
                .div_euclid(1_000_000_000)
                .try_into()
                .map_err(|_| invalid())?,
            nanos: nanos.rem_euclid(1_000_000_000) as u32,
        })
    }

    pub fn system(self) -> io::Result<std::time::SystemTime> {
        if self.nanos >= 1_000_000_000 {
            return Err(invalid());
        }
        let nanos = self.seconds as i128 * 1_000_000_000 + self.nanos as i128;
        let magnitude = nanos.unsigned_abs();
        let duration = std::time::Duration::new(
            (magnitude / 1_000_000_000) as u64,
            (magnitude % 1_000_000_000) as u32,
        );
        if nanos >= 0 {
            std::time::UNIX_EPOCH.checked_add(duration)
        } else {
            std::time::UNIX_EPOCH.checked_sub(duration)
        }
        .ok_or_else(invalid)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: u64,
    pub name: String,
    pub directory: bool,
    pub size: u64,
    pub mode: u32,
    pub modified: Option<Timestamp>,
    pub created: Option<Timestamp>,
    pub accessed: Option<Timestamp>,
}

impl Entry {
    pub fn validate(&self) -> io::Result<()> {
        if self.directory
            && self.name.len() <= MAX_NAME
            && super::archive_security::relative_archive_path(&self.name)?
                .as_os_str()
                .is_empty()
        {
            // Preserve harmless root-directory markers found in TAR and 7z.
        } else {
            validate_name(&self.name)?;
        }
        if (self.directory && self.size != 0)
            || self.mode & !0o7777 != 0
            || [self.modified, self.created, self.accessed]
                .into_iter()
                .flatten()
                .any(|t| t.nanos >= 1_000_000_000)
        {
            return Err(invalid());
        }
        Ok(())
    }
}

fn validate_name(name: &str) -> io::Result<()> {
    if name.is_empty() || name.len() > MAX_NAME {
        return Err(invalid());
    }
    let relative = super::archive_security::relative_archive_path(name)?;
    if relative.as_os_str().is_empty() {
        return Err(invalid());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum FailureKind {
    Corrupt,
    Unsupported,
    SourceChanged,
    Cancelled,
    Worker,
    Bytes,
    SourceBytes,
    EncodedBytes,
    Entries,
    Depth,
    Seconds,
    Decoded,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Message {
    Start {
        operation: Operation,
    },
    Read {
        source: u64,
        offset: u64,
        length: u32,
    },
    Bytes {
        source: u64,
        offset: u64,
    },
    Write {
        offset: u64,
    },
    Flush,
    Source {
        id: u64,
    },
    Entry {
        entry: Entry,
    },
    ManifestEnd,
    Begin {
        id: u64,
    },
    Chunk {
        id: u64,
        offset: u64,
    },
    End {
        id: u64,
    },
    Decoded {
        bytes: u64,
    },
    Checkpoint,
    Ack {
        pause: bool,
        turbo: bool,
    },
    Paused,
    Resume {
        turbo: bool,
    },
    Finished,
    Failure {
        cause: FailureKind,
        limit: u64,
        detail: String,
    },
}

impl Message {
    fn validate(&self, bytes: usize) -> io::Result<()> {
        let has_data = matches!(
            self,
            Self::Bytes { .. } | Self::Write { .. } | Self::Chunk { .. }
        );
        if bytes > MAX_BLOCK || (!has_data && bytes != 0) {
            return Err(invalid());
        }
        match self {
            Self::Start { operation } => {
                let limits = match operation {
                    Operation::Decode {
                        selected, limits, ..
                    } => {
                        if let Some(name) = selected {
                            validate_name(name)?;
                        }
                        limits
                    }
                    Operation::Encode {
                        format,
                        entries,
                        output_limit,
                        limits,
                        ..
                    } => {
                        if !matches!(
                            format,
                            Format::Zip | Format::SevenZip | Format::Tar | Format::TarZst
                        ) || *entries > limits.entries
                            || *output_limit == 0
                        {
                            return Err(invalid());
                        }
                        limits
                    }
                };
                if limits.entries == 0 || limits.depth == 0 || limits.seconds == 0 {
                    return Err(invalid());
                }
            }
            Self::Read { offset, length, .. } => {
                if *length == 0
                    || *length as usize > MAX_BLOCK
                    || offset.checked_add(*length as u64).is_none()
                {
                    return Err(invalid());
                }
            }
            Self::Write { offset } | Self::Chunk { offset, .. } | Self::Bytes { offset, .. } => {
                if offset.checked_add(bytes as u64).is_none() {
                    return Err(invalid());
                }
            }
            Self::Entry { entry } => entry.validate()?,
            Self::Failure {
                cause,
                limit,
                detail,
            } => {
                if detail.len() > MAX_ERROR
                    || (!matches!(
                        cause,
                        FailureKind::Bytes
                            | FailureKind::SourceBytes
                            | FailureKind::EncodedBytes
                            | FailureKind::Entries
                            | FailureKind::Depth
                            | FailureKind::Seconds
                            | FailureKind::Decoded
                    ) && *limit != 0)
                {
                    return Err(invalid());
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub message: Message,
    pub data: Vec<u8>,
}

impl Frame {
    pub fn validate(&self) -> io::Result<()> {
        self.message.validate(self.data.len())
    }
    pub fn new(message: Message) -> Self {
        Self {
            message,
            data: Vec::new(),
        }
    }
    pub fn data(message: Message, data: Vec<u8>) -> Self {
        Self { message, data }
    }
}

/// Independent sequence in each direction; validate identity before allocating.
pub struct Wire {
    session: [u8; 16],
    send: u64,
    receive: u64,
}

impl Wire {
    pub fn new(session: [u8; 16]) -> Self {
        Self {
            session,
            send: 0,
            receive: 0,
        }
    }

    pub fn write(&mut self, output: &mut impl Write, frame: &Frame) -> io::Result<()> {
        frame.message.validate(frame.data.len())?;
        let metadata = serde_json::to_vec(&frame.message).map_err(|_| invalid())?;
        if metadata.len() > MAX_METADATA || HEADER + metadata.len() + frame.data.len() > MAX_FRAME {
            return Err(invalid());
        }
        let next = self.send.checked_add(1).ok_or_else(invalid)?;
        let mut header = [0u8; HEADER];
        header[..4].copy_from_slice(MAGIC);
        header[4..6].copy_from_slice(&VERSION.to_le_bytes());
        header[8..24].copy_from_slice(&self.session);
        header[24..32].copy_from_slice(&self.send.to_le_bytes());
        header[32..36].copy_from_slice(&(metadata.len() as u32).to_le_bytes());
        header[36..40].copy_from_slice(&(frame.data.len() as u32).to_le_bytes());
        output.write_all(&header)?;
        output.write_all(&metadata)?;
        output.write_all(&frame.data)?;
        output.flush()?;
        self.send = next;
        Ok(())
    }

    pub fn read(&mut self, input: &mut impl Read) -> io::Result<Frame> {
        let mut header = [0u8; HEADER];
        input.read_exact(&mut header)?;
        self.frame_size(&header)?;
        let next = self.receive.checked_add(1).ok_or_else(invalid)?;
        let metadata_len = u32::from_le_bytes(header[32..36].try_into().unwrap()) as usize;
        let data_len = u32::from_le_bytes(header[36..40].try_into().unwrap()) as usize;
        let mut metadata = vec![0; metadata_len];
        input.read_exact(&mut metadata)?;
        let message: Message = serde_json::from_slice(&metadata).map_err(|_| invalid())?;
        message.validate(data_len)?;
        // Serde's internally tagged unit variants otherwise accept extra keys.
        // Require the canonical encoding emitted by the matching protocol version.
        if serde_json::to_vec(&message).map_err(|_| invalid())? != metadata {
            return Err(invalid());
        }
        let mut data = vec![0; data_len];
        input.read_exact(&mut data)?;
        self.receive = next;
        Ok(Frame { message, data })
    }

    /// Inspect a polled pipe header before reserving space for its body.
    pub fn frame_size(&self, header: &[u8]) -> io::Result<usize> {
        if header.len() != HEADER {
            return Err(invalid());
        }
        if &header[..4] != MAGIC
            || u16::from_le_bytes(header[4..6].try_into().unwrap()) != VERSION
            || header[6..8] != [0, 0]
            || header[8..24] != self.session
            || u64::from_le_bytes(header[24..32].try_into().unwrap()) != self.receive
        {
            return Err(invalid());
        }
        self.receive.checked_add(1).ok_or_else(invalid)?;
        let metadata_len = u32::from_le_bytes(header[32..36].try_into().unwrap()) as usize;
        let data_len = u32::from_le_bytes(header[36..40].try_into().unwrap()) as usize;
        if metadata_len == 0
            || metadata_len > MAX_METADATA
            || data_len > MAX_BLOCK
            || HEADER + metadata_len + data_len > MAX_FRAME
        {
            return Err(invalid());
        }
        Ok(HEADER + metadata_len + data_len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(frame: Frame) -> Vec<u8> {
        let mut bytes = Vec::new();
        Wire::new([7; 16]).write(&mut bytes, &frame).unwrap();
        bytes
    }

    #[test]
    fn blocks_are_binary_and_roundtrip_at_the_boundary() {
        let frame = Frame::data(Message::Chunk { id: 8, offset: 2 }, vec![255; MAX_BLOCK]);
        let bytes = encoded(frame.clone());
        assert!(bytes.len() < MAX_BLOCK + 1024);
        assert_eq!(
            Wire::new([7; 16]).read(&mut bytes.as_slice()).unwrap(),
            frame
        );
    }

    #[test]
    fn identity_version_sequence_and_lengths_are_checked_before_body_reads() {
        let bytes = encoded(Frame::new(Message::Checkpoint));
        for index in [0, 4, 6, 8, 24, 35, 39] {
            let mut bad = bytes[..HEADER].to_vec();
            bad[index] ^= 255;
            assert_eq!(
                Wire::new([7; 16])
                    .read(&mut bad.as_slice())
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }
    }

    #[test]
    fn duplicate_and_truncated_frames_fail_closed() {
        let bytes = encoded(Frame::new(Message::Finished));
        let mut wire = Wire::new([7; 16]);
        wire.read(&mut bytes.as_slice()).unwrap();
        assert!(wire.read(&mut bytes.as_slice()).is_err());
        for length in 0..bytes.len() {
            assert!(Wire::new([7; 16]).read(&mut &bytes[..length]).is_err());
        }
    }

    #[test]
    fn metadata_and_raw_payloads_are_validated_before_writing() {
        for frame in [
            Frame::data(Message::Write { offset: u64::MAX }, vec![0]),
            Frame::data(Message::Checkpoint, vec![0]),
            Frame::data(Message::Write { offset: 0 }, vec![0; MAX_BLOCK + 1]),
            Frame::new(Message::Read {
                source: 0,
                offset: 0,
                length: 0,
            }),
            Frame::new(Message::Failure {
                cause: FailureKind::Worker,
                limit: 1,
                detail: String::new(),
            }),
            Frame::new(Message::Failure {
                cause: FailureKind::Worker,
                limit: 0,
                detail: "x".repeat(MAX_ERROR + 1),
            }),
        ] {
            let mut output = Vec::new();
            assert!(Wire::new([1; 16]).write(&mut output, &frame).is_err());
            assert!(output.is_empty());
        }
    }

    #[test]
    fn invalid_names_types_timestamps_and_unknown_metadata_are_refused() {
        let entry = Entry {
            id: 0,
            name: "safe.txt".into(),
            directory: false,
            size: 1,
            modified: None,
            created: None,
            accessed: None,
            mode: 0o644,
        };
        for name in ["../escape", "C:escape", "a:b", "CON", "", "a/../b"] {
            assert!(Entry {
                name: name.into(),
                ..entry.clone()
            }
            .validate()
            .is_err());
        }
        assert!(Entry {
            directory: true,
            ..entry.clone()
        }
        .validate()
        .is_err());
        assert!(Entry {
            modified: Some(Timestamp {
                seconds: 0,
                nanos: 1_000_000_000
            }),
            ..entry
        }
        .validate()
        .is_err());
        let metadata = br#"{"kind":"Checkpoint","extra":0}"#;
        let mut bytes = encoded(Frame::new(Message::Checkpoint));
        bytes.truncate(HEADER);
        bytes[32..36].copy_from_slice(&(metadata.len() as u32).to_le_bytes());
        bytes.extend_from_slice(metadata);
        assert!(Wire::new([7; 16]).read(&mut bytes.as_slice()).is_err());
    }
}
