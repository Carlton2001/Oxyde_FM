use std::fmt;

#[derive(Clone, Debug)]
pub struct HistoryUndoRecoveryFailure {
    pub locations: String,
    pub technical_detail: String,
}
impl fmt::Display for HistoryUndoRecoveryFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.technical_detail)
    }
}
impl std::error::Error for HistoryUndoRecoveryFailure {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryMoveIssue {
    Changed,
    Conflict,
    Unverified,
    Incomplete,
}
#[derive(Clone, Debug)]
pub struct HistoryMoveFailure {
    pub issue: HistoryMoveIssue,
    pub detail: String,
}
impl fmt::Display for HistoryMoveFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Move history {:?}: {}", self.issue, self.detail)
    }
}
impl std::error::Error for HistoryMoveFailure {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewFolderHistoryIssue {
    Changed,
    Unverified,
    NotEmpty,
    Missing,
    Conflict,
    ParentMissing,
}

#[derive(Clone, Debug)]
pub struct NewFolderHistoryFailure {
    pub issue: NewFolderHistoryIssue,
    pub path: String,
}
impl fmt::Display for NewFolderHistoryFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "New-folder history {:?}: {}", self.issue, self.path)
    }
}
impl std::error::Error for NewFolderHistoryFailure {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryIdentityIssue {
    Changed,
    Unverified,
}

#[derive(Clone, Debug)]
pub struct HistoryRenameFailure {
    pub issue: HistoryIdentityIssue,
    pub path: String,
}
impl fmt::Display for HistoryRenameFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rename history {:?}: {}", self.issue, self.path)
    }
}
impl std::error::Error for HistoryRenameFailure {}

#[derive(Clone, Debug)]
pub struct HistoryIdentityFailure {
    pub issue: HistoryIdentityIssue,
    pub path: String,
}
impl fmt::Display for HistoryIdentityFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self.issue {
            HistoryIdentityIssue::Changed => "History file identity changed; replacement preserved",
            HistoryIdentityIssue::Unverified => "Copy history has no verified file identity",
        };
        write!(f, "{reason}: {}", self.path)
    }
}
impl std::error::Error for HistoryIdentityFailure {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryCopyIssue {
    SourceChanged,
    Unverified,
}

#[derive(Clone, Debug)]
pub struct HistoryCopyFailure {
    pub issue: HistoryCopyIssue,
    pub path: String,
}
impl fmt::Display for HistoryCopyFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self.issue {
            HistoryCopyIssue::SourceChanged => "Redo copy source changed since the operation",
            HistoryCopyIssue::Unverified => "Redo copy journal has no verified hash for source",
        };
        write!(f, "{reason}: {}", self.path)
    }
}
impl std::error::Error for HistoryCopyFailure {}

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

#[derive(Debug)]
pub struct ArchiveSourceConflict;
impl ArchiveSourceConflict {
    pub const MARKER: &str = "[archive-source-conflict]";
    const MESSAGE: &str =
        "Archive changed during the operation; original preserved [archive-source-conflict]";
    pub fn from_message(message: &str) -> bool {
        message.contains(Self::MESSAGE)
    }
}
impl fmt::Display for ArchiveSourceConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(Self::MESSAGE)
    }
}
impl std::error::Error for ArchiveSourceConflict {}

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
    HistoryCopy(HistoryCopyFailure),
    HistoryIdentity(HistoryIdentityFailure),
    NewFolderHistory(NewFolderHistoryFailure),
    HistoryMove(HistoryMoveFailure),
    HistoryRename(HistoryRenameFailure),
    HistoryUndoRecovery(HistoryUndoRecoveryFailure),
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
            CommandError::HistoryCopy(failure) => failure.fmt(f),
            CommandError::HistoryIdentity(failure) => failure.fmt(f),
            CommandError::NewFolderHistory(failure) => failure.fmt(f),
            CommandError::HistoryMove(failure) => failure.fmt(f),
            CommandError::HistoryRename(failure) => failure.fmt(f),
            CommandError::HistoryUndoRecovery(failure) => failure.fmt(f),
            CommandError::TrashError(msg) => write!(f, "Trash Error: {}", msg),
            CommandError::TrashFull(msg) => write!(f, "Trash Full: {}", msg),
            CommandError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl From<std::io::Error> for CommandError {
    fn from(err: std::io::Error) -> Self {
        if let Some(failure) = err
            .get_ref()
            .and_then(|error| error.downcast_ref::<HistoryRenameFailure>())
        {
            return Self::HistoryRename(failure.clone());
        }
        if let Some(failure) = err
            .get_ref()
            .and_then(|error| error.downcast_ref::<HistoryUndoRecoveryFailure>())
        {
            return Self::HistoryUndoRecovery(failure.clone());
        }

        if let Some(failure) = err
            .get_ref()
            .and_then(|error| error.downcast_ref::<HistoryMoveFailure>())
        {
            return Self::HistoryMove(failure.clone());
        }

        if let Some(failure) = err
            .get_ref()
            .and_then(|error| error.downcast_ref::<NewFolderHistoryFailure>())
        {
            return Self::NewFolderHistory(failure.clone());
        }
        if let Some(failure) = err
            .get_ref()
            .and_then(|e| e.downcast_ref::<HistoryIdentityFailure>())
        {
            return Self::HistoryIdentity(failure.clone());
        }
        if let Some(failure) = err
            .get_ref()
            .and_then(|e| e.downcast_ref::<HistoryCopyFailure>())
        {
            return Self::HistoryCopy(failure.clone());
        }
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
