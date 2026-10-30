//! A worker's reported failure, turned into the parent's typed errors. Its limits are
//! accepted only if they are the ones the parent gave it.
use crate::models::error::{ArchiveLimitFailure, ArchiveLimitKind, ArchiveSourceConflict};
use crate::utils::archive_io_protocol::{invalid, FailureKind, Frame, Message, Operation};
use std::io;

pub(crate) fn cancelled() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "Operation cancelled")
}
pub(crate) fn failure(frame: &Frame) -> io::Error {
    let Message::Failure {
        cause,
        limit,
        detail,
    } = &frame.message
    else {
        return invalid();
    };
    let kind = match cause {
        FailureKind::Bytes => Some(ArchiveLimitKind::Bytes),
        FailureKind::SourceBytes => Some(ArchiveLimitKind::SourceBytes),
        FailureKind::EncodedBytes => Some(ArchiveLimitKind::EncodedBytes),
        FailureKind::Entries => Some(ArchiveLimitKind::Entries),
        FailureKind::Depth => Some(ArchiveLimitKind::Depth),
        FailureKind::Seconds => Some(ArchiveLimitKind::Seconds),
        FailureKind::Decoded => Some(ArchiveLimitKind::Decoded),
        _ => None,
    };
    if let Some(kind) = kind {
        return io::Error::new(
            if kind == ArchiveLimitKind::Seconds {
                io::ErrorKind::TimedOut
            } else {
                io::ErrorKind::InvalidData
            },
            ArchiveLimitFailure {
                kind,
                limit: *limit,
            },
        );
    }
    match cause {
        FailureKind::SourceChanged => io::Error::other(ArchiveSourceConflict),
        FailureKind::Cancelled => cancelled(),
        FailureKind::Corrupt | FailureKind::Unsupported | FailureKind::Worker => {
            log::warn!("Archive codec failure: {detail:?}");
            io::Error::other(crate::models::error::ArchiveProcessingFailure)
        }
        _ => invalid(),
    }
}

pub(crate) fn checked_failure(frame: &Frame, operation: &Operation) -> io::Error {
    let error = failure(frame);
    let Some(mut value) = ArchiveLimitFailure::from_message(&error.to_string()) else {
        return error;
    };
    let (limits, output) = match operation {
        Operation::Decode { limits, .. } => (limits, None),
        Operation::Encode {
            limits,
            output_limit,
            ..
        } => (limits, Some(*output_limit)),
    };
    let expected = match value.kind {
        ArchiveLimitKind::Bytes | ArchiveLimitKind::SourceBytes => limits.bytes,
        ArchiveLimitKind::EncodedBytes => match output {
            Some(value) => value,
            None => return invalid(),
        },
        ArchiveLimitKind::Entries => limits.entries,
        ArchiveLimitKind::Depth => limits.depth,
        ArchiveLimitKind::Seconds => limits.seconds,
        ArchiveLimitKind::Decoded => match limits
            .entries
            .checked_mul(4096)
            .and_then(|n| limits.bytes.checked_add(n))
        {
            Some(value) => value,
            None => return invalid(),
        },
    };
    if value.limit != expected || (value.kind == ArchiveLimitKind::SourceBytes && output.is_none())
    {
        return invalid();
    }
    if value.kind == ArchiveLimitKind::Bytes && output.is_some() {
        value.kind = ArchiveLimitKind::SourceBytes;
    }
    io::Error::new(error.kind(), value)
}
