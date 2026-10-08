use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveCacheIssue {
    Full,
    Busy,
    Unavailable,
}

impl fmt::Display for ArchiveCacheIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Full => "Archive cache capacity is occupied by protected entries",
            Self::Busy => "Archive cache preparation is still busy",
            Self::Unavailable => "Archive cache could not be prepared",
        })
    }
}
impl std::error::Error for ArchiveCacheIssue {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveLimitKind {
    Bytes,
    Entries,
    Depth,
    Seconds,
    Decoded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchiveLimitFailure {
    pub kind: ArchiveLimitKind,
    pub limit: u64,
}
impl fmt::Display for ArchiveLimitFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.kind {
            ArchiveLimitKind::Bytes => "bytes",
            ArchiveLimitKind::Entries => "entries",
            ArchiveLimitKind::Depth => "depth",
            ArchiveLimitKind::Seconds => "seconds",
            ArchiveLimitKind::Decoded => "decoded",
        };
        write!(f, "Archive limit exceeded [{kind}:{}]", self.limit)
    }
}
impl std::error::Error for ArchiveLimitFailure {}
impl ArchiveLimitFailure {
    /// Preserve the structured cause through existing string-based operation
    /// status and third-party archive errors. Never guess from ordinary prose.
    pub fn from_message(message: &str) -> Option<Self> {
        let start = message.find("Archive limit exceeded [")?;
        let value = &message[start + "Archive limit exceeded [".len()..];
        let (value, _) = value.split_once(']')?;
        let (kind, limit) = value.split_once(':')?;
        let kind = match kind {
            "bytes" => ArchiveLimitKind::Bytes,
            "entries" => ArchiveLimitKind::Entries,
            "depth" => ArchiveLimitKind::Depth,
            "seconds" => ArchiveLimitKind::Seconds,
            "decoded" => ArchiveLimitKind::Decoded,
            _ => return None,
        };
        Some(Self {
            kind,
            limit: limit.parse().ok()?,
        })
    }
}

#[derive(Debug)]
pub enum CommandError {
    IoError(String),
    PathNotFound(String),
    AccessDenied(String),
    PathError(String),
    SystemError(String),
    ArchiveError(String),
    ArchiveCache(ArchiveCacheIssue),
    TrashError(String),
    TrashFull(String),
    Other(String),
}

impl std::error::Error for CommandError {}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::IoError(msg) => write!(f, "IO Error: {}", msg),
            CommandError::PathNotFound(path) => write!(f, "Path not found: {}", path),
            CommandError::AccessDenied(path) => write!(f, "Access denied: {}", path),
            CommandError::PathError(msg) => write!(f, "Path Error: {}", msg),
            CommandError::SystemError(msg) => write!(f, "System Error: {}", msg),
            CommandError::ArchiveError(msg) => write!(f, "Archive Error: {}", msg),
            CommandError::ArchiveCache(issue) => issue.fmt(f),
            CommandError::TrashError(msg) => write!(f, "Trash Error: {}", msg),
            CommandError::TrashFull(msg) => write!(f, "Trash Full: {}", msg),
            CommandError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl From<std::io::Error> for CommandError {
    fn from(err: std::io::Error) -> Self {
        if let Some(issue) = err
            .get_ref()
            .and_then(|e| e.downcast_ref::<ArchiveCacheIssue>())
        {
            return Self::ArchiveCache(*issue);
        }
        CommandError::IoError(err.to_string())
    }
}

impl From<String> for CommandError {
    fn from(err: String) -> Self {
        CommandError::Other(err)
    }
}

impl From<&str> for CommandError {
    fn from(err: &str) -> Self {
        CommandError::Other(err.to_string())
    }
}

#[cfg(target_os = "windows")]
impl From<windows::core::Error> for CommandError {
    fn from(err: windows::core::Error) -> Self {
        CommandError::SystemError(err.to_string())
    }
}
